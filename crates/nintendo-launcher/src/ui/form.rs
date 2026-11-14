//! A game's settings page as data: its rows, what each shows, why one is
//! off, and how the arrows change the [`Settings`] behind it. The `crt`
//! key is not offered here (command line only until it is checked on a
//! real screen); saving keeps whatever the file says for it.

use std::path::Path;

use nintendo_core::catalog::Host;
use nintendo_core::settings::{Settings, Size, Upscaler};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Render,
    Output,
    Upscaler,
    Sharpness,
    Fullscreen,
    Deadzone,
}

impl Row {
    pub const ALL: [Row; 6] = [
        Row::Render,
        Row::Output,
        Row::Upscaler,
        Row::Sharpness,
        Row::Fullscreen,
        Row::Deadzone,
    ];

    /// The rows the settings window shows on `host`: Windows has no
    /// gamescope, so only what the games themselves take (render size)
    /// and the pad's dead zone.
    pub fn shown(host: Host) -> Vec<Row> {
        match host {
            Host::Linux => Row::ALL.to_vec(),
            Host::Windows => vec![Row::Render, Row::Deadzone],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Row::Render => "Game render size",
            Row::Output => "Output size",
            Row::Upscaler => "Upscaler",
            Row::Sharpness => "Sharpness",
            Row::Fullscreen => "Fullscreen",
            Row::Deadzone => "Dead zone",
        }
    }

    /// The settings key (and command-line option `--<key>`) of this row.
    #[cfg(test)]
    pub fn key(self) -> &'static str {
        match self {
            Row::Render => "render",
            Row::Output => "output",
            Row::Upscaler => "upscaler",
            Row::Sharpness => "sharpness",
            Row::Fullscreen => "fullscreen",
            Row::Deadzone => "deadzone",
        }
    }

    #[cfg(test)]
    pub fn from_key(key: &str) -> Option<Row> {
        Row::ALL.into_iter().find(|r| r.key() == key)
    }

    pub fn description(self) -> &'static str {
        match self {
            Row::Render => {
                "The size the game draws at. Bigger is sharper and needs a stronger GPU."
            }
            Row::Output => "The size of the picture on your screen. Pick your screen's size.",
            Row::Upscaler => "How the game's picture is stretched to the output size.",
            Row::Sharpness => "How crisp the upscaler makes edges. 0 is the sharpest.",
            Row::Fullscreen => "Fill the whole screen, or play in a window.",
            Row::Deadzone => "How far a controller stick moves before the car starts to steer.",
        }
    }
}

/// Output sizes offered in the window; any other saved size is kept.
pub const OUTPUTS: [Size; 3] = [
    Size::new(1920, 1080),
    Size::new(2560, 1440),
    Size::new(3840, 2160),
];

/// Render sizes (`None` = the cabinet's own size).
pub const RENDERS: [Option<Size>; 4] = [
    None,
    Some(Size::new(1920, 1080)),
    Some(Size::new(2560, 1440)),
    Some(Size::new(3840, 2160)),
];

const SHARPNESS_OFF: &str = "Only FSR 1 and NVIDIA Image Scaling use sharpness.";

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    pub settings: Settings,
    /// The cabinet's own screen size, for "native".
    pub native: Size,
}

pub fn size_name(s: Size) -> String {
    let tag = match (s.w, s.h) {
        (1920, 1080) => " (1080p)",
        (2560, 1440) => " (1440p)",
        (3840, 2160) => " (4K)",
        _ => "",
    };
    format!("{} × {}{tag}", s.w, s.h)
}

fn area(s: &Size) -> u64 {
    u64::from(s.w) * u64::from(s.h)
}

/// The curated output sizes plus the current one, smallest first.
pub fn output_choices(current: Size) -> Vec<Size> {
    let mut v = OUTPUTS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v.sort_by_key(area);
    }
    v
}

/// The curated render sizes plus the current one.
pub fn render_choices(current: Option<Size>) -> Vec<Option<Size>> {
    let mut v = RENDERS.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v[1..].sort_by_key(|s| s.as_ref().map(area));
    }
    v
}

/// The next item in `all` after `cur`, `d` steps away, clamped at the ends.
fn step<T: Copy + PartialEq>(all: &[T], cur: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    all[(i + d).clamp(0, all.len() as i32 - 1) as usize]
}

/// [`step`], wrapping around at the ends when `wrap`.
fn pick<T: Copy + PartialEq>(all: &[T], cur: T, d: i32, wrap: bool) -> T {
    let next = step(all, cur, d);
    match (wrap && next == cur, d > 0) {
        (false, _) => next,
        (true, true) => all[0],
        (true, false) => all[all.len() - 1],
    }
}

impl Form {
    pub fn new(settings: Settings, native: Size) -> Self {
        Self { settings, native }
    }

    /// Why a row can't be changed now, if it can't.
    pub fn disabled(&self, row: Row) -> Option<&'static str> {
        match row {
            Row::Sharpness if !matches!(self.settings.upscaler, Upscaler::Fsr | Upscaler::Nis) => {
                Some(SHARPNESS_OFF)
            }
            _ => None,
        }
    }

    pub fn value(&self, row: Row) -> String {
        let s = &self.settings;
        match row {
            Row::Render => match s.render {
                None => format!("Native, {}", size_name(self.native)),
                Some(z) => size_name(z),
            },
            Row::Output => size_name(s.output),
            Row::Upscaler => s.upscaler.label().to_string(),
            Row::Sharpness => s.sharpness.to_string(),
            Row::Fullscreen => if s.fullscreen { "On" } else { "Off" }.to_string(),
            Row::Deadzone => format!("{:.0}%", s.deadzone * 100.0),
        }
    }

    /// Move a row's value `d` steps (left = -1, right = +1, Enter = +1 with
    /// wrap for lists). Returns whether anything changed.
    pub fn adjust(&mut self, row: Row, d: i32, wrap: bool) -> bool {
        if self.disabled(row).is_some() || d == 0 {
            return false;
        }
        let before = self.settings.clone();
        let s = &mut self.settings;
        match row {
            Row::Render => s.render = pick(&render_choices(s.render), s.render, d, wrap),
            Row::Output => s.output = pick(&output_choices(s.output), s.output, d, wrap),
            Row::Upscaler => s.upscaler = pick(&Upscaler::ALL, s.upscaler, d, wrap),
            Row::Fullscreen => s.fullscreen = !s.fullscreen,
            Row::Sharpness => s.sharpness = (i32::from(s.sharpness) + d).clamp(0, 20) as u8,
            Row::Deadzone => {
                let pct = (s.deadzone * 100.0).round() as i32 + d;
                s.deadzone = pct.clamp(0, 30) as f32 / 100.0;
            }
        }
        self.settings != before
    }
}

/// Write `settings` into the file at `path`, keeping the player's comments
/// and unknown keys; atomically (temporary file, then rename).
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("cfg.tmp");
    std::fs::write(&tmp, settings.save(&existing))?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> Form {
        Form::new(Settings::default(), Size::new(1360, 768))
    }

    #[test]
    fn every_row_has_a_settings_key_and_crt_is_not_one() {
        for row in Row::ALL {
            assert_eq!(Row::from_key(row.key()), Some(row));
            assert!(nintendo_core::settings::KEYS.contains(&row.key()));
        }
        assert_eq!(Row::from_key("crt"), None);
        assert_eq!(Row::ALL.len(), nintendo_core::settings::KEYS.len() - 1);
    }

    #[test]
    fn output_steps_through_the_curated_sizes_and_keeps_a_custom_one() {
        let mut f = form();
        assert_eq!(f.settings.output, Size::new(3840, 2160));
        assert!(!f.adjust(Row::Output, 1, false), "clamped at the end");
        assert!(f.adjust(Row::Output, -1, false));
        assert_eq!(f.settings.output, Size::new(2560, 1440));
        assert!(f.adjust(Row::Output, 1, true));
        assert!(f.adjust(Row::Output, 1, true), "Enter wraps around");
        assert_eq!(f.settings.output, Size::new(1920, 1080));

        let custom = Size::new(3440, 1440);
        assert_eq!(
            output_choices(custom),
            vec![
                Size::new(1920, 1080),
                Size::new(2560, 1440),
                custom,
                Size::new(3840, 2160)
            ]
        );
        let mut f = form();
        f.settings.output = custom;
        assert_eq!(f.value(Row::Output), "3440 × 1440");
        f.adjust(Row::Output, 1, false);
        assert_eq!(f.settings.output, Size::new(3840, 2160));
    }

    #[test]
    fn render_size_starts_native_and_steps_up() {
        let mut f = form();
        assert_eq!(f.value(Row::Render), "Native, 1360 × 768");
        f.adjust(Row::Render, 1, false);
        assert_eq!(f.settings.render, Some(Size::new(1920, 1080)));
        f.adjust(Row::Render, 1, false);
        f.adjust(Row::Render, 1, false);
        f.adjust(Row::Render, 1, false);
        assert_eq!(f.settings.render, Some(Size::new(3840, 2160)));
        assert_eq!(f.value(Row::Render), "3840 × 2160 (4K)");
        f.adjust(Row::Render, -9, false);
        assert_eq!(f.settings.render, None);
        assert!(f.adjust(Row::Render, -1, true), "Enter wraps back");
        assert_eq!(f.settings.render, Some(Size::new(3840, 2160)));
    }

    #[test]
    fn sharpness_is_off_unless_fsr_or_nis() {
        let mut f = form();
        assert!(f.disabled(Row::Sharpness).is_none());
        assert!(f.adjust(Row::Sharpness, 1, false));
        assert_eq!(f.settings.sharpness, 6);
        // FSR -> NIS -> Smooth
        f.adjust(Row::Upscaler, 1, false);
        assert_eq!(f.settings.upscaler, Upscaler::Nis);
        f.adjust(Row::Upscaler, 1, false);
        assert_eq!(f.settings.upscaler, Upscaler::Linear);
        assert_eq!(f.value(Row::Upscaler), "Smooth");
        assert!(f.disabled(Row::Sharpness).is_some());
        assert!(!f.adjust(Row::Sharpness, 1, false));
        assert_eq!(f.settings.sharpness, 6);
    }

    #[test]
    fn sharpness_and_dead_zone_clamp() {
        let mut f = form();
        for _ in 0..40 {
            f.adjust(Row::Sharpness, 1, false);
            f.adjust(Row::Deadzone, 1, false);
        }
        assert_eq!(f.settings.sharpness, 20);
        assert!((f.settings.deadzone - 0.30).abs() < 1e-6);
        assert_eq!(f.value(Row::Deadzone), "30%");
        for _ in 0..40 {
            f.adjust(Row::Deadzone, -1, false);
        }
        assert_eq!(f.settings.deadzone, 0.0);
    }

    #[test]
    fn fullscreen_toggles() {
        let mut f = form();
        assert_eq!(f.value(Row::Fullscreen), "On");
        assert!(f.adjust(Row::Fullscreen, 1, false));
        assert_eq!(f.value(Row::Fullscreen), "Off");
    }

    #[test]
    fn every_row_has_words_and_disabled_rows_say_why() {
        let f = form();
        for row in Row::ALL {
            assert!(!row.label().is_empty() && row.description().ends_with('.'));
            assert!(!f.value(row).is_empty());
        }
        assert!(SHARPNESS_OFF.ends_with('.'));
    }

    #[test]
    fn saving_keeps_notes_unknown_keys_and_crt() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("settings/id6.cfg");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# my notes\nfuture = 1\ncrt = lottes\n").unwrap();
        let mut f = Form::new(
            Settings::parse(&std::fs::read_to_string(&path).unwrap()).0,
            Size::new(1360, 768),
        );
        f.adjust(Row::Output, -2, false);
        f.adjust(Row::Fullscreen, 1, false);
        save(&path, &f.settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("# my notes\nfuture = 1\ncrt = lottes\n"),
            "{text}"
        );
        let (back, problems) = Settings::parse(&text);
        assert!(problems.is_empty());
        assert_eq!(back, f.settings);
        assert_eq!(back.output, Size::new(1920, 1080));
        assert!(!back.fullscreen);
        assert!(!path.with_extension("cfg.tmp").exists());
    }

    #[test]
    fn windows_shows_only_what_the_games_themselves_take() {
        assert_eq!(Row::shown(Host::Linux), Row::ALL.to_vec());
        assert_eq!(Row::shown(Host::Windows), vec![Row::Render, Row::Deadzone]);
    }
}
