//! One game run: build the runtime, write the emulator's or loader's files,
//! start the game (on Linux in gamescope with the library read-only), feed
//! its input, wait, and leave nothing behind.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use nintendo_core::catalog::{Game, Route};
use nintendo_core::input::Aim;
use nintendo_core::launch::{self, DolphinWheel, Feed as Wiring, Places, Plan};
use nintendo_core::runtime;
use nintendo_core::settings::Settings;
use rea_launcher::catalog::{Host, Support};
use rea_launcher::library::Entry;

use crate::input::{self, Script, Wire};
use crate::paths::Paths;
use crate::proc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Screen,
    /// In a headless gamescope (tests, screenshots).
    Headless,
}

pub struct Prepared {
    pub plan: Plan,
    pub runtime: PathBuf,
}

/// What drives the game.
#[derive(Clone)]
pub enum Feed {
    /// The input thread (pads and wheels through SDL).
    Devices {
        tx: Sender<input::Command>,
        deadzone: f32,
    },
    /// A timed script (tests).
    Script(Script),
    /// Nothing.
    None,
}

fn check_components(game: &Game, p: &Paths) -> Result<()> {
    let c = p.components();
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
    paths: &Paths,
    game: &Game,
    entry: &Entry<Game>,
    settings: &Settings,
    display: Display,
    wheel: Option<&DolphinWheel>,
) -> Result<Prepared> {
    if let Support::Blocked(why) = game.support_on(Host::current()) {
        bail!("{} can't start yet: {why}", game.title);
    }
    check_components(game, paths)?;
    let rt = paths.runtime(game.slug);
    let config = paths.config();
    let items =
        runtime::list(&entry.dir).with_context(|| format!("reading {}", entry.dir.display()))?;
    if rt.is_dir() {
        for d in runtime::dangling(&rt)? {
            runtime::remove_link(&d)?;
        }
    }
    runtime::apply(&runtime::plan(game, &entry.dir, &items, &rt))
        .with_context(|| format!("building {}", rt.display()))?;
    let places = Places {
        runtime: rt.clone(),
        wal_root: paths.wal_root(),
        segaboot: config.segaboot.clone().filter(|s| s.is_file()),
    };
    let plan = launch::plan(
        game,
        &entry.file,
        &places,
        &paths.components(),
        settings,
        display == Display::Headless,
        Host::current(),
        wheel,
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
            links.push(runtime::Op::Dir(d.to_path_buf()));
        }
        links.push(runtime::Op::Link {
            target: target.clone(),
            at: at.clone(),
        });
    }
    runtime::apply(&links)?;
    if let Wiring::DolphinPipe(pipe) = &plan.feed {
        make_fifo(pipe)?;
    }
    Ok(Prepared { plan, runtime: rt })
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
    let mut v = Vec::new();
    if let Some(home) = &plan.gamescope_home {
        v.extend(["env".into(), format!("HOME={}", home.display())]);
    }
    v.push("gamescope".to_string());
    v.extend(plan.gamescope.iter().cloned());
    v.push("--".into());
    if plan.gamescope_home.is_some() {
        v.extend(["env".into(), format!("HOME={user_home}")]);
    }
    if bwrap {
        v.extend(["bwrap", "--dev-bind", "/", "/"].map(String::from));
        for l in libraries {
            let l = l.to_string_lossy().into_owned();
            v.extend(["--ro-bind".into(), l.clone(), l]);
        }
    }
    v.push(plan.game.program.to_string_lossy().into_owned());
    v.extend(plan.game.args.iter().cloned());
    v
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

/// Where a script's or the input thread's lines go, and in which form.
fn sink_for(plan: &Plan, child: &mut std::process::Child) -> Option<(Box<dyn Write + Send>, Wire)> {
    match &plan.feed {
        Wiring::Wal(layout) => child
            .stdin
            .take()
            .map(|s| (Box::new(s) as Box<dyn Write + Send>, Wire::Wal(*layout))),
        #[cfg(unix)]
        Wiring::DolphinPipe(pipe) => Some((
            Box::new(input::FifoSink::new(pipe.clone())) as Box<dyn Write + Send>,
            Wire::Dolphin,
        )),
        #[cfg(not(unix))]
        Wiring::DolphinPipe(_) => None,
        Wiring::None => None,
    }
}

/// Run the game until it exits or `stop` is set; then stop everything the
/// session started. Returns the game's exit status.
pub fn run(paths: &Paths, p: &Prepared, feed: Feed, stop: Arc<AtomicBool>) -> Result<ExitStatus> {
    let log_path = p.runtime.join("session.log");
    let log = Arc::new(Mutex::new(fs::File::create(&log_path)?));
    let argv = session_argv(&p.plan, &paths.config().libraries);
    let tag = p.runtime.to_string_lossy().into_owned();
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(&p.plan.game.cwd)
        .envs(p.plan.game.env.iter().map(|(k, v)| (k, v)))
        .stdout(Stdio::piped())
        .stderr(Stdio::from(log.lock().unwrap().try_clone()?));
    cmd.stdin(if matches!(p.plan.feed, Wiring::Wal(_)) {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    proc::configure(&mut cmd, &tag);
    writeln!(log.lock().unwrap(), "# {}", argv.join(" "))?;
    let mut child = cmd
        .spawn()
        .with_context(|| format!("starting {}", argv[0]))?;
    let group = proc::Group::of(&child, &tag).context("grouping the game's processes")?;

    // stdout: force feedback lines to the input thread, the rest to the log
    let out_tx = match &feed {
        Feed::Devices { tx, .. } => Some(tx.clone()),
        _ => None,
    };
    if let Some(stdout) = child.stdout.take() {
        let log = Arc::clone(&log);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if line.starts_with("out ") {
                    if let Some(tx) = &out_tx {
                        let _ = tx.send(input::Command::Output(line));
                    }
                } else {
                    let _ = writeln!(log.lock().unwrap(), "{line}");
                }
            }
        });
    }
    // windowsarcadeloader's input is closed to ask it to stop (it ends the
    // game and gives back what it set up)
    let detach = match &feed {
        Feed::Devices { tx, .. } => Some(tx.clone()),
        _ => None,
    };
    let mut script_sink = None;
    if let Some((sink, wire)) = sink_for(&p.plan, &mut child) {
        match feed {
            Feed::Devices { tx, deadzone } => {
                let _ = tx.send(input::Command::Attach {
                    sink,
                    wire,
                    deadzone,
                });
            }
            Feed::Script(script) => script_sink = Some((sink, wire, script)),
            Feed::None => drop(sink),
        }
    }
    let start = Instant::now();
    let mut last_line = String::new();
    let mut last_sent = Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if stop.load(Ordering::SeqCst) {
            if let Some((mut sink, Wire::Wal(_), _)) = script_sink.take() {
                let _ = writeln!(sink, "quit");
            }
            if let Some(tx) = &detach {
                let _ = tx.send(input::Command::Detach);
            }
            group.terminate();
            let deadline = Instant::now() + Duration::from_secs(8);
            while Instant::now() < deadline && child.try_wait()?.is_none() {
                std::thread::sleep(Duration::from_millis(100));
            }
            group.kill();
            break child.wait()?;
        }
        if let Some((sink, wire, script)) = script_sink.as_mut() {
            let (read, pitch) = script.at(start.elapsed().as_secs_f64());
            // a script places a gun game's crosshair: steer and pitch are its x and y
            let aim = Aim {
                x: read.steer,
                y: pitch,
            };
            let line = wire.line(&read, pitch, aim);
            // resent now and then: Dolphin's pipe drops what came before it opened
            if line != last_line || last_sent.elapsed() > Duration::from_millis(200) {
                let _ = sink.write_all(line.as_bytes()).and_then(|_| sink.flush());
                last_line = line;
                last_sent = Instant::now();
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    group.terminate();
    std::thread::sleep(Duration::from_millis(500));
    group.kill();
    let left = group.leftovers();
    writeln!(
        log.lock().unwrap(),
        "# exit: {status}; stopped {left} leftover process(es)"
    )?;
    Ok(status)
}

// the Linux command line (gamescope, bubblewrap)
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use nintendo_core::catalog::{GameId, game};
    use nintendo_core::launch::Components;
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
            crt: nintendo_core::settings::Crt::Lottes,
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
            crt: nintendo_core::settings::Crt::Lottes,
            ..Settings::default()
        };
        let libraries: Vec<PathBuf> = vec!["/lib".into(), "/tf games".into()];
        let mut out = String::new();
        for g in nintendo_core::catalog::GAMES {
            let places = Places {
                runtime: Path::new("/d/rt").join(g.slug),
                wal_root: "/d/wal".into(),
                segaboot: None,
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
            let paths = Paths {
                data: data.clone(),
                components: root.join("components"),
            };
            fs::create_dir_all(&data).unwrap();
            fs::write(
                paths.config_file(),
                format!("segaboot = {}\n", root.join("segaboot.gcm").display()),
            )
            .unwrap();
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
            // not launcher.cfg: the test wrote it, with the scratch path
            for dir in [paths.runtime(g.slug), paths.wal_root()] {
                if dir.exists() {
                    tree(&dir, root, &mut out);
                }
            }
        }
        rea_launcher::snapshot!("session/prepared.txt", out);
    }
}
