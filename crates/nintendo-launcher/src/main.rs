//! nintendo-launcher: one launcher for the Nintendo arcade games (F-Zero AX
//! Monster Ride, Mario Kart Arcade GP 2 and GP DX, Luigi's Mansion Arcade).
//! This file wires the command line (`rea_launcher::hub::cli`) to the
//! window, the sessions and the messages; the decisions live in
//! `nintendo-core`.

// a GUI program on Windows: no console window on a double click; commands
// reach the terminal that started them (console::attach_parent)
#![cfg_attr(windows, windows_subsystem = "windows")]

mod input;
mod paths;
mod session;
mod ui;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nintendo_core::catalog::{self, GAMES, Game, Route};
use nintendo_core::input::Button;
use nintendo_core::settings::{self, Look, Settings};
use rea_launcher::catalog::{Host, Support};
use rea_launcher::cfg::KeyStyle;
use rea_launcher::cfg::Value;
use rea_launcher::hub::cli::{self, BAD_COMMAND_LINE, CliSpec, Invocation, Mode, OK, REFUSED};
use rea_launcher::hub::{HubConfig, HubPaths, PathKind, display_path, typed_path};
use rea_launcher::library::{Found, Scan};
use rea_launcher::log::Log;
use rea_launcher::wheel::Script;
use rea_launcher_native::feeder;
use rea_launcher_ui::report::{self, Link};
use rea_launcher_win32::report::{Audience, Mode as Says, Run};

use crate::paths::{DIRS, FIELDS};
use crate::session::{Display, Feed};

/// The single-instance lock's name.
const INSTANCE: &str = "rea-nintendo-launcher";
const ISSUES: &str =
    "https://github.com/akitaonrails/rea-teknoparrot-nintendo-series/issues/new/choose";

/// The games' ids, as `--play` takes them.
const SLUGS: [&str; GAMES.len()] = {
    let mut v = [""; GAMES.len()];
    let mut i = 0;
    while i < v.len() {
        v[i] = GAMES[i].slug;
        i += 1;
    }
    v
};

/// What the command line knows.
const CLI: CliSpec = CliSpec {
    program: "nintendo-launcher",
    about: "F-Zero AX, the Mario Kart Arcade GP games and Luigi's Mansion Arcade",
    games: &SLUGS,
    options: settings::OPTIONS,
    style: KeyStyle::Underscore,
    paths: &FIELDS,
    offers: None,
};

/// The family's view of this run, for who sees its messages.
fn run_of(inv: &Invocation) -> Run {
    Run {
        command: inv.is_command(),
        no_window: inv.no_window(),
        settings_only: false,
        linux: inv.linux,
    }
}

fn main() -> ExitCode {
    #[cfg(windows)]
    if std::env::args().len() > 1 {
        rea_launcher_win32::console::attach_parent();
    }
    let inv = match cli::parse(&CLI, std::env::args().skip(1)) {
        Ok(inv) => inv,
        Err(e) => {
            eprintln!("{e}\n\n{}", cli::usage(&CLI));
            return ExitCode::from(BAD_COMMAND_LINE);
        }
    };
    if inv.mode == Mode::Help {
        print!("{}", cli::usage(&CLI));
        return ExitCode::from(OK);
    }
    let paths = HubPaths::discover(&DIRS);
    let log = Log::new(paths.log());
    let says = match Audience::of(run_of(&inv), false).mode() {
        Says::Command { interactive } => report::Mode::Command { interactive },
        Says::Play { interactive } => report::Mode::Play { interactive },
    };
    let fail = |text: &str| {
        let link = Link {
            label: "Report a problem",
            url: ISSUES,
        };
        rea_launcher_ui::hub::fatal(&ui::THEME, &log, text, Some(link), says);
    };
    // --list only reads: it works while the window is open
    let _instance = match (inv.mode != Mode::List).then(|| rea_launcher::instance::single(INSTANCE))
    {
        Some(Err(running)) if inv.is_action() => {
            fail("The Nintendo Arcade Launcher is already open. Close it first, then try again.");
            return ExitCode::from(running.exit_code() as u8);
        }
        Some(Err(_)) => {
            eprintln!("The Nintendo Arcade Launcher is already open.");
            return ExitCode::from(OK);
        }
        Some(Ok(guard)) => Some(guard),
        None => None,
    };
    match run_mode(&paths, &inv) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            fail(&e);
            ExitCode::from(REFUSED)
        }
    }
}

fn run_mode(paths: &HubPaths, inv: &Invocation) -> Result<u8, String> {
    let file = paths.config_file();
    let mut config = HubConfig::load(&file, &FIELDS)
        .map_err(|e| format!("Couldn't read {}: {e}", display_path(&file)))?;
    if !inv.libraries.is_empty() || inv.paths.iter().any(Option::is_some) {
        configure(paths, &mut config, inv)?;
    }
    match &inv.mode {
        Mode::Window => ui::run(paths.clone()).map(|()| OK),
        Mode::Help | Mode::Configure => Ok(OK),
        Mode::List => list(&config),
        Mode::Check(game) => check(paths, &config, game.as_deref()),
        Mode::Set(slug) => set(paths, game_of(slug), inv).map(|()| OK),
        Mode::Play(slug) => {
            let game = game_of(slug);
            set(paths, game, inv)?;
            play(paths, &config, game, inv)
        }
    }
}

/// A game the parser checked.
fn game_of(slug: &str) -> &'static Game {
    catalog::by_slug(slug).expect("checked by the parser")
}

/// The library folders and paths of the command line saved in `launcher.cfg`.
fn configure(paths: &HubPaths, config: &mut HubConfig, inv: &Invocation) -> Result<(), String> {
    for dir in &inv.libraries {
        config.add_library(&typed_path(&dir.to_string_lossy(), PathKind::Folder, "")?);
    }
    for (n, (given, field)) in inv.paths.iter().zip(&FIELDS).enumerate() {
        if let Some(p) = given {
            config.paths[n] = Some(typed_path(&p.to_string_lossy(), field.kind, field.empty)?);
        }
    }
    config
        .save(&paths.config_file(), &FIELDS)
        .map_err(|e| format!("Could not save the settings: {e}"))
}

/// A game's settings; each line its file couldn't read is a warning.
fn load_settings(paths: &HubPaths, game: &Game) -> Result<Settings, String> {
    let file = paths.settings(game.slug);
    let (s, problems) = settings::load(&file, game.native, screen())
        .map_err(|e| format!("Couldn't read {}: {e}", display_path(&file)))?;
    problems.iter().for_each(|p| warn(p));
    Ok(s)
}

/// The player's screen, for the looks: known on Windows, not from a
/// Linux command line (the window decides there).
fn screen() -> Option<(u32, u32)> {
    #[cfg(windows)]
    return rea_launcher_win32::display::screen_size();
    #[cfg(not(windows))]
    None
}

/// A warning of the command line, on stderr.
fn warn(text: &str) {
    eprintln!("warning: {text}");
}

/// The options of the command line saved into the game's settings.
fn set(paths: &HubPaths, game: &Game, inv: &Invocation) -> Result<(), String> {
    if inv.options.is_empty() {
        return Ok(());
    }
    let base = load_settings(paths, game)?;
    let mut s = base.clone();
    // a look first, then the other flags, which win
    inv.apply_options(settings::OPTIONS, &[], |spec, v| match v {
        Value::Choice(key) if spec.key == "look" => {
            if let Some(look) = Look::from_key(key) {
                look.apply(&mut s, game.native, screen());
            }
        }
        _ => {}
    });
    let looked = s.clone();
    inv.apply_options(settings::OPTIONS, &[], |spec, v| {
        if spec.key != "look" {
            let _ = s.apply(spec.key, v);
        }
    });
    // a picture value changed by a flag: the look is custom now, as in
    // the window
    if settings::PICTURE
        .iter()
        .any(|k| s.value(k) != looked.value(k))
    {
        s.look = None;
    }
    // only what changed and what was given: a look decided for no screen
    // stays out of the file
    let given: Vec<&str> = inv.options.iter().map(|(k, _)| *k).collect();
    settings::write_over(&paths.settings(game.slug), &s, &base, &given)
        .map_err(|e| format!("Could not save the options: {e}"))
}

fn state(game: &Game) -> String {
    match game.support_on(Host::current()) {
        Support::Ready => "ready".to_string(),
        Support::Unverified(note) => format!("starts; {note}"),
        Support::Blocked(why) => format!("not yet: {why}"),
    }
}

/// The games in the library folders; what couldn't be read is a warning.
fn scan(config: &HubConfig) -> Result<Scan<Game>, String> {
    if config.libraries.is_empty() {
        return Err("No game folder yet: run with --library <folder>.".into());
    }
    let scan = catalog::library().scan(&config.libraries);
    for problem in &scan.problems {
        warn(&format!("couldn't read {problem}"));
    }
    Ok(scan)
}

fn list(config: &HubConfig) -> Result<u8, String> {
    for e in scan(config)?.entries {
        let dir = display_path(&e.dir);
        match &e.found {
            Found::Known { game, edition } => println!(
                "{:<5} {:<26} {:<28} {}\n      {dir}",
                game.slug,
                game.title,
                edition.label,
                state(game)
            ),
            Found::Unknown { file, digest } => {
                println!("unknown {file} (SHA-256 {digest}): report it at {ISSUES}\n      {dir}")
            }
        }
    }
    Ok(OK)
}

/// Whether each game (or the one named) could start now: found in the
/// library, a known edition, supported here, its components in place. The
/// unknown builds reported are the named game's only.
fn check(paths: &HubPaths, config: &HubConfig, only: Option<&str>) -> Result<u8, String> {
    let found = scan(config)?;
    let by_game = found.by_game();
    let asked = |g: &Game| only.is_none_or(|s| s == g.slug);
    let mut problems = 0;
    for game in GAMES.iter().filter(|g| asked(g)) {
        let verdict = match (by_game.get(&game.id), game.support_on(Host::current())) {
            (None, _) => Err("not in your game folders".to_string()),
            (Some(_), Support::Blocked(why)) => Err(format!("not yet: {why}")),
            (Some(_), _) => session::check_components(game, paths)
                .map(|()| state(game))
                .map_err(|e| format!("{e:#}")),
        };
        match verdict {
            Ok(s) => println!("{:<5} ok: {s}", game.slug),
            Err(e) => {
                problems += 1;
                println!("{:<5} {e}", game.slug);
            }
        }
    }
    for e in &found.entries {
        if let Found::Unknown { file, digest } = &e.found
            && GAMES.iter().any(|g| asked(g) && g.main_file == file)
        {
            problems += 1;
            println!(
                "unknown {file} (SHA-256 {digest}) in {}: report it at {ISSUES}",
                display_path(&e.dir)
            );
        }
    }
    Ok(if problems == 0 { OK } else { REFUSED })
}

/// Starts `game` without the window and waits until it ends (or a signal,
/// or `--seconds`, stops it).
fn play(
    paths: &HubPaths,
    config: &HubConfig,
    game: &'static Game,
    inv: &Invocation,
) -> Result<u8, String> {
    let found = scan(config)?;
    let entry = found
        .by_game()
        .get(&game.id)
        .map(|e| (*e).clone())
        .ok_or_else(|| format!("{} is not in your game folders.", game.title))?;
    let settings = load_settings(paths, game)?;
    let display = if inv.test.headless {
        Display::Headless
    } else {
        Display::Screen
    };
    // Dolphin on Windows reads the devices itself: a look at the wheel
    // for its mapping, and no input thread
    let reads_itself = game.route == Route::Dolphin && Host::current() == Host::Windows;
    let feeds = !inv.test.no_input && inv.test.script.is_none() && !reads_itself;
    let input = feeds.then(input::service);
    let wheel = reads_itself.then(feeder::find_wheel).flatten();
    let prepared = session::prepare(
        paths,
        config,
        game,
        &entry,
        &settings,
        display,
        wheel.as_ref(),
    )
    .map_err(|e| format!("{e:#}"))?;
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        let _ = signal_hook::flag::register(sig, Arc::clone(&stop));
    }
    if let Some(secs) = inv.test.seconds {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            stop.store(true, Ordering::SeqCst);
        });
    }
    let feed = if let Some(f) = &inv.test.script {
        let text = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
        Feed::Script(Script::parse::<Button>(&text).map_err(|e| format!("{}: {e}", f.display()))?)
    } else if let Some(i) = &input {
        Feed::Devices {
            tx: i.sender(),
            deadzone: settings.deadzone,
        }
    } else {
        Feed::None
    };
    let status = session::run(&prepared, feed, &stop).map_err(|e| format!("{e:#}"))?;
    eprintln!(
        "{} ended ({status}); log: {}",
        game.title,
        display_path(&prepared.runtime.join("session.log"))
    );
    Ok(OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_v(v: &[&str]) -> Result<Invocation, String> {
        cli::parse(&CLI, v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn options_are_checked_before_they_are_saved() {
        let inv = parse_v(&["--set", "mkdx", "--render", "3840x2160", "--crt", "lottes"]).unwrap();
        assert_eq!(
            inv.options,
            [
                ("render", "3840x2160".to_string()),
                ("crt", "lottes".into())
            ]
        );
        for bad in [
            &["--set", "mkdx", "--crt", "rainbow"][..],
            &["--set", "mkdx", "--deadzone", "0.9"],
            &["--render"],
            &["--bogus"],
        ] {
            assert!(parse_v(bad).is_err(), "{bad:?}");
        }
        let e = parse_v(&["--set", "mkdx", "--deadzone", "0.9"]).unwrap_err();
        assert!(e.starts_with("bad value for --deadzone: 0.9"), "{e}");
        assert_eq!(parse_v(&["--help"]).unwrap().mode, Mode::Help);
    }

    /// `--game mkdx` with no options did nothing and said nothing.
    #[test]
    fn set_needs_options_and_game_is_its_old_name() {
        assert!(parse_v(&["--game", "mkdx"]).is_err());
        let inv = parse_v(&["--game", "mkdx", "--deadzone", "0.1"]).unwrap();
        assert_eq!(inv.mode, Mode::Set("mkdx".into()));
    }

    /// `--look` sets the look's values; a flag given with it wins, wherever
    /// it stands, and makes the picture custom.
    #[test]
    fn a_look_sets_its_values_and_the_other_flags_win() {
        let t = tempfile::tempdir().unwrap();
        let p = HubPaths {
            data: t.path().into(),
            components: t.path().join("c"),
            dev_assets: None,
        };
        let inv = parse_v(&["--set", "mkdx", "--upscaler=nis", "--look", "steam_deck"]).unwrap();
        set(&p, game_of("mkdx"), &inv).unwrap();
        let (s, _) = settings::read(&p.settings("mkdx")).unwrap().unwrap();
        assert_eq!(s.look, None, "custom");
        assert_eq!(s.output, rea_launcher::gamescope::Size::new(1280, 800));
        assert_eq!(
            s.upscaler,
            rea_launcher::gamescope::Upscaler::Nis,
            "the flag wins"
        );
        let inv = parse_v(&["--set", "fzax", "--look", "original"]).unwrap();
        set(&p, game_of("fzax"), &inv).unwrap();
        let (s, _) = settings::read(&p.settings("fzax")).unwrap().unwrap();
        assert_eq!((s.look, s.render), (Some(Look::Original), None));
        // a picture flag alone: the look is custom now
        let inv = parse_v(&["--set", "fzax", "--upscaler", "nis"]).unwrap();
        set(&p, game_of("fzax"), &inv).unwrap();
        assert_eq!(
            settings::read(&p.settings("fzax")).unwrap().unwrap().0.look,
            None
        );
    }

    /// `--look X` then a picture flag is a custom picture, as in the window.
    #[test]
    fn a_picture_flag_after_a_look_makes_it_custom() {
        let t = tempfile::tempdir().unwrap();
        let p = HubPaths {
            data: t.path().into(),
            components: t.path().join("c"),
            dev_assets: None,
        };
        let inv = parse_v(&["--set", "mkdx", "--look", "steam_deck", "--upscaler", "nis"]).unwrap();
        set(&p, game_of("mkdx"), &inv).unwrap();
        let (s, _) = settings::read(&p.settings("mkdx")).unwrap().unwrap();
        assert_eq!(s.look, None, "custom");
        assert_eq!(
            s.output,
            rea_launcher::gamescope::Size::new(1280, 800),
            "the look's values"
        );
        assert_eq!(s.upscaler, rea_launcher::gamescope::Upscaler::Nis);
        // a look with a flag that changes nothing it set stays
        let inv = parse_v(&["--set", "mkdx", "--look", "steam_deck", "--deadzone", "0.1"]).unwrap();
        set(&p, game_of("mkdx"), &inv).unwrap();
        let (s, _) = settings::read(&p.settings("mkdx")).unwrap().unwrap();
        assert_eq!(s.look, Some(Look::SteamDeck));
    }

    /// `--set` on a game without a settings file and with the screen
    /// unknown saves the flags but not a look decided for no screen.
    #[test]
    #[cfg(not(windows))]
    fn set_without_a_file_saves_no_look_for_an_unknown_screen() {
        let t = tempfile::tempdir().unwrap();
        let p = HubPaths {
            data: t.path().into(),
            components: t.path().join("c"),
            dev_assets: None,
        };
        let inv = parse_v(&["--set", "fzax", "--deadzone", "0.1"]).unwrap();
        set(&p, game_of("fzax"), &inv).unwrap();
        let (s, _) = settings::read(&p.settings("fzax")).unwrap().unwrap();
        assert_eq!((s.look, s.deadzone), (None, 0.1));
    }

    #[test]
    fn set_saves_the_flags_and_keeps_the_players_lines() {
        let t = tempfile::tempdir().unwrap();
        let p = HubPaths {
            data: t.path().into(),
            components: t.path().join("c"),
            dev_assets: None,
        };
        std::fs::create_dir_all(t.path().join("settings")).unwrap();
        std::fs::write(p.settings("mkdx"), "# mine\nupscaler = nis\n").unwrap();
        let inv = parse_v(&["--set", "mkdx", "--render", "2560x1440"]).unwrap();
        set(&p, game_of("mkdx"), &inv).unwrap();
        let text = std::fs::read_to_string(p.settings("mkdx")).unwrap();
        assert!(text.starts_with("# mine\nupscaler = nis\n"), "{text}");
        assert!(text.contains("render = 2560x1440"));
    }
}
