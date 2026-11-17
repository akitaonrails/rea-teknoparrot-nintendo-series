//! One game run: build the runtime, write the emulator's or loader's files,
//! start the game (on Linux in gamescope with the library read-only), feed
//! its input, wait, and leave nothing behind.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use nintendo_core::catalog::{Game, Route};
use nintendo_core::launch::{self, Feed as Wiring, Places, Plan};
use nintendo_core::runtime;
use nintendo_core::settings::Settings;
use rea_launcher::catalog::{Host, Support};
use rea_launcher::gamescope::CommandLine;
use rea_launcher::library::Entry;
use rea_launcher::linkfarm::{self, Occupied, Op};
use rea_launcher::session;
use rea_launcher::wal;
use rea_launcher::wheel::Script;
use rea_launcher_native::feeder::{self, Feeding, Scripted, Tuning, WheelInfo};
use rea_launcher_native::group::{Group, Tag};
use rea_launcher_native::supervise::{self, Ended, Hold, Options, Outcome};

use crate::input;
use crate::paths;
use rea_launcher::hub::{HubConfig, HubPaths};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Screen,
    /// In a headless gamescope (tests, screenshots).
    Headless,
}

pub struct Prepared {
    pub plan: Plan,
    pub runtime: PathBuf,
    /// The library folders, bound read-only around the game (Linux).
    pub libraries: Vec<PathBuf>,
}

fn check_components(game: &Game, p: &HubPaths) -> Result<()> {
    let c = paths::components(p);
    let windows = Host::current() == Host::Windows;
    let need: Vec<PathBuf> = match game.route {
        Route::Dolphin if windows => vec![c.dolphin_win.join("Dolphin.exe")],
        Route::Dolphin => vec![c.dolphin.join("dolphin-emu-nogui"), c.dolphin.join("Sys")],
        Route::Wal { .. } if windows => vec![
            c.wal.join("arcade-launcher.exe"),
            c.wal.join("payloads"),
            c.wal.join("systemprofiles"),
        ],
        Route::Wal { .. } => vec![
            c.wal.join("arcade-launcher"),
            c.wal.join("payloads"),
            c.wal.join("systemprofiles"),
            c.runner.join("bin/wine"),
        ],
    };
    let missing: Vec<String> = need
        .iter()
        .filter(|f| !f.exists())
        .map(|f| f.display().to_string())
        .collect();
    if !missing.is_empty() {
        bail!(
            "{} needs launcher components that are missing:\n  {}\n(see README, \"For developers\")",
            game.title,
            missing.join("\n  ")
        );
    }
    Ok(())
}

pub fn which(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join(program))
            .find(|p| p.is_file())
    })
}

/// Makes the named pipe Dolphin reads as a pad (replacing anything else
/// by that name: the folder is the launcher's).
#[cfg(unix)]
fn make_fifo(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::FileTypeExt;
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_fifo() => return Ok(()),
        Ok(_) => fs::remove_file(path)?,
        Err(_) => {}
    }
    let ok = Command::new("mkfifo")
        .arg(path)
        .status()
        .context("running mkfifo")?;
    if !ok.success() {
        bail!("could not make the input pipe {}", path.display());
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_fifo(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

/// Build the runtime folder and write the game's files. `wheel` is the
/// wheel the input thread sees (Dolphin on Windows maps it itself).
pub fn prepare(
    paths: &HubPaths,
    config: &HubConfig,
    game: &Game,
    entry: &Entry<Game>,
    settings: &Settings,
    display: Display,
    wheel: Option<&WheelInfo>,
) -> Result<Prepared> {
    if let Support::Blocked(why) = game.support_on(Host::current()) {
        bail!("{} can't start yet: {why}", game.title);
    }
    check_components(game, paths)?;
    let rt = paths.runtime(game.slug);
    let ops = runtime::plan(game, &entry.dir, &rt, &linkfarm::list)
        .with_context(|| format!("reading {}", entry.dir.display()))?;
    if rt.is_dir() {
        for d in linkfarm::dangling(&rt)? {
            linkfarm::remove_link(&d)?;
        }
    }
    linkfarm::apply(&ops, Occupied::Refuse)
        .with_context(|| format!("building {}", rt.display()))?;
    let places = Places {
        runtime: rt.clone(),
        wal_root: paths::wal_root(paths),
        segaboot: config.paths[0].clone().filter(|s| s.is_file()),
        wal_port: wal::free_port_block(launch::WAL_PORT),
    };
    let plan = launch::plan(
        game,
        &entry.file,
        &places,
        &paths::components(paths),
        settings,
        display == Display::Headless,
        Host::current(),
        wheel.map(input::dolphin_wheel).as_ref(),
    );
    for (file, text) in &plan.files {
        if let Some(d) = file.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(file, text).with_context(|| format!("writing {}", file.display()))?;
    }
    let mut links = Vec::new();
    for (at, target) in &plan.links {
        if let Some(d) = at.parent() {
            links.push(Op::Dir(d.to_path_buf()));
        }
        links.push(Op::Link {
            target: target.clone(),
            at: at.clone(),
        });
    }
    linkfarm::apply(&links, Occupied::Refuse)?;
    if let Wiring::DolphinPipe(pipe) = &plan.feed {
        make_fifo(pipe)?;
    }
    Ok(Prepared {
        plan,
        runtime: rt,
        libraries: config.libraries.clone(),
    })
}

/// The command line: gamescope, then bubblewrap with every library folder
/// read-only (nothing the game runs can write to the player's dumps), then
/// the game. With a ReShade effect, gamescope runs with the plan's `HOME`
/// and the game with `user_home` (see [`Plan::gamescope_home`]).
pub fn command_line(
    plan: &Plan,
    libraries: &[PathBuf],
    bwrap: bool,
    user_home: &str,
) -> Vec<String> {
    CommandLine {
        gamescope: plan.gamescope.clone(),
        homes: plan
            .gamescope_home
            .clone()
            .map(|home| (home, user_home.to_string())),
        read_only: bwrap.then(|| libraries.to_vec()),
        cpus: None,
    }
    .argv(&plan.game.program, &plan.game.args)
}

/// The command the session starts: on Linux [`command_line`], on Windows
/// the game's own (no gamescope, no bubblewrap).
fn session_argv(plan: &Plan, libraries: &[PathBuf]) -> Vec<String> {
    if Host::current() == Host::Windows {
        return std::iter::once(plan.game.program.to_string_lossy().into_owned())
            .chain(plan.game.args.iter().cloned())
            .collect();
    }
    let bwrap = which("bwrap").is_some();
    let user_home = std::env::var("HOME").unwrap_or_default();
    command_line(plan, libraries, bwrap, &user_home)
}

/// What drives the game.
pub enum Feed {
    /// The input thread (pads and wheels through SDL).
    Devices {
        tx: Sender<feeder::Command>,
        deadzone: f32,
    },
    /// A timed script (tests).
    Script(Script),
    /// Nothing: the game's input is held open until the stop.
    None,
}

/// The environment variable every process of a session carries (Linux).
const TAG: &str = "NINTENDO_SESSION";

/// How long a stopping game gets to save and quit.
const GRACE: Duration = Duration::from_secs(8);

/// The feed of one run over the game's wiring.
fn feed_for(feed: Feed, wiring: &Wiring) -> Box<dyn supervise::Feed> {
    let scripted = matches!(feed, Feed::Script(_));
    match (feed, input::wire(wiring, scripted)) {
        (Feed::Devices { tx, deadzone }, Some(wire)) => {
            let tuning = Tuning {
                deadzone,
                ..Tuning::default()
            };
            Box::new(Feeding::new(tx, wire, tuning))
        }
        (Feed::Script(script), Some(wire)) => Box::new(Scripted::new(wire, script)),
        _ => Box::new(Hold::default()),
    }
}

/// The game's input when it isn't the loader's stdin: Dolphin's pipe.
fn pipe_sink(wiring: &Wiring) -> Option<Box<dyn Write + Send>> {
    match wiring {
        #[cfg(unix)]
        Wiring::DolphinPipe(pipe) => Some(Box::new(input::FifoSink::new(pipe.clone()))),
        _ => None,
    }
}

/// Dolphin on Linux is asked alone to stop (it saves, then quits, and
/// gamescope with it): a stop to the whole group took gamescope's display
/// away from it first. Elsewhere the session is asked as a whole
/// (Windows: Dolphin's window gets WM_CLOSE).
#[cfg(target_os = "linux")]
struct Polite {
    /// The program asked alone (`dolphin-emu-nogui`).
    alone: Option<&'static str>,
}

#[cfg(target_os = "linux")]
impl Polite {
    fn pid(&self, group: &Group) -> Option<u32> {
        let name = self.alone?;
        group
            .members()
            .into_iter()
            .find(|m| {
                m.argv
                    .first()
                    .is_some_and(|a| std::path::Path::new(a).file_name() == Some(name.as_ref()))
            })
            .map(|m| m.pid)
    }
}

#[cfg(target_os = "linux")]
impl supervise::Watcher for Polite {
    fn ask_to_stop(&mut self, group: &Group) {
        match self.pid(group) {
            Some(pid) if group.terminate_one(pid) => {}
            _ => group.terminate(),
        }
    }

    /// While the emulator asked alone still runs, it is saving.
    fn saving(&mut self, group: &Group) -> bool {
        self.alone.is_none() || self.pid(group).is_some()
    }
}

/// Starts the game in its session (the window's or the command line's
/// thread), feeds it, and waits until it ends or `stop` is set; then
/// nothing of it is left. A game that closed by itself within seconds
/// didn't start: that is an error saying why.
pub fn run(p: &Prepared, feed: Feed, stop: &AtomicBool) -> Result<ExitStatus> {
    let log_path = p.runtime.join("session.log");
    let file = fs::File::create(&log_path)?;
    let argv = session_argv(&p.plan, &p.libraries);
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(&p.plan.game.cwd)
        .envs(p.plan.game.env.iter().map(|(k, v)| (k, v)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if matches!(p.plan.feed, Wiring::Wal(_)) {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let log: supervise::Log = Arc::new(Mutex::new(file));
    writeln!(log.lock().expect("the log"), "# {}", argv.join(" "))?;
    let tag = Tag::new(TAG, p.runtime.to_string_lossy());
    let group = Group::spawn(&mut cmd, &tag).with_context(|| format!("starting {}", argv[0]))?;
    #[cfg(target_os = "linux")]
    let mut watcher = Polite {
        alone: matches!(p.plan.feed, Wiring::DolphinPipe(_)).then_some("dolphin-emu-nogui"),
    };
    #[cfg(not(target_os = "linux"))]
    let mut watcher = ();
    let opts = Options {
        stop,
        log,
        grace: GRACE,
        sink: pipe_sink(&p.plan.feed),
    };
    let out = supervise::run(
        group,
        feed_for(feed, &p.plan.feed).as_mut(),
        &mut watcher,
        opts,
    )?;
    match early(&out, &fs::read_to_string(&log_path).unwrap_or_default()) {
        Some(why) => bail!("{why}"),
        None => Ok(out.status),
    }
}

/// Why a run that ended by itself within seconds didn't start (Windows
/// refusing a program, a missing runtime), from its session `log`.
fn early(out: &Outcome, log: &str) -> Option<String> {
    let itself = out.ended == Ended::Itself;
    itself
        .then(|| session::early_end(out.ran, &out.status.to_string(), log))
        .flatten()
}

// the Linux command line (gamescope, bubblewrap)
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use nintendo_core::catalog::{GameId, game};
    use nintendo_core::launch::Components;
    use rea_launcher_native::supervise::Watcher;
    use std::path::Path;

    fn comps() -> Components {
        Components {
            dolphin: "/c/dolphin".into(),
            dolphin_win: "/c/dw".into(),
            wal: "/c/wal".into(),
            runner: "/c/r".into(),
            winetricks: "/c/w".into(),
            shaders: "/c/s".into(),
        }
    }

    #[test]
    fn the_library_is_bound_read_only_around_the_game() {
        let places = Places {
            runtime: "/d/rt/mkdx".into(),
            wal_root: "/d/wal".into(),
            segaboot: None,
            wal_port: launch::WAL_PORT,
        };
        let plan = launch::plan(
            game(GameId::MarioKartDx),
            Path::new("/lib/x/MK_AGP3_FINAL.exe"),
            &places,
            &comps(),
            &Settings::default(),
            true,
            Host::Linux,
            None,
        );
        let v = command_line(&plan, &["/lib".into(), "/tf".into()], true, "/home/p");
        let s = v.join(" ");
        assert!(s.starts_with("gamescope -w 1360 -h 768 "), "{s}");
        assert!(s.contains(" -- bwrap --dev-bind / / --ro-bind /lib /lib --ro-bind /tf /tf /c/wal/arcade-launcher run "), "{s}");
        let plain = command_line(&plan, &["/lib".into()], false, "/home/p");
        assert!(!plain.join(" ").contains("bwrap"));
        // the CRT effect: gamescope with the runtime's HOME, the game with the player's
        let crt = Settings {
            crt: rea_launcher::gamescope::Crt::Lottes,
            ..Settings::default()
        };
        let plan = launch::plan(
            game(GameId::FZeroAx),
            Path::new("/lib/x/f.iso"),
            &Places {
                runtime: "/d/rt/fzax".into(),
                ..places
            },
            &comps(),
            &crt,
            true,
            Host::Linux,
            None,
        );
        let s = command_line(&plan, &["/lib".into()], true, "/home/p").join(" ");
        assert!(
            s.starts_with("env HOME=/d/rt/fzax/gamescope-home gamescope "),
            "{s}"
        );
        assert!(s.contains(" -- env HOME=/home/p bwrap "), "{s}");
        assert!(
            s.ends_with(
                "/c/dolphin/dolphin-emu-nogui -u /d/rt/fzax/dolphin -p x11 -e /lib/x/f.iso"
            ),
            "{s}"
        );
    }

    /// `--no-input`: windowsarcadeloader quits when its input closes, so
    /// a run without a feed keeps it open until the stop (it was dropped
    /// at the start and the loader quit at once).
    #[test]
    fn without_input_the_loaders_input_stays_open_until_the_stop() {
        let mut cmd = Command::new("cat");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let group = Group::spawn(&mut cmd, &Tag::new(TAG, "nintendo-test-hold")).unwrap();
        let stop = AtomicBool::new(false);
        let opts = Options {
            stop: &stop,
            log: Arc::new(Mutex::new(Vec::<u8>::new())),
            grace: Duration::from_secs(2),
            sink: None,
        };
        let wiring = Wiring::Wal(nintendo_core::input::StickLayout::NamcoEs3Kart);
        let out = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(400));
                stop.store(true, std::sync::atomic::Ordering::SeqCst);
            });
            supervise::run(
                group,
                feed_for(Feed::None, &wiring).as_mut(),
                &mut Polite { alone: None },
                opts,
            )
            .unwrap()
        });
        assert_eq!(out.ended, Ended::Stopped, "{out:?}");
        assert_eq!(out.leftovers, 0);
    }

    /// windowsarcadeloader's port is a free one at each start: a fixed
    /// 33790 held by another program stopped the game.
    #[test]
    fn a_held_input_port_is_not_given_to_the_loader() {
        let held = std::net::TcpListener::bind(("127.0.0.1", launch::WAL_PORT));
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        for c in [
            "components/wal/arcade-launcher",
            "components/wal/payloads/x",
            "components/wal/systemprofiles/x",
            "components/runner/bin/wine",
            "library/mkdx/MK_AGP3_FINAL.exe",
        ] {
            fs::create_dir_all(root.join(c).parent().unwrap()).unwrap();
            fs::write(root.join(c), b"placeholder").unwrap();
        }
        let paths = HubPaths {
            data: root.join("data"),
            components: root.join("components"),
            dev_assets: None,
        };
        let g = game(GameId::MarioKartDx);
        let dir = root.join("library/mkdx");
        let entry = Entry {
            file: dir.join(g.main_file),
            dir,
            found: rea_launcher::library::Found::Known {
                game: g,
                edition: &g.editions[0],
            },
        };
        let p = prepare(
            &paths,
            &HubConfig::new(&paths::FIELDS),
            g,
            &entry,
            &Settings::default(),
            Display::Headless,
            None,
        )
        .unwrap();
        let layer = &p
            .plan
            .files
            .iter()
            .find(|(f, _)| f.ends_with("layer-mkdx.yaml"))
            .unwrap()
            .1;
        assert!(held.is_err() || !layer.contains("port: 33790\n"), "{layer}");
        assert!(layer.contains("port: "), "{layer}");
    }

    /// A game that closed by itself at once says why (it said nothing).
    #[test]
    fn a_game_that_ends_at_once_says_why() {
        use std::os::unix::process::ExitStatusExt;
        let out = |ended, secs| Outcome {
            status: ExitStatus::from_raw(1 << 8),
            ran: Duration::from_secs(secs),
            ended,
            leftovers: 0,
        };
        let log = "wine: starting\nerror: VACUUM.exe missing\n";
        let why = early(&out(Ended::Itself, 2), log).unwrap();
        assert!(why.contains("closed by itself 2 s after starting"), "{why}");
        assert!(why.contains("VACUUM.exe missing"), "{why}");
        assert_eq!(early(&out(Ended::Itself, 60), log), None);
        assert_eq!(early(&out(Ended::Stopped, 2), log), None);
    }

    /// A stop asks Dolphin alone (it saves, then quits); the rest of the
    /// session (gamescope) keeps running until Dolphin is gone.
    #[test]
    #[cfg(target_os = "linux")]
    fn dolphin_is_asked_alone_to_stop() {
        let t = tempfile::tempdir().unwrap();
        let dolphin = t.path().join("dolphin-emu-nogui");
        std::os::unix::fs::symlink("/bin/sleep", &dolphin).unwrap();
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "\"$0\" 30; sleep 30"]).arg(&dolphin);
        let mut group = Group::spawn(&mut cmd, &Tag::new(TAG, "nintendo-test-polite")).unwrap();
        let mut polite = Polite {
            alone: Some("dolphin-emu-nogui"),
        };
        let started = std::time::Instant::now();
        while polite.pid(&group).is_none() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(polite.saving(&group), "Dolphin runs");
        polite.ask_to_stop(&group);
        while polite.saving(&group) && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!polite.saving(&group), "Dolphin stopped");
        assert!(
            group.exited().unwrap().is_none(),
            "the leader was not asked"
        );
        group.kill();
        group.finish().unwrap();
    }

    #[test]
    fn the_input_pipe_is_a_fifo_and_replaces_a_stray_file() {
        use std::os::unix::fs::FileTypeExt;
        let t = tempfile::tempdir().unwrap();
        let pipe = t.path().join("Pipes/launcher");
        fs::create_dir_all(pipe.parent().unwrap()).unwrap();
        fs::write(&pipe, b"not a pipe").unwrap();
        make_fifo(&pipe).unwrap();
        assert!(fs::metadata(&pipe).unwrap().file_type().is_fifo());
        make_fifo(&pipe).unwrap();
    }

    // ------------------------------------------------------------ golden snapshots

    /// The whole Linux command line of every game, plain and with the CRT
    /// effect, with and without bubblewrap (tests/snapshots/session/).
    #[test]
    fn golden_command_lines() {
        let crt = Settings {
            crt: rea_launcher::gamescope::Crt::Lottes,
            ..Settings::default()
        };
        let libraries: Vec<PathBuf> = vec!["/lib".into(), "/tf games".into()];
        let mut out = String::new();
        for g in nintendo_core::catalog::GAMES {
            let places = Places {
                runtime: Path::new("/d/rt").join(g.slug),
                wal_root: "/d/wal".into(),
                segaboot: None,
                wal_port: launch::WAL_PORT,
            };
            let file = Path::new("/lib").join(g.slug).join(g.main_file);
            for (look, s) in [("default", Settings::default()), ("crt", crt.clone())] {
                for headless in [false, true] {
                    let plan =
                        launch::plan(g, &file, &places, &comps(), &s, headless, Host::Linux, None);
                    for bwrap in [true, false] {
                        out.push_str(&format!(
                            "== {} {look}{}{}\n",
                            g.slug,
                            if headless { " headless" } else { "" },
                            if bwrap { "" } else { " no-bwrap" }
                        ));
                        for a in command_line(&plan, &libraries, bwrap, "/home/p") {
                            out.push_str(&format!("  {a}\n"));
                        }
                    }
                }
            }
        }
        rea_launcher::snapshot!("session/command-lines.txt", out);
    }

    /// One line per entry below `dir`, sorted: folders, files with their
    /// size, links with their target, pipes; `root` spelled `<tmp>`.
    fn tree(dir: &Path, root: &Path, out: &mut String) {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        names.sort();
        for path in names {
            let rel = path.strip_prefix(root).unwrap().display().to_string();
            let meta = fs::symlink_metadata(&path).unwrap();
            let kind = meta.file_type();
            if kind.is_symlink() {
                let target = fs::read_link(&path).unwrap().display().to_string();
                let target = target.replace(&root.display().to_string(), "<tmp>");
                out.push_str(&format!("{rel} -> {target}\n"));
            } else if kind.is_dir() {
                out.push_str(&format!("{rel}/\n"));
                tree(&path, root, out);
            } else if std::os::unix::fs::FileTypeExt::is_fifo(&kind) {
                out.push_str(&format!("{rel} (pipe)\n"));
            } else {
                out.push_str(&format!("{rel} ({} bytes)\n", meta.len()));
            }
        }
    }

    /// What `prepare` leaves in the runtime and windowsarcadeloader folders
    /// for every game, over a synthetic game folder and placeholder
    /// components; the game folder is left as it was.
    #[test]
    fn golden_prepared_data_folders() {
        use nintendo_core::catalog::{GAMES, Route};
        use rea_launcher::library::Found;
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let put = |rel: &str, bytes: &[u8]| {
            let f = root.join(rel);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, bytes).unwrap();
        };
        for c in [
            "components/dolphin/dolphin-emu-nogui",
            "components/dolphin/Sys/x",
            "components/wal/arcade-launcher",
            "components/wal/payloads/x",
            "components/wal/systemprofiles/x",
            "components/runner/bin/wine",
        ] {
            put(c, b"placeholder");
        }
        put("segaboot.gcm", b"not segaboot");
        let mut out = String::new();
        for g in GAMES {
            let game_dir = root.join("library").join(g.slug);
            put(
                &format!("library/{}/{}", g.slug, g.main_file),
                b"not the game",
            );
            if matches!(g.route, Route::Wal { .. }) {
                put(&format!("library/{}/data/big.bin", g.slug), &[0; 16]);
                put(&format!("library/{}/notes.txt", g.slug), b"notes");
                put(&format!("library/{}/F/TestMode/op.ini", g.slug), b"stale");
                for (from, _) in g.seeds {
                    put(&format!("library/{}/{from}/settings.ini", g.slug), b"seed");
                }
            }
            let data = root.join("data").join(g.slug);
            let paths = HubPaths {
                data: data.clone(),
                components: root.join("components"),
                dev_assets: None,
            };
            let mut config = HubConfig::new(&paths::FIELDS);
            config.paths[0] = Some(root.join("segaboot.gcm"));
            let entry = Entry {
                dir: game_dir.clone(),
                file: game_dir.join(g.main_file),
                found: Found::Known {
                    game: g,
                    edition: &g.editions[0],
                },
            };
            let mut before = String::new();
            tree(&game_dir, root, &mut before);
            prepare(
                &paths,
                &config,
                g,
                &entry,
                &Settings::default(),
                Display::Headless,
                None,
            )
            .unwrap();
            let mut after = String::new();
            tree(&game_dir, root, &mut after);
            assert_eq!(before, after, "{} touched its game folder", g.slug);
            out.push_str(&format!("== {}\n", g.slug));
            for dir in [paths.runtime(g.slug), paths::wal_root(&paths)] {
                if dir.exists() {
                    tree(&dir, root, &mut out);
                }
            }
        }
        rea_launcher::snapshot!("session/prepared.txt", out);
    }
}
