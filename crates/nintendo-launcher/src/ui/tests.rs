//! The window over the launcher's real backend, headless (the SDK's
//! `hub::testing` harness): its games, settings rows, setup fields,
//! controls, sessions, and golden AccessKit dumps of every page.

use std::path::{Path, PathBuf};

use eframe::egui::Vec2;
use eframe::egui::accesskit::Role;
use nintendo_core::catalog::{GAMES, Game, Route};
use nintendo_core::input::Button;
use rea_launcher::catalog::Support;
use rea_launcher::hub::{HubConfig, HubPaths, Tab};
use rea_launcher::library::{Entry, Found, Scan};
use rea_launcher::wheel::{Control, Controls, Kind};
use rea_launcher_native::feeder::{DeviceInfo, Snapshot};
use rea_launcher_ui::hub::HubApp;
use rea_launcher_ui::hub::testing::{self, SIZE_720, SIZE_800, says};
use rea_launcher_ui::testing::{Harness, Queryable};

use super::*;
use crate::paths::FIELDS;

type H = Harness<'static, HubApp<Launcher>>;

fn paths_in(t: &Path) -> HubPaths {
    HubPaths {
        data: t.join("data"),
        components: t.join("components"),
        dev_assets: None,
    }
}

/// A window with `libraries` (and `segaboot`) saved, at `size`.
fn window_with(t: &Path, libraries: &[PathBuf], segaboot: Option<&str>, size: Vec2) -> H {
    let paths = paths_in(t);
    let mut config = HubConfig::new(&FIELDS);
    for l in libraries {
        config.add_library(l);
    }
    config.paths[0] = segaboot.map(PathBuf::from);
    if !libraries.is_empty() || segaboot.is_some() {
        config.save(&paths.config_file(), &FIELDS).unwrap();
    }
    testing::harness(spec(paths.clone()), Launcher::new(paths, None), size)
}

fn window_sized(t: &Path, libraries: &[PathBuf], size: Vec2) -> H {
    window_with(t, libraries, None, size)
}

/// A window over an empty library folder, once its scan is done.
fn hub(t: &Path) -> H {
    let lib = t.join("games");
    std::fs::create_dir_all(&lib).unwrap();
    let mut h = window_sized(t, &[lib], SIZE_720);
    testing::wait(&mut h, "the scan", |a| a.library().is_some());
    h
}

fn index(slug: &str) -> usize {
    GAMES.iter().position(|g| g.slug == slug).unwrap()
}

fn triforce() -> &'static Game {
    GAMES.iter().find(|g| g.route == Route::Dolphin).unwrap()
}

/// Every game found, at `<root>/<slug>`.
fn all_found(h: &mut H, root: &Path) {
    let entries = GAMES
        .iter()
        .map(|g| Entry {
            dir: root.join(g.slug),
            file: root.join(g.slug).join(g.main_file),
            found: Found::Known {
                game: g,
                edition: &g.editions[0],
            },
        })
        .collect();
    h.state_mut().set_library(Scan {
        entries,
        problems: Vec::new(),
    });
    h.run_steps(3);
}

fn select(h: &mut H, slug: &str) {
    let g = &GAMES[index(slug)];
    h.get_by_role_and_label(Role::RadioButton, g.title).click();
    h.run_steps(3);
}

fn click_button(h: &mut H, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    h.run_steps(3);
}

fn open_tab(h: &mut H, tab: Tab) {
    h.get_by_role_and_label(Role::Tab, tab.label()).click();
    h.run_steps(3);
}

fn type_into(h: &mut H, field: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, field).click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::TextInput, field)
        .type_text(text);
}

fn log_of(t: &Path) -> String {
    std::fs::read_to_string(t.join("data/launcher.log")).unwrap_or_default()
}

#[track_caller]
fn wait_says(h: &mut H, words: &str) {
    for _ in 0..500 {
        if says(h, words) {
            return;
        }
        h.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("never said {words:?}");
}

// ------------------------------------------------------------ the hub

#[test]
fn the_theme_meets_wcag_aa_and_the_window_is_not_named_like_a_game() {
    assert_eq!(THEME.contrast_problems(), Vec::<String>::new());
    assert!(GAMES.iter().all(|g| g.title != THEME.title));
    for (h, g) in HUB_GAMES.iter().zip(GAMES) {
        assert_eq!((h.slug, h.title, h.short), (g.slug, g.title, g.short));
    }
}

#[test]
fn the_hub_has_a_tile_for_each_of_the_four_games() {
    let t = tempfile::tempdir().unwrap();
    let h = hub(t.path());
    assert_eq!(h.get_all_by_role(Role::RadioButton).count(), GAMES.len());
    assert!(says(&h, "Your games · 0 of 4 found"));
}

#[test]
fn a_found_game_shows_its_edition_and_folder() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path());
    all_found(&mut h, Path::new("/library"));
    select(&mut h, "mkdx");
    assert!(says(&h, "Namco ES3 · 2013 · 1.18.16 (English mod 1.0)"));
    assert!(says(&h, "Your games · 4 of 4 found"));
}

#[test]
fn a_triforce_game_without_segaboot_says_its_test_menu_is_off() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path());
    all_found(&mut h, Path::new("/library"));
    select(&mut h, triforce().slug);
    assert!(says(&h, "Test menu off: add segaboot.gcm in Settings"));
    select(&mut h, "mkdx");
    assert!(!says(&h, "Test menu off"));
}

/// The real session: without components it can't start, says why, and
/// the error is in launcher.log (it was only on the screen).
#[test]
fn a_game_that_cant_start_says_why_and_the_log_has_it() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path());
    all_found(&mut h, Path::new("/library"));
    select(&mut h, "mkdx");
    click_button(&mut h, "Play");
    wait_says(&mut h, "Mario Kart Arcade GP DX couldn't start");
    assert!(says(&h, "needs launcher components that are missing"));
    assert!(log_of(t.path()).contains("needs launcher components that are missing"));
}

/// The pad moves through the hub: a press steps once, Y opens Settings.
#[test]
fn the_pad_moves_between_tiles_and_opens_settings() {
    use rea_launcher::hub::Page;
    use rea_launcher::pad;
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path());
    let mut press = |h: &mut H, bits: u16| {
        h.state_mut().backend_mut().snapshot.nav = bits;
        h.run_steps(2);
        h.state_mut().backend_mut().snapshot.nav = 0;
        h.run_steps(2);
    };
    press(&mut h, pad::DPAD_RIGHT);
    assert_eq!(h.state().model().selected, 1);
    press(&mut h, pad::Y);
    assert_eq!(h.state().model().page, Page::Settings);
}

// ------------------------------------------------------------ settings

fn settings_text(t: &Path, slug: &str) -> String {
    std::fs::read_to_string(paths_in(t).settings(slug)).unwrap_or_default()
}

/// Every settings row reaches its key in the game's file, in the format
/// the file keeps (the rows the Linux helper owns are off on Windows).
#[test]
fn every_settings_row_reaches_its_key_in_the_file() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path());
    select(&mut h, "mkdx");
    open_tab(&mut h, Tab::Settings);
    let steps: &[(&str, &str, &str)] = &[
        ("Game render size", "next", "render = 1920x1080"),
        ("Dead zone", "next", "deadzone = 0.06"),
        ("Output size", "previous", "output = 2560x1440"),
        ("Upscaler", "next", "upscaler = nis"),
        ("Sharpness", "next", "sharpness = 6"),
        ("Fullscreen", "next", "fullscreen = false"),
    ];
    for (label, way, line) in steps {
        if cfg!(windows) && !matches!(*label, "Game render size" | "Dead zone") {
            // shown off, with the reason (it was hidden)
            let row = h.get_by_role_and_label(Role::Button, label);
            assert!(rea_launcher_ui::testing::disabled(&row), "{label}");
            let why = rea_launcher_ui::testing::description(&row).unwrap();
            assert_eq!(why, WORDS.not_here, "{label}");
            continue;
        }
        click_button(&mut h, &format!("{label}: {way}"));
        let text = settings_text(t.path(), "mkdx");
        assert!(text.contains(line), "{label}: {text}");
    }
    let (read, problems) = settings::load(&paths_in(t.path()).settings("mkdx")).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(read.deadzone, 0.06);
}

#[test]
fn sharpness_is_off_without_fsr_or_nis() {
    let mut f = Form::new(Settings::default(), Size::new(1360, 768));
    assert!(f.disabled("sharpness").is_none());
    assert!(f.adjust("upscaler", 2, false));
    assert_eq!(f.value("upscaler"), "Smooth");
    assert_eq!(f.disabled("sharpness").as_deref(), Some(SHARPNESS_OFF));
    assert!(!f.adjust("sharpness", 1, false));
}

#[test]
fn render_and_output_step_through_their_sizes_and_keep_a_custom_one() {
    let mut f = Form::new(Settings::default(), Size::new(1360, 768));
    assert_eq!(f.value("render"), "Native, 1360 × 768");
    f.adjust("render", 9, false);
    assert_eq!(f.value("render"), "3840 × 2160 (4K)");
    assert!(f.adjust("render", 1, true), "Enter wraps around");
    assert_eq!(f.settings.render, None);
    f.settings.output = Size::new(3440, 1440);
    assert_eq!(f.value("output"), "3440 × 1440");
    f.adjust("output", 1, false);
    assert_eq!(f.settings.output, Size::new(3840, 2160));
    for _ in 0..40 {
        f.adjust("deadzone", 1, false);
    }
    assert_eq!(f.value("deadzone"), "30%");
}

/// A value the launcher couldn't read is logged when the page opens and
/// stays in the file when another row is saved.
#[test]
fn a_bad_settings_line_is_logged_and_kept() {
    let t = tempfile::tempdir().unwrap();
    let file = paths_in(t.path()).settings("mkdx");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "fullscreen = maybe\n").unwrap();
    let mut h = hub(t.path());
    select(&mut h, "mkdx");
    open_tab(&mut h, Tab::Settings);
    assert!(log_of(t.path()).contains("fullscreen needs on or off"));
    click_button(&mut h, "Dead zone: next");
    assert!(settings_text(t.path(), "mkdx").starts_with("fullscreen = maybe\n"));
}

// ------------------------------------------------------------ setup

#[test]
fn the_setup_page_takes_the_segaboot_path() {
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path());
    open_tab(&mut h, Tab::Settings);
    click_button(&mut h, "Folders and files");
    click_button(&mut h, "Use file");
    assert!(says(&h, "Type the path of segaboot.gcm first."));
    let sb = t.path().join("segaboot.gcm");
    std::fs::write(&sb, b"x").unwrap();
    type_into(&mut h, "segaboot.gcm file", &sb.display().to_string());
    h.run_steps(2);
    click_button(&mut h, "Use file");
    assert!(says(&h, "Triforce test menu saved."));
    let config = HubConfig::load(&paths_in(t.path()).config_file(), &FIELDS).unwrap();
    assert_eq!(config.paths[0], Some(sb));
}

// ------------------------------------------------------------ controls

fn wheel_and_pad() -> Snapshot {
    let mut controls = Controls {
        steer: -0.5,
        accel: 0.25,
        brake: 0.75,
        buttons: 0,
    };
    controls.press(Button::Start);
    Snapshot {
        devices: vec![
            DeviceInfo {
                name: "Test Pad".into(),
                kind: Kind::Pad,
                haptic: false,
            },
            DeviceInfo {
                name: "Test Wheel".into(),
                kind: Kind::Wheel,
                haptic: true,
            },
        ],
        driver: Some(1),
        controls,
        feeding: true,
        ..Snapshot::default()
    }
}

#[test]
fn the_controls_page_shows_the_layout_the_wheel_and_live_input() {
    let v = controls_view(&wheel_and_pad());
    assert_eq!(v.devices[v.driver.unwrap()].name, "Test Wheel");
    assert!(v.layout.contains(&("Item / boost".into(), "A".into())));
    assert!(v.layout.contains(&("Gas".into(), "RT".into())));
    assert_eq!(v.buttons.len(), Button::ALL.len());
    assert!(v.buttons.iter().any(|b| b.key == "start" && b.pressed));
    assert_eq!((v.steer, v.accel, v.brake), (-0.5, 0.25, 0.75));
    let t = tempfile::tempdir().unwrap();
    let mut h = hub(t.path());
    h.state_mut().backend_mut().snapshot = wheel_and_pad();
    open_tab(&mut h, Tab::Controls);
    assert!(says(&h, "Wheel: Test Wheel (drives the car)"));
    assert!(says(&h, "A game is running: it reads these controls."));
}

// ------------------------------------------------------------ fit and goldens

/// Every page and state the goldens pin, by name; `root` is the window's
/// scratch folder.
type Setup = Box<dyn Fn(&mut H, &Path)>;

/// A golden state: its name, whether a library folder is saved, the
/// segaboot path saved, and how the window gets there.
type State = (String, bool, Option<&'static str>, Setup);

/// A session the test drives: it reports what arrives on the channel and
/// ends with what the last message says.
enum Step {
    Playing,
    End(Result<(), String>),
}

fn scripted(h: &mut H, steps: Vec<Step>) {
    let session: Session = Box::new(move |r: &Reporter| {
        for step in steps {
            match step {
                Step::Playing => r.playing(),
                Step::End(e) => return e,
            }
        }
        while !r.stop_requested() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(())
    });
    h.state_mut().backend_mut().next = Some(session);
    all_found(h, Path::new("/library"));
    select(h, "mkdx");
    click_button(h, "Play");
    h.run_steps(5);
}

fn states() -> Vec<State> {
    let mut v: Vec<State> = vec![
        ("first-run".into(), false, None, Box::new(|_, _| {})),
        (
            "first-run-empty-add".into(),
            false,
            None,
            Box::new(|h, _| {
                click_button(h, "Add folder");
                click_button(h, "Use file");
            }),
        ),
        ("hub-none-found".into(), true, None, Box::new(|_, _| {})),
        (
            "hub-blocked".into(),
            true,
            None,
            Box::new(|h, _| {
                h.state_mut().backend_mut().supports[index("mkdx")] =
                    Support::Blocked("Not yet: it crashes.");
                all_found(h, Path::new("/library"));
                select(h, "mkdx");
            }),
        ),
        (
            "hub-ready".into(),
            true,
            None,
            Box::new(|h, _| {
                h.state_mut().backend_mut().supports[index("mkdx")] = Support::Ready;
                all_found(h, Path::new("/library"));
                select(h, "mkdx");
            }),
        ),
        (
            "hub-not-found".into(),
            true,
            None,
            Box::new(|h, _| select(h, "mkdx")),
        ),
        (
            "controls-none".into(),
            true,
            None,
            Box::new(|h, _| open_tab(h, Tab::Controls)),
        ),
        (
            "controls-error".into(),
            true,
            None,
            Box::new(|h, _| {
                h.state_mut().backend_mut().snapshot.error =
                    Some("controllers unavailable: no SDL".into());
                open_tab(h, Tab::Controls);
            }),
        ),
        (
            "controls-wheel-and-pad".into(),
            true,
            None,
            Box::new(|h, _| {
                h.state_mut().backend_mut().snapshot = wheel_and_pad();
                open_tab(h, Tab::Controls);
            }),
        ),
        (
            "settings-saved".into(),
            true,
            None,
            Box::new(|h, _| {
                open_tab(h, Tab::Settings);
                click_button(h, "Dead zone: next");
            }),
        ),
        (
            "settings-folders".into(),
            true,
            Some("/bios/segaboot.gcm"),
            Box::new(|h, _| {
                open_tab(h, Tab::Settings);
                click_button(h, "Folders and files");
            }),
        ),
        (
            "hub-triforce-with-segaboot".into(),
            true,
            Some("/bios/segaboot.gcm"),
            Box::new(|h, _| {
                all_found(h, Path::new("/library"));
                select(h, triforce().slug);
            }),
        ),
        (
            "running-getting-ready".into(),
            true,
            None,
            Box::new(|h, _| scripted(h, vec![])),
        ),
        (
            "running-playing".into(),
            true,
            None,
            Box::new(|h, _| scripted(h, vec![Step::Playing])),
        ),
        (
            "message-stopped-with-error".into(),
            true,
            None,
            Box::new(|h, _| {
                scripted(
                    h,
                    vec![Step::Playing, Step::End(Err("dolphin crashed.".into()))],
                );
                wait_says(h, "stopped with an error");
            }),
        ),
        (
            "message-could-not-start".into(),
            true,
            None,
            Box::new(|h, _| {
                all_found(h, Path::new("/library"));
                select(h, "mkdx");
                click_button(h, "Play");
                wait_says(h, "couldn't start");
            }),
        ),
    ];
    for g in GAMES {
        v.push((
            format!("hub-found-{}", g.slug),
            true,
            None,
            Box::new(|h, _| {
                all_found(h, Path::new("/library"));
                select(h, g.slug);
            }),
        ));
        v.push((
            format!("settings-{}", g.slug),
            true,
            None,
            Box::new(|h, _| {
                select(h, g.slug);
                open_tab(h, Tab::Settings);
            }),
        ));
    }
    v
}

/// The window in each state at 1280×720 and 1280×800: every control fits,
/// and the AccessKit tree (with positions) matches its golden dump. The
/// scratch folder is spelled `<tmp>` in the dumps.
#[test]
fn every_page_fits_and_matches_its_golden_dump() {
    let host = if cfg!(windows) { "windows" } else { "linux" };
    for (state, library, segaboot, setup) in states() {
        for size in [SIZE_720, SIZE_800] {
            let t = tempfile::tempdir().unwrap();
            let root = t.path();
            let lib = root.join("games");
            std::fs::create_dir_all(&lib).unwrap();
            let libraries = if library { vec![lib] } else { vec![] };
            let mut h = window_with(root, &libraries, segaboot, size);
            if library {
                testing::wait(&mut h, "the scan", |a| a.library().is_some());
            }
            setup(&mut h, root);
            h.run_steps(3);
            let misfits = testing::misfits(&h);
            assert!(misfits.is_empty(), "{state} at {size:?}: {misfits:#?}");
            let shown = |p: &Path| {
                let s = p.display().to_string();
                (s.replace('\\', "\\\\"), s)
            };
            let (escaped, plain) = shown(root);
            let dump = testing::dump(&h)
                .replace(&escaped, "<tmp>")
                .replace(&plain, "<tmp>");
            let at = format!("{state}-{}x{}", size.x, size.y);
            rea_launcher::snapshot!(&format!("ui/{host}/{at}.txt"), dump);
        }
    }
}
