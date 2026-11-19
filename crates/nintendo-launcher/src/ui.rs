//! The launcher window: the SDK's hub (`rea_launcher_ui::hub`, the third
//! sanctioned look) over this launcher's games, settings, sessions and
//! controllers. The window, its pages, its threads and its input are the
//! SDK's; what is here is the game knowledge it asks for.

use std::io;
use std::path::{Path, PathBuf};

use eframe::egui::Color32;
use nintendo_core::catalog::{self, GAMES, Game, Route};
use nintendo_core::input::Button;
use nintendo_core::settings::{self, Look, Settings};
use rea_launcher::catalog::{Host, Support};
use rea_launcher::gamescope::{Size, Upscaler};
use rea_launcher::hub::bindings::Bindings;
use rea_launcher::hub::{self, HubConfig, HubPaths, Input};
use rea_launcher::library::{self, HashCache, Scan};
use rea_launcher::log::Log;
use rea_launcher::pad::PadNav;
use rea_launcher::wheel::{Control, PadInput};
use rea_launcher_native::feeder::{InputService, Snapshot};
use rea_launcher_ui::hub::{
    ButtonView, ControlsView, Device, Found, HubBackend, HubForm, HubGame, HubLayout, HubSpec,
    HubTheme, HubWords, Note, Reporter, Scanner, Session,
};

use crate::paths::FIELDS;
use crate::session::{self, Display, Feed};

/// A race track seen from the driver's seat: dark asphalt, lamp amber and
/// the blue-and-white of track signs.
pub const THEME: HubTheme = HubTheme {
    base: Color32::from_rgb(0x0B, 0x12, 0x1A),
    panel: Color32::from_rgb(0x14, 0x1E, 0x2A),
    line: Color32::from_rgb(0x6B, 0x7A, 0x8C),
    lamp: Color32::from_rgb(0xFF, 0xC8, 0x57),
    lamp_hover: Color32::from_rgb(0xFF, 0xD7, 0x84),
    sign: Color32::from_rgb(0x1D, 0x4F, 0x8F),
    ink: Color32::from_rgb(0xF4, 0xF7, 0xF2),
    muted: Color32::from_rgb(0x8A, 0x97, 0xA6),
    signal: Color32::from_rgb(0xB3, 0x26, 0x1E),
    error: Color32::from_rgb(0xFF, 0x8A, 0x80),
    disabled: Color32::from_rgb(0x5E, 0x6B, 0x7A),
    focus_fill: Color32::from_rgb(0x1F, 0x2C, 0x3B),
    scrim: 240,
    brand: "ARC",
    title: "Nintendo Arcade Launcher",
    app_id: "rea-nintendo",
};

/// Four games: tiles kept to the left.
const LAYOUT: HubLayout = HubLayout {
    tile_max_width: Some(208.0),
    ..HubLayout::DEFAULT
};

const WORDS: HubWords = HubWords {
    setup_intro: "Type the path of a folder that holds your game folders (TeknoParrot layout) \
                  and add it. The launcher looks inside for each game and never changes \
                  what's there.",
    ..HubWords::DEFAULT
};

const fn hub_game(g: &Game) -> HubGame {
    HubGame {
        slug: g.slug,
        title: g.title,
        short: g.short,
        board: g.board,
        year: g.year,
    }
}

static HUB_GAMES: [HubGame; 4] = [
    hub_game(&GAMES[0]),
    hub_game(&GAMES[1]),
    hub_game(&GAMES[2]),
    hub_game(&GAMES[3]),
];

/// The window's description of the launcher.
pub fn spec(paths: HubPaths) -> HubSpec {
    HubSpec {
        theme: THEME,
        layout: LAYOUT,
        words: WORDS,
        fields: &FIELDS,
        paths,
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Opens the window and runs it until it is closed.
pub fn run(paths: HubPaths) -> Result<(), String> {
    let log = Log::new(paths.log());
    let bindings = crate::input::bindings(&paths, |w| log.warn(w));
    let mut launcher = Launcher::new(paths.clone(), Some(crate::input::service(&bindings)));
    launcher.bindings = bindings;
    rea_launcher_ui::hub::run(spec(paths), launcher)
}

/// The hub's backend: the games, their scan, settings and sessions.
pub struct Launcher {
    paths: HubPaths,
    log: Log,
    /// The digests every scan shares (a rescan during a scan too).
    cache: HashCache,
    /// Owns SDL; `None` in the headless tests.
    input: Option<InputService>,
    /// Each game's support on this system (the tests set others).
    pub supports: Vec<Support>,
    /// What the Controls page shows when there is no input thread (tests).
    pub snapshot: Snapshot,
    /// The pads' menu steps (one per press).
    nav: PadNav,
    /// The player's pad layout (`controls.cfg`).
    bindings: Bindings<Button>,
    /// The next Play's session, made by the test.
    #[cfg(test)]
    pub next: Option<Session>,
}

impl Launcher {
    pub fn new(paths: HubPaths, input: Option<InputService>) -> Launcher {
        Launcher {
            log: Log::new(paths.log()),
            paths,
            cache: HashCache::default(),
            input,
            supports: GAMES
                .iter()
                .map(|g| g.support_on(Host::current()))
                .collect(),
            snapshot: Snapshot::default(),
            nav: PadNav::default(),
            bindings: Bindings::new(nintendo_core::input::default_pad_map()),
            #[cfg(test)]
            next: None,
        }
    }

    /// Hands the changed pad layout to the input thread; what the
    /// Controls page says (a save that failed also goes to `launcher.log`).
    fn bindings_changed(&self, done: Result<String, String>) -> Note {
        if let Some(i) = &self.input {
            i.set_pad_map(self.bindings.map().clone());
        }
        match done {
            Ok(said) => Note::ok(said),
            Err(e) => {
                self.log.error(&e);
                Note::error(e)
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        self.input
            .as_ref()
            .map_or_else(|| self.snapshot.clone(), InputService::snapshot)
    }
}

impl HubBackend for Launcher {
    type Library = Scan<Game>;
    type Form = Form;

    fn games(&self) -> &[HubGame] {
        &HUB_GAMES
    }

    fn options(&self) -> &'static [rea_launcher::cfg::OptionSpec] {
        settings::OPTIONS
    }

    fn support(&self, game: usize) -> Support {
        self.supports[game]
    }

    fn note(&self, game: usize, config: &HubConfig) -> Option<Note> {
        (GAMES[game].route == Route::Dolphin && config.paths[0].is_none())
            .then(|| Note::ok("Test menu off: add segaboot.gcm in Settings to open it."))
    }

    fn scanner(&self) -> Scanner<Scan<Game>> {
        let library = catalog::library().with_cache(self.cache.clone());
        Box::new(move |roots| library.scan(roots))
    }

    fn found(library: &Scan<Game>, game: usize) -> Option<Found> {
        let entry = *library.by_game().get(&GAMES[game].id)?;
        let edition = match &entry.found {
            library::Found::Known { edition, .. } => Some(edition.label.to_string()),
            library::Found::Unknown { .. } => None,
        };
        Some(Found {
            dir: entry.dir.clone(),
            edition,
        })
    }

    fn unknown(library: &Scan<Game>) -> Vec<String> {
        let unknown = library.entries.iter().filter_map(|e| match &e.found {
            library::Found::Unknown { file, digest } => Some(format!(
                "unknown build {file} {digest} in {}",
                hub::display_path(&e.dir)
            )),
            library::Found::Known { .. } => None,
        });
        let unread = library
            .problems
            .iter()
            .map(|p| format!("couldn't read {p}"));
        unknown.chain(unread).collect()
    }

    fn form(&mut self, game: usize, screen: Option<(u32, u32)>) -> Form {
        let (file, native) = (self.paths.settings(GAMES[game].slug), GAMES[game].native);
        let settings = match settings::load(&file, native, screen) {
            Ok((s, problems)) => {
                for p in problems {
                    self.log.line(&format!("{}: {p}", hub::display_path(&file)));
                }
                s
            }
            Err(e) => {
                self.log
                    .error(&format!("{}: {e}", hub::display_path(&file)));
                Settings::default()
            }
        };
        Form::new(settings, native, screen)
    }

    fn session(
        &mut self,
        game: usize,
        library: &Scan<Game>,
        config: &HubConfig,
        screen: Option<(u32, u32)>,
    ) -> Result<Session, String> {
        #[cfg(test)]
        if let Some(s) = self.next.take() {
            return Ok(s);
        }
        let g = &GAMES[game];
        let entry = (*library
            .by_game()
            .get(&g.id)
            .ok_or_else(|| format!("{} is not in your library folders", g.title))?)
        .clone();
        let file = self.paths.settings(g.slug);
        let (settings, problems) = settings::load(&file, g.native, screen)
            .map_err(|e| format!("{} can't be read: {e}", hub::display_path(&file)))?;
        for p in problems {
            self.log.line(&format!("{}: {p}", hub::display_path(&file)));
        }
        let wheel = self.snapshot().wheel;
        let feed = match &self.input {
            Some(i) => Feed::Devices {
                tx: i.sender(),
                deadzone: settings.deadzone,
            },
            None => Feed::None,
        };
        let (paths, config) = (self.paths.clone(), config.clone());
        Ok(Box::new(move |r: &Reporter| {
            let prepared = session::prepare(
                &paths,
                &config,
                g,
                &entry,
                &settings,
                Display::Screen,
                wheel.as_ref(),
            )
            .map_err(|e| format!("{e:#}"))?;
            r.playing();
            session::run(&prepared, feed, &r.stop_flag())
                .map(drop)
                .map_err(|e| format!("{e:#}"))
        }))
    }

    fn game_log(&self, game: usize) -> Option<PathBuf> {
        Some(self.paths.runtime(GAMES[game].slug).join("session.log"))
    }

    fn controls(&self) -> ControlsView {
        controls_view(&self.snapshot(), &self.bindings)
    }

    fn pad(&mut self) -> Vec<Input> {
        let bits = self.snapshot().nav;
        Input::from_pad(&mut self.nav, bits)
    }

    fn pad_now(&self) -> Option<(u16, Option<PadInput>)> {
        let snap = self.snapshot();
        Some((snap.nav, snap.held))
    }

    fn rebindable(&self) -> Vec<usize> {
        self.bindings.rebindable()
    }

    fn rebind(&mut self, row: usize, input: PadInput) -> Note {
        let file = self.paths.controls_file();
        let done = self.bindings.rebind_saved(row, input, &file);
        self.bindings_changed(done)
    }

    fn reset_bindings(&mut self) -> Note {
        let file = self.paths.controls_file();
        let done = self.bindings.reset_saved(&file);
        self.bindings_changed(done)
    }
}

const LAYOUT_NOTE: &str = if cfg!(windows) {
    "A wheel steers with its wheel and pedals, and its paddles shift. Luigi's Mansion \
     Arcade: the left stick moves the crosshair, the right trigger or A vacuums, B is the \
     gun's button."
} else {
    "A wheel steers with its wheel and pedals, and its paddles shift. Luigi's Mansion \
     Arcade: the left stick (or a mouse, or a lightgun) moves the crosshair, the right \
     trigger, A or the left mouse button vacuums, B is the gun's button."
};

fn controls_view(s: &Snapshot, bindings: &Bindings<Button>) -> ControlsView {
    let c = s.controls;
    ControlsView {
        error: s.error.clone(),
        devices: s
            .devices
            .iter()
            .map(|d| Device {
                name: d.name.clone(),
                kind: d.kind,
            })
            .collect(),
        driver: s.driver,
        layout: bindings.layout(),
        layout_note: LAYOUT_NOTE.into(),
        steer: c.steer,
        accel: c.accel,
        brake: c.brake,
        buttons: Button::ALL
            .iter()
            .map(|b| ButtonView {
                key: b.key(),
                label: b.label().into(),
                pressed: c.is_pressed(*b),
            })
            .collect(),
        feeding: s.feeding,
        lines: Vec::new(),
    }
}

// ---------------------------------------------------------------- settings

/// Output sizes offered in the window; any other saved size is kept.
const OUTPUTS: [Size; 3] = [
    Size::new(1920, 1080),
    Size::new(2560, 1440),
    Size::new(3840, 2160),
];

/// Render sizes (`None` = the cabinet's own size).
const RENDERS: [Option<Size>; 4] = [
    None,
    Some(Size::new(1920, 1080)),
    Some(Size::new(2560, 1440)),
    Some(Size::new(3840, 2160)),
];

const SHARPNESS_OFF: &str = "Only FSR 1 and NVIDIA Image Scaling use sharpness.";

fn area(s: &Size) -> u64 {
    u64::from(s.w) * u64::from(s.h)
}

/// The curated output sizes plus the current one, smallest first.
fn output_choices(current: Size) -> Vec<Size> {
    let mut v = OUTPUTS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v.sort_by_key(area);
    }
    v
}

/// The curated render sizes plus the current one.
fn render_choices(current: Option<Size>) -> Vec<Option<Size>> {
    let mut v = RENDERS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v[1..].sort_by_key(|s| s.as_ref().map(area));
    }
    v
}

/// A game's settings page.
#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    pub settings: Settings,
    /// The cabinet's own screen size, for "native" and the looks.
    native: Size,
    /// The player's screen, for the looks.
    screen: Option<(u32, u32)>,
}

impl Form {
    pub fn new(settings: Settings, native: Size, screen: Option<(u32, u32)>) -> Form {
        Form {
            settings,
            native,
            screen,
        }
    }
}

impl HubForm for Form {
    fn value(&self, key: &str) -> String {
        let s = &self.settings;
        match key {
            "look" => s.look.map_or("Custom".into(), |l| l.label().to_string()),
            "render" => match s.render {
                None => format!("Native, {}", hub::size_name(self.native)),
                Some(z) => hub::size_name(z),
            },
            "output" => hub::size_name(s.output),
            "upscaler" => s.upscaler.label().to_string(),
            "sharpness" => s.sharpness.to_string(),
            "fullscreen" => if s.fullscreen { "On" } else { "Off" }.to_string(),
            "deadzone" => format!("{:.0}%", s.deadzone * 100.0),
            _ => String::new(),
        }
    }

    fn disabled(&self, key: &str) -> Option<String> {
        (key == "sharpness" && !self.settings.upscaler.sharpens()).then(|| SHARPNESS_OFF.into())
    }

    fn adjust(&mut self, key: &str, d: i32, wrap: bool) -> bool {
        if self.disabled(key).is_some() || d == 0 {
            return false;
        }
        let before = self.settings.clone();
        let s = &mut self.settings;
        match key {
            // from Custom, the look for this screen first
            "look" => match s.look {
                Some(cur) => hub::pick(&Look::ALL, cur, d, wrap),
                None => Look::recommended(self.screen),
            }
            .apply(s, self.native, self.screen),
            "render" => s.render = hub::pick(&render_choices(s.render), s.render, d, wrap),
            "output" => s.output = hub::pick(&output_choices(s.output), s.output, d, wrap),
            "upscaler" => s.upscaler = hub::pick(&Upscaler::ALL, s.upscaler, d, wrap),
            "fullscreen" => s.fullscreen = !s.fullscreen,
            "sharpness" => s.sharpness = (i32::from(s.sharpness) + d).clamp(0, 20) as u8,
            "deadzone" => {
                let pct = (s.deadzone * 100.0).round() as i32 + d;
                s.deadzone = pct.clamp(0, 30) as f32 / 100.0;
            }
            _ => {}
        }
        if settings::PICTURE.contains(&key) && self.settings != before {
            self.settings.look = None;
        }
        self.settings != before
    }

    fn save(&self, file: &Path) -> io::Result<()> {
        settings::write(file, &self.settings)
    }
}

#[cfg(test)]
mod tests;
