//! Golden snapshots (rea-launcher-sdk docs/STANDARDS.md, "Golden
//! snapshots"): everything the launcher produces, compared with the files in
//! `tests/snapshots/`, so an SDK bump or a refactor can't change it
//! silently. A diff is reviewed like code: it must match an intended change
//! or an SDK Migration item.
//!
//! `REA_UPDATE_SNAPSHOTS=1 cargo test` rewrites them. The window's
//! AccessKit dumps (`ui/`) and the Linux command line and runtime folders
//! (`session/`) are pinned by the crate's own tests (src/ui.rs,
//! src/session.rs): they reach code a test here can't.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use nintendo_core::catalog::{GAMES, Route};
use nintendo_core::launch::{self, Components, DolphinWheel, Places};
use nintendo_core::settings::Settings;
use rea_launcher::catalog::Host;
use rea_launcher::gamescope::{Crt, Size, Upscaler};
use rea_launcher::linkfarm::{self, Item, Op};
use rea_launcher::wheel::PedalAxis;

/// A path as the snapshot spells it on every system (Windows joins with `\`).
fn p(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

// ------------------------------------------------------------ the CLI

/// A fresh folder for one CLI test, below cargo's scratch folder.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The launcher run with its data and components in `root`: what it
/// printed and how it ended, the scratch folder spelled `<tmp>`.
fn run(root: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_nintendo-launcher"))
        // first: the terminal is the audience (Windows too), so a refused
        // command prints and exits instead of opening a message window
        .arg("--linux")
        .args(args)
        .env("NINTENDO_DATA", root.join("data"))
        .env("NINTENDO_COMPONENTS", root.join("components"))
        // its own single-instance lock: the tests run in parallel
        .env("XDG_RUNTIME_DIR", root)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .expect("run the launcher");
    let text = format!(
        "$ nintendo-launcher {}\nexit {:?}\n--- stdout\n{}--- stderr\n{}",
        args.join(" "),
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    tidy(root, &text)
}

/// `root` spelled `<tmp>`, `/` for `\`, and Wine's drive of an absolute
/// path (`Z:`) left out.
fn tidy(root: &Path, text: &str) -> String {
    let root = p(root);
    text.replace('\\', "/")
        .replace(&format!("Z:{root}"), "<tmp>")
        .replace(&root, "<tmp>")
}

fn file(root: &Path, rel: &str) -> String {
    let text = fs::read_to_string(root.join(rel)).unwrap_or_else(|e| format!("({e})\n"));
    tidy(root, &format!("--- {rel}\n{text}"))
}

/// A library of synthetic files named like the games (never game files):
/// their digests are no catalog edition's, so they list as unknown builds.
fn fixture_library(root: &Path) -> PathBuf {
    let lib = root.join("library");
    let put = |rel: &str, bytes: &[u8]| {
        let f = lib.join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, bytes).unwrap();
    };
    put("MK_AGP3_FINAL.exe", b"not mario kart dx");
    put("luigi/exe/x64/VACUUM.exe", b"not luigi");
    put(
        "triforce/F-Zero AX (Triforce) (Monster) [JAP][BHA6].iso",
        b"not f-zero",
    );
    // a disc image under another name, of no catalog game: not reported
    put("triforce/other.gcm", b"some other disc");
    put("notes.txt", b"a player's notes");
    lib
}

#[test]
fn help() {
    let root = scratch("help");
    let out = Command::new(env!("CARGO_BIN_EXE_nintendo-launcher"))
        .arg("--help")
        .env("NINTENDO_DATA", root.join("data"))
        .output()
        .expect("run the launcher");
    assert!(out.status.success());
    rea_launcher::snapshot!("help.txt", String::from_utf8_lossy(&out.stdout));
}

/// Every CLI mode but the window and a game run, on a synthetic library,
/// with the files each one writes.
#[test]
fn every_cli_mode() {
    let root = scratch("cli");
    let lib = fixture_library(&root);
    let lib_arg = lib.display().to_string();
    let segaboot = root.join("segaboot.gcm");
    fs::write(&segaboot, b"not segaboot").unwrap();
    let sb_arg = segaboot.display().to_string();

    let mut out = String::new();
    let mut step = |args: &[&str], files: &[&str]| {
        out.push_str(&run(&root, args));
        for f in files {
            out.push_str(&file(&root, f));
        }
        out.push('\n');
    };
    // mistakes on the command line
    step(&["--bogus"], &[]);
    step(&["--render"], &[]);
    step(&["--seconds", "soon"], &[]);
    step(&["--game", "mkdx", "--crt", "rainbow"], &[]);
    step(&["--game", "mkdx", "--deadzone", "0.9"], &[]);
    step(&["--render", "1920x1080"], &[]);
    step(&["--game", "nope", "--crt", "off"], &[]);
    // no library yet
    step(&["--list"], &[]);
    step(&["--play", "nope"], &[]);
    // folders and segaboot are remembered
    step(&["--library", &lib_arg], &["data/launcher.cfg"]);
    step(
        &["--library", &lib_arg, "--segaboot", &sb_arg],
        &["data/launcher.cfg"],
    );
    step(&["--list"], &[]);
    // options are saved per game
    step(&["--game", "mkdx"], &[]);
    step(
        &[
            "--game",
            "mkdx",
            "--render",
            "2720x1536",
            "--output",
            "2560x1440",
            "--upscaler",
            "nis",
            "--sharpness",
            "7",
            "--crt",
            "lottes",
            "--fullscreen",
            "off",
            "--deadzone",
            "0.1",
        ],
        &["data/settings/mkdx.cfg"],
    );
    // the output given too: a first file's look comes from the screen,
    // which Windows knows (Wine's 1024x768) and a Linux command line doesn't
    step(
        &[
            "--game",
            "fzax",
            "--render",
            "native",
            "--output",
            "1920x1080",
        ],
        &["data/settings/fzax.cfg"],
    );
    rea_launcher::snapshot!("cli.txt", out);
}

/// The actions the launcher refuses (a path that isn't there, a game it
/// can't start): said on stderr and in launcher.log. Linux only: on Windows
/// a refused action is a message window (the family's audience rule),
/// which a test can't answer.
#[test]
#[cfg(not(windows))]
fn refused_actions() {
    let root = scratch("cli-refused");
    let lib = fixture_library(&root);
    let missing = root.join("missing").display().to_string();
    let mut out = String::new();
    for args in [
        &["--library", &missing][..],
        &["--segaboot", &missing],
        // no library yet
        &["--play", "mkdx"],
        &["--library", &lib.display().to_string()],
        // a synthetic dump is no catalog edition
        &["--play", "mkdx", "--headless", "--no-input"],
    ] {
        out.push_str(&run(&root, args));
        out.push('\n');
    }
    out.push_str(&file(&root, "data/launcher.log"));
    rea_launcher::snapshot!("cli-refused.txt", out);
}

/// `launcher.cfg` written over an older one: what of it survives (comments,
/// unknown and empty keys) and what is read back.
#[test]
fn the_launcher_cfg_over_an_older_one() {
    let root = scratch("cfg");
    let lib = fixture_library(&root);
    fs::create_dir_all(root.join("data")).unwrap();
    fs::write(
        root.join("data/launcher.cfg"),
        "# my notes\n\
         library = /old/games\n\
         library =\n\
         segaboot = /bios/segaboot.gcm\n\
         window = fullscreen\n\
         not a key\n\
         \n\
         library = /old/games\n",
    )
    .unwrap();
    let mut out = run(&root, &["--library", &lib.display().to_string()]);
    out.push_str(&file(&root, "data/launcher.cfg"));
    rea_launcher::snapshot!("launcher-cfg-rewrite.txt", out);
}

// ------------------------------------------------------------ settings

#[test]
fn the_settings_file_from_defaults() {
    rea_launcher::snapshot!("settings-default.cfg.txt", Settings::default().save(""));
}

/// A player's file with comments, unknown, legacy, repeated, commented-out
/// and bad lines: what is read, what is reported, and what is written back.
#[test]
fn a_settings_file_round_trip() {
    let old = "# my notes about Mario Kart\n\
               upscaler = nis\n\
               future_key = 7\n\
               segaboot = /bios/segaboot.gcm\n\
               library = /games\n\
               # crt = lottes\n\
               output=2560x1440\n\
               sharpness = 50\n\
               fullscreen = maybe\n\
               no equals sign\n\
               render = 1920x1080\n\
               render = native\n\
               deadzone = 0.125\n\
               \n\
               [section]\n";
    let (s, problems) = Settings::parse(old);
    let mut out = format!("--- read\n{s:#?}\n--- problems\n");
    for p in &problems {
        let _ = writeln!(out, "{p}");
    }
    let saved = s.save(old);
    let _ = write!(out, "--- saved\n{saved}");
    let (again, problems) = Settings::parse(&saved);
    let _ = writeln!(out, "--- read back the same: {}", again == s);
    let _ = writeln!(out, "--- problems: {problems:?}");
    rea_launcher::snapshot!("settings-round-trip.txt", out);
}

// ------------------------------------------------------------ plans

fn components() -> Components {
    Components {
        dolphin: "/c/dolphin".into(),
        dolphin_win: "/c/dolphin-win".into(),
        wal: "/c/wal".into(),
        runner: "/c/runner".into(),
        winetricks: "/c/winetricks".into(),
        shaders: "/c/shaders".into(),
    }
}

/// The settings each plan is made with, by name.
fn looks() -> Vec<(&'static str, Settings)> {
    vec![
        ("default", Settings::default()),
        (
            "custom",
            Settings {
                look: None,
                render: Some(Size::new(1920, 1080)),
                output: Size::new(2560, 1440),
                upscaler: Upscaler::Nis,
                sharpness: 6,
                crt: Crt::Lottes,
                fullscreen: false,
                deadzone: 0.1,
            },
        ),
        (
            "4k-linear",
            Settings {
                render: Some(Size::new(3840, 2160)),
                upscaler: Upscaler::Linear,
                ..Settings::default()
            },
        ),
        (
            "nearest-small",
            Settings {
                render: Some(Size::new(640, 360)),
                output: Size::new(1280, 720),
                upscaler: Upscaler::Nearest,
                ..Settings::default()
            },
        ),
    ]
}

fn wheels() -> [(&'static str, Option<DolphinWheel>); 3] {
    [
        ("no wheel", None),
        (
            "wheel with split pedals",
            Some(DolphinWheel {
                name: "Test Wheel".into(),
                steer_axis: 0,
                gas: Some(PedalAxis {
                    axis: 1,
                    rest: 0,
                    peak: 32767,
                }),
                brake: Some(PedalAxis {
                    axis: 2,
                    rest: 0,
                    peak: -32768,
                }),
            }),
        ),
        (
            "wheel with resting-high pedals",
            Some(DolphinWheel {
                name: "Other Wheel".into(),
                steer_axis: 3,
                gas: Some(PedalAxis {
                    axis: 4,
                    rest: 32767,
                    peak: -32768,
                }),
                brake: Some(PedalAxis {
                    axis: 5,
                    rest: -32768,
                    peak: 32767,
                }),
            }),
        ),
    ]
}

fn write_plan(out: &mut String, plan: &launch::Plan) {
    let _ = writeln!(out, "size {}", plan.game_size);
    let _ = writeln!(
        out,
        "run {} {}",
        p(&plan.game.program),
        plan.game.args.join(" ").replace('\\', "/")
    );
    let _ = writeln!(out, "cwd {}", p(&plan.game.cwd));
    for (k, v) in &plan.game.env {
        let _ = writeln!(out, "env {k}={}", v.replace('\\', "/"));
    }
    let _ = writeln!(out, "gamescope {}", plan.gamescope.join(" "));
    if let Some(home) = &plan.gamescope_home {
        let _ = writeln!(out, "gamescope HOME {}", p(home));
    }
    let feed = match &plan.feed {
        launch::Feed::Wal(layout) => format!("windowsarcadeloader stdin, {layout:?}"),
        launch::Feed::DolphinPipe(pipe) => format!("Dolphin pipe {}", p(pipe)),
        launch::Feed::None => "none".into(),
    };
    let _ = writeln!(out, "feed {feed}");
    for (at, target) in &plan.links {
        let _ = writeln!(out, "link {} -> {}", p(at), p(target));
    }
    // file contents as written (a loader path keeps its `\`)
    for (f, text) in &plan.files {
        let _ = writeln!(out, "file {}\n{text}", p(f));
    }
}

/// What each game plans for each look, windowed and headless, with and
/// without segaboot and a wheel, on Linux and Windows.
#[test]
fn every_plan_of_every_game() {
    let c = components();
    let mut out = String::new();
    for host in [Host::Linux, Host::Windows] {
        for g in GAMES {
            let game_file = PathBuf::from("/lib").join(g.slug).join(g.main_file);
            let extras = extras(g.route, host);
            for (look, s) in looks() {
                for headless in [false, true] {
                    for (extra, segaboot, wheel) in &extras {
                        let places = Places {
                            runtime: PathBuf::from("/d/runtimes").join(g.slug),
                            wal_root: "/d/wal".into(),
                            segaboot: segaboot.clone(),
                            wal_port: launch::WAL_PORT,
                        };
                        let headless_name = if headless { " headless" } else { "" };
                        let _ =
                            writeln!(out, "== {host:?} {} {look}{headless_name}{extra}", g.slug);
                        let plan = launch::plan(
                            g,
                            &game_file,
                            &places,
                            &c,
                            &s,
                            headless,
                            host,
                            wheel.as_ref(),
                        );
                        write_plan(&mut out, &plan);
                    }
                }
            }
        }
    }
    rea_launcher::snapshot!("plans.txt", out);
}

/// What else a game's plan is made with, named: segaboot (a Triforce game)
/// and a wheel (Dolphin on Windows maps it itself).
fn extras(route: Route, host: Host) -> Vec<(String, Option<PathBuf>, Option<DolphinWheel>)> {
    let segaboots = match route {
        Route::Dolphin => vec![None, Some(PathBuf::from("/bios/segaboot.gcm"))],
        Route::Wal { .. } => vec![None],
    };
    let wheel_cases = match (route, host) {
        (Route::Dolphin, Host::Windows) => wheels().to_vec(),
        _ => wheels()[..1].to_vec(),
    };
    let mut out = Vec::new();
    for sb in segaboots {
        for (wheel_name, wheel) in &wheel_cases {
            let mut name = String::new();
            if sb.is_some() {
                name.push_str(" segaboot");
            }
            if wheel.is_some() {
                name.push_str(&format!(" {wheel_name}"));
            }
            out.push((name, sb.clone(), wheel.clone()));
        }
    }
    out
}

/// The runtime folder each game is given over the same synthetic game
/// folder listing: what is linked, copied, mirrored and seeded.
#[test]
fn every_runtime_plan() {
    let item = |name: &str, dir: bool, size: u64| Item {
        name: name.into(),
        dir,
        size,
    };
    let items = vec![
        item("$RECYCLE.BIN", true, 0),
        item("AMFS", true, 0),
        item("AppData", true, 0),
        item("Data", true, 0),
        item("F", true, 0),
        item(
            "F-Zero AX (Triforce) (Monster) [JAP][BHA6].iso",
            false,
            1 << 30,
        ),
        item("FTestMode", true, 0),
        item("G", true, 0),
        item("GTestMode", true, 0),
        item("MK_AGP3_FINAL.exe", false, 30 << 20),
        item("System Volume Information", true, 0),
        item("WindowsLoader", true, 0),
        item("amfs", true, 0),
        item("big.dat", false, 64 << 20),
        item("exe", true, 0),
        item("game.dll", false, 1000),
        item("game.manifest", false, 100),
        item("init.ps1", false, 100),
        item("log.txt", false, 4 * 1024 * 1024 - 1),
        item("old.windowsloader", false, 10),
        item("settings.ini", false, 4 * 1024 * 1024),
    ];
    // the folders a plan opens (Luigi's way to its program) hold these
    let root = Path::new("/lib/game");
    let ls = |dir: &Path| -> std::io::Result<Vec<Item>> {
        Ok(match dir.file_name().and_then(|n| n.to_str()) {
            _ if dir == root => items.clone(),
            Some("exe") => vec![item("readme.txt", false, 100), item("x64", true, 0)],
            _ => vec![item("VACUUM.exe", false, 30 << 20)],
        })
    };
    let mut out = String::new();
    for g in GAMES {
        let _ = writeln!(out, "== {}", g.slug);
        let ops = linkfarm::plan(root, Path::new("/rt/game"), g, &ls).unwrap();
        for op in ops {
            let line = match op {
                Op::Dir(d) => format!("dir {}", p(&d)),
                Op::Link { target, at } => format!("link {} -> {}", p(&at), p(&target)),
                Op::CopyOnce { from, to } => format!("copy once {} -> {}", p(&from), p(&to)),
                Op::Seed { from, to } => format!("seed {} -> {}", p(&from), p(&to)),
                other => format!("{other:?}"),
            };
            let _ = writeln!(out, "{line}");
        }
    }
    rea_launcher::snapshot!("runtime-plans.txt", out);
}
