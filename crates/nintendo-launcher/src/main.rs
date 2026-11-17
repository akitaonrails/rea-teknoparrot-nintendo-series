//! nintendo-launcher: one launcher for the Nintendo arcade racers
//! (F-Zero AX Monster Ride, Mario Kart Arcade GP 2 and GP DX).

// a GUI program on Windows: no console window on a double click; commands
// reach the terminal that started them (console::attach_parent)
#![cfg_attr(windows, windows_subsystem = "windows")]

mod input;
mod paths;
mod proc;
mod session;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nintendo_core::catalog;
use nintendo_core::settings::Settings;
use rea_launcher::catalog::{Host, Support};
use rea_launcher::library::Found;

use crate::paths::Paths;
use crate::session::{Display, Feed};

const USAGE: &str = "\
nintendo-launcher — F-Zero AX and the Mario Kart Arcade GP games

  nintendo-launcher                      open the window (games, settings, controls)
  nintendo-launcher --library <folder>   add a folder with your games (repeatable)
  nintendo-launcher --segaboot <file>    the Triforce test-menu program (segaboot.gcm), optional
  nintendo-launcher --list               list the games found and their state
  nintendo-launcher --play <game>        start a game (fzax, mkgp2, mkdx, luigi)
  nintendo-launcher --game <game> <options>   save options for a game without starting it
  Options (saved in that game's settings):
      --render <WxH|native>            the size the game draws at, e.g. 3840x2160
                                       (Triforce games: 480 lines times 1 to 8)
      --output <WxH>                   the size gamescope shows on the screen
      --upscaler <fsr|nis|linear|nearest>
      --sharpness <0-20>               FSR/NIS sharpness, 0 is sharpest
      --crt <off|lottes>               CRT scanlines and mask
      --fullscreen <on|off>
      --deadzone <0-0.3>               controller stick dead zone
  Test options for --play:
      --headless                       run in an invisible display
      --no-input                       no controller input
      --input-script <file>            timed inputs, see docs/RUNTIME_NOTES.md
      --seconds <n>                    stop the game after n seconds
  nintendo-launcher --help             this text
";

#[derive(Default)]
struct Args {
    libraries: Vec<PathBuf>,
    bios: Option<PathBuf>,
    list: bool,
    play: Option<String>,
    game: Option<String>,
    headless: bool,
    no_input: bool,
    script: Option<PathBuf>,
    seconds: Option<u64>,
    options: Vec<String>,
}

fn parse_args(mut it: impl Iterator<Item = String>) -> Result<Option<Args>, String> {
    let mut a = Args::default();
    while let Some(x) = it.next() {
        let mut value = |what: &str| it.next().ok_or(format!("{x} needs {what}"));
        match x.as_str() {
            "--help" | "-h" => return Ok(None),
            "--library" => a.libraries.push(value("a folder")?.into()),
            "--segaboot" => a.bios = Some(value("a file")?.into()),
            "--list" => a.list = true,
            "--play" => a.play = Some(value("a game")?),
            "--game" => a.game = Some(value("a game")?),
            "--headless" => a.headless = true,
            "--no-input" => a.no_input = true,
            "--input-script" => a.script = Some(value("a file")?.into()),
            "--seconds" => {
                a.seconds = Some(
                    value("a number")?
                        .parse()
                        .map_err(|_| "--seconds needs a number")?,
                )
            }
            "--render" | "--output" | "--upscaler" | "--sharpness" | "--crt" | "--fullscreen"
            | "--deadzone" => {
                let key = &x[2..];
                let v = value("a value")?;
                let line = format!("{key} = {v}");
                if !Settings::parse(&line).1.is_empty() {
                    return Err(format!("bad value for {x}: {v}"));
                }
                a.options.push(line);
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(Some(a))
}

fn load_settings(paths: &Paths, slug: &str) -> Settings {
    let text = std::fs::read_to_string(paths.settings(slug)).unwrap_or_default();
    Settings::parse(&text).0
}

/// Apply `key = value` options to a game's settings file, keeping the rest.
fn save_options(paths: &Paths, slug: &str, options: &[String]) -> std::io::Result<()> {
    let file = paths.settings(slug);
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let (settings, _) = Settings::parse(&format!("{old}\n{}", options.join("\n")));
    if let Some(p) = file.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, settings.save(&old))?;
    std::fs::rename(tmp, file)
}

fn main() -> ExitCode {
    #[cfg(windows)]
    if std::env::args().len() > 1 {
        rea_launcher_win32::console::attach_parent();
    }
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(Some(a)) => a,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let paths = Paths::discover();
    let mut config = paths.config();
    if !args.libraries.is_empty() || args.bios.is_some() {
        for dir in &args.libraries {
            if !dir.is_dir() {
                eprintln!("{} is not a folder", dir.display());
                return ExitCode::from(2);
            }
            config.add_library(&std::path::absolute(dir).unwrap_or(dir.clone()));
        }
        if let Some(b) = &args.bios {
            if !b.is_file() {
                eprintln!("{} is not a file", b.display());
                return ExitCode::from(2);
            }
            config.segaboot = Some(std::path::absolute(b).unwrap_or(b.clone()));
        }
        if let Err(e) = paths.save_config(&config) {
            eprintln!("could not save the settings: {e}");
            return ExitCode::FAILURE;
        }
    }
    if args.list {
        if config.libraries.is_empty() {
            eprintln!("no library yet: run with --library <folder>");
            return ExitCode::FAILURE;
        }
        let scan = catalog::library().scan(&config.libraries);
        for p in &scan.problems {
            eprintln!("couldn't read {p}");
        }
        for e in scan.entries {
            match &e.found {
                Found::Known { game, edition } => {
                    let state = match game.support_on(Host::current()) {
                        Support::Ready => "ready".to_string(),
                        Support::Unverified(note) => format!("starts; {note}"),
                        Support::Blocked(why) => format!("not yet: {why}"),
                    };
                    println!(
                        "{:<5} {:<26} {:<28} {state}\n      {}",
                        game.slug,
                        game.title,
                        edition.label,
                        e.dir.display()
                    );
                }
                Found::Unknown { file, digest } => {
                    println!("unknown {file} {digest}\n      {}", e.dir.display())
                }
            }
        }
        return ExitCode::SUCCESS;
    }
    if !args.options.is_empty() {
        let Some(slug) = args.play.as_ref().or(args.game.as_ref()) else {
            eprintln!("options need --play <game> or --game <game>");
            return ExitCode::from(2);
        };
        if catalog::by_slug(slug).is_none() {
            eprintln!("unknown game {slug}");
            return ExitCode::from(2);
        }
        if let Err(e) = save_options(&paths, slug, &args.options) {
            eprintln!("could not save the options: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Some(slug) = &args.play {
        return play(&paths, slug, &args);
    }
    if args.libraries.is_empty() && args.bios.is_none() && args.game.is_none() {
        return match ui::run(paths) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e:#}");
                ExitCode::FAILURE
            }
        };
    }
    ExitCode::SUCCESS
}

fn play(paths: &Paths, slug: &str, args: &Args) -> ExitCode {
    let Some(game) = catalog::by_slug(slug) else {
        eprintln!("unknown game {slug}");
        return ExitCode::from(2);
    };
    let config = paths.config();
    let found = catalog::library().scan(&config.libraries);
    let Some(entry) = found.by_game().get(&game.id).map(|e| (*e).clone()) else {
        eprintln!("{} is not in your library folders", game.title);
        return ExitCode::FAILURE;
    };
    let settings = load_settings(paths, slug);
    let display = if args.headless {
        Display::Headless
    } else {
        Display::Screen
    };
    let input = (!args.no_input && args.script.is_none()).then(input::InputService::start);
    // the wheel, for Dolphin's own mapping on Windows: give SDL a moment to list it
    let wheel = input.as_ref().and_then(|i| {
        std::thread::sleep(Duration::from_millis(300));
        i.snapshot().wheel
    });
    let prepared = match session::prepare(paths, game, &entry, &settings, display, wheel.as_ref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e:#}");
            return ExitCode::FAILURE;
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        let _ = signal_hook::flag::register(sig, Arc::clone(&stop));
    }
    if let Some(secs) = args.seconds {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            stop.store(true, Ordering::SeqCst);
        });
    }
    let feed = if let Some(f) = &args.script {
        match std::fs::read_to_string(f)
            .map_err(|e| e.to_string())
            .and_then(|t| input::Script::parse(&t))
        {
            Ok(s) => Feed::Script(s),
            Err(e) => {
                eprintln!("{}: {e}", f.display());
                return ExitCode::from(2);
            }
        }
    } else if let Some(i) = &input {
        Feed::Devices {
            tx: i.sender(),
            deadzone: settings.deadzone,
        }
    } else {
        Feed::None
    };
    match session::run(paths, &prepared, feed, stop) {
        Ok(status) => {
            eprintln!(
                "{} ended ({status}); log: {}",
                game.title,
                prepared.runtime.join("session.log").display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(v: &[&str]) -> Result<Option<Args>, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn options_are_checked_before_they_are_saved() {
        let a = parse(&["--game", "mkdx", "--render", "3840x2160", "--crt", "lottes"])
            .unwrap()
            .unwrap();
        assert_eq!(a.options, ["render = 3840x2160", "crt = lottes"]);
        assert!(parse(&["--game", "mkdx", "--crt", "rainbow"]).is_err());
        assert!(parse(&["--render"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--help"]).unwrap().is_none());
    }

    #[test]
    fn saving_options_keeps_the_players_lines() {
        let t = tempfile::tempdir().unwrap();
        let p = Paths {
            data: t.path().into(),
            components: t.path().join("c"),
        };
        std::fs::create_dir_all(t.path().join("settings")).unwrap();
        std::fs::write(p.settings("mkdx"), "# mine\nupscaler = nis\n").unwrap();
        save_options(&p, "mkdx", &["render = 2560x1440".into()]).unwrap();
        let text = std::fs::read_to_string(p.settings("mkdx")).unwrap();
        assert!(text.starts_with("# mine\nupscaler = nis\n"), "{text}");
        assert!(text.contains("render = 2560x1440"));
    }
}
