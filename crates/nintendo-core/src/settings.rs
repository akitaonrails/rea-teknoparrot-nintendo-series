//! Per-game settings, stored as `key = value` lines in
//! `<data>/settings/<slug>.cfg`.
//!
//! [`OPTIONS`] names every setting once (`rea_launcher::cfg` option specs):
//! its key, its flag, the values it takes, and the label and description
//! the window and `--help` show. The file parser, the command line,
//! `--help`, the template and the window rows all come from it. A save
//! writes only the keys that changed, keeping comments, unknown keys and
//! lines the launcher couldn't read as the player wrote them.

use std::io;
use std::path::Path;

use rea_launcher::cfg::{self, Kind, OptionSpec, Rejected, Resolution, Value};
use rea_launcher::gamescope::{CRTS, Crt, SIZE_MAX, SIZE_MIN, Size, UPSCALERS, Upscaler};

/// `render = native`: the cabinet's own size.
const NATIVE: &cfg::Aliases = &[("native", Resolution::Original)];

/// Every per-game setting. `crt` has no window row (command line only
/// until it was checked on a real screen); the gamescope rows are Linux's
/// (shown off on Windows, where the games draw full screen themselves).
pub static OPTIONS: &[OptionSpec] = &[
    OptionSpec::new(
        "render",
        Kind::Size {
            min: SIZE_MIN,
            max: SIZE_MAX,
            aliases: NATIVE,
        },
        "Game render size",
        "The size the game draws at. Bigger is sharper and needs a stronger GPU.",
    )
    .with_placeholder("<WxH|native>"),
    OptionSpec::new(
        "output",
        Kind::Size {
            min: SIZE_MIN,
            max: SIZE_MAX,
            aliases: cfg::SIZES_ONLY,
        },
        "Output size",
        "The size of the picture on your screen. Pick your screen's size.",
    )
    .helper_owned(),
    OptionSpec::new(
        "upscaler",
        Kind::Choice(UPSCALERS),
        "Upscaler",
        "How the game's picture is stretched to the output size.",
    )
    .helper_owned(),
    OptionSpec::new(
        "sharpness",
        Kind::Range { min: 0, max: 20 },
        "Sharpness",
        "How crisp the upscaler makes edges. 0 is the sharpest.",
    )
    .helper_owned(),
    OptionSpec::new(
        "crt",
        Kind::Choice(CRTS),
        "CRT look",
        "Scanlines and a shadow mask drawn over the game.",
    )
    .launcher_only()
    .helper_owned(),
    OptionSpec::new(
        "fullscreen",
        Kind::Toggle,
        "Fullscreen",
        "Fill the whole screen, or play in a window.",
    )
    .helper_owned(),
    // a fraction of the stick's travel, stored with two places: 0.05
    OptionSpec::new(
        "deadzone",
        Kind::Decimal {
            min: 0,
            max: 30,
            places: 2,
        },
        "Dead zone",
        "How far a controller stick moves before the car starts to steer.",
    ),
];

/// Comment lines at the top of a new settings file.
pub const HEADING: &str = "Settings for one game of the Nintendo arcade launcher.\n\
    Every key is also a command-line flag with --set <game> or --play <game>: \
    render = 1920x1080 is --render 1920x1080.";

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The size the game itself draws at; `None`: the cabinet's own.
    pub render: Option<Size>,
    /// What gamescope shows on the screen.
    pub output: Size,
    pub upscaler: Upscaler,
    /// 0–20, gamescope's FSR/NIS sharpness (0 = sharpest).
    pub sharpness: u8,
    pub crt: Crt,
    pub fullscreen: bool,
    /// Pad stick dead zone around the centre, 0.0–0.3.
    pub deadzone: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            render: None,
            output: Size::new(3840, 2160),
            upscaler: Upscaler::Fsr,
            sharpness: 5,
            crt: Crt::Off,
            fullscreen: true,
            deadzone: 0.05,
        }
    }
}

impl Settings {
    /// Takes one checked value (from the file or a flag).
    pub fn apply(&mut self, key: &str, value: Value) -> Result<(), Rejected> {
        match (key, value) {
            ("render", Value::Size(Resolution::Size(w, h))) => self.render = Some(Size::new(w, h)),
            ("render", Value::Size(_)) => self.render = None,
            ("output", Value::Size(Resolution::Size(w, h))) => self.output = Size::new(w, h),
            ("upscaler", Value::Choice(v)) => {
                self.upscaler = Upscaler::parse(v).ok_or(Rejected::Unknown)?;
            }
            ("sharpness", Value::Number(n)) => self.sharpness = n as u8,
            ("crt", Value::Choice(v)) => self.crt = Crt::parse(v).ok_or(Rejected::Unknown)?,
            ("fullscreen", Value::Toggle(b)) => self.fullscreen = b,
            ("deadzone", Value::Decimal(d)) => self.deadzone = d.as_f32(),
            _ => return Err(Rejected::Unknown),
        }
        Ok(())
    }

    /// Reads a settings file; bad values fall back to the default and are
    /// reported.
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut s = Settings::default();
        let problems = cfg::spec::parse_file(text, "settings", OPTIONS, |k, v| s.apply(k, v));
        (s, problems)
    }

    /// The file value of `key`.
    pub fn value(&self, key: &str) -> Option<String> {
        Some(match key {
            "render" => self.render.map_or("native".into(), |z| z.to_string()),
            "output" => self.output.to_string(),
            "upscaler" => self.upscaler.key().into(),
            "sharpness" => self.sharpness.to_string(),
            "crt" => self.crt.key().into(),
            "fullscreen" => self.fullscreen.to_string(),
            "deadzone" => format!("{:.2}", self.deadzone),
            _ => return None,
        })
    }

    /// Rewrites `existing` with these settings, keeping comments and
    /// unknown keys in place: a key is written when its value differs from
    /// what `existing` says, or `existing` doesn't have it. A line the
    /// launcher couldn't read (`fullscreen = off # mine`) stays as the
    /// player wrote it unless that setting is changed. A new file starts
    /// from the commented template.
    pub fn save(&self, existing: &str) -> String {
        let new = existing.trim().is_empty();
        let (was, _) = Settings::parse(existing);
        let mut out = if new {
            template()
        } else {
            existing.to_string()
        };
        for spec in OPTIONS {
            let Some(now) = self.value(spec.key) else {
                continue;
            };
            let has_line = cfg::entries(existing).any(|e| spec.answers_to(e.key));
            if new || !has_line || Some(&now) != was.value(spec.key).as_ref() {
                out = cfg::spec::set_key(&out, spec, &now);
            }
        }
        out
    }
}

/// A game's settings file and the lines it couldn't read (`None`: no file
/// yet). An unreadable file is an error, never defaults a later save would
/// write over it.
pub fn read(file: &Path) -> io::Result<Option<(Settings, Vec<String>)>> {
    Ok(cfg::read_file(file)?.map(|text| Settings::parse(&text)))
}

/// A game's settings and the lines its file couldn't read; defaults when
/// there is no file yet.
pub fn load(file: &Path) -> io::Result<(Settings, Vec<String>)> {
    Ok(read(file)?.unwrap_or_default())
}

/// Writes `s` into `file` ([`Settings::save`]) as a whole-file swap; a
/// file that can't be read is left alone and the error returned.
pub fn write(file: &Path, s: &Settings) -> io::Result<()> {
    let existing = cfg::read_file(file)?.unwrap_or_default();
    rea_launcher::fsx::write_atomic(file, s.save(&existing))
}

/// The template a new settings file starts from.
pub fn template() -> String {
    cfg::spec::template(HEADING, OPTIONS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_comments_and_unknown_keys() {
        let text = "# my notes\nupscaler = nis\nfuture_key = 7\noutput=2560x1440\ncrt = lottes\n";
        let (s, problems) = Settings::parse(text);
        assert_eq!(problems.len(), 1, "the unknown key, kept: {problems:?}");
        assert_eq!(s.upscaler, Upscaler::Nis);
        assert_eq!(s.output, Size::new(2560, 1440));
        assert_eq!(s.crt, Crt::Lottes);
        let saved = s.save(text);
        assert!(saved.starts_with(text), "lines kept as written: {saved}");
        assert!(saved.contains("\nrender = native\n"), "missing keys added");
        assert_eq!(Settings::parse(&saved).0, s);
        assert_eq!(s.save(&saved), saved, "nothing changed, nothing written");
    }

    #[test]
    fn bad_values_fall_back_and_are_reported() {
        let (s, p) = Settings::parse(
            "output = 99x99\nsharpness = 50\nfullscreen = maybe\ncrt = rainbow\ndeadzone = 0.9\n",
        );
        assert_eq!(s, Settings::default());
        assert_eq!(p.len(), 5, "{p:?}");
        assert!(p[4].contains("deadzone needs 0.00 to 0.30"), "{p:?}");
    }

    /// A line the launcher couldn't read stays as the player wrote it
    /// when another setting is saved: it was replaced by the default.
    #[test]
    fn a_bad_line_is_never_replaced_by_the_default() {
        let text = "fullscreen = off # mine\nsharpness = 5\n";
        let (mut s, problems) = Settings::parse(text);
        assert_eq!(problems.len(), 1);
        assert!(s.fullscreen, "the default, for this run");
        s.sharpness = 9;
        assert!(
            s.save(text)
                .starts_with("fullscreen = off # mine\nsharpness = 9\n")
        );
    }

    #[test]
    fn a_byte_order_mark_hides_no_key() {
        let text = "\u{feff}render = 1920x1080\n";
        let (s, problems) = Settings::parse(text);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(s.render, Some(Size::new(1920, 1080)));
        let saved = s.save(text);
        assert!(saved.starts_with(text), "the BOM kept: {saved:?}");
        assert_eq!(saved.matches("render =").count(), 1, "no duplicate key");
    }

    /// An unreadable file is an error, and a save leaves it alone (it was
    /// read as empty and overwritten with defaults).
    #[test]
    fn an_unreadable_file_is_never_overwritten() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("mkdx.cfg");
        std::fs::create_dir(&file).unwrap();
        assert!(load(&file).is_err());
        assert!(write(&file, &Settings::default()).is_err());
        assert!(file.is_dir());
        // Latin-1 is read, not refused
        let file = t.path().join("fzax.cfg");
        std::fs::write(&file, b"# Jo\xe3o\nsharpness = 3\n").unwrap();
        assert_eq!(load(&file).unwrap().0.sharpness, 3);
    }

    #[test]
    fn a_new_file_is_the_template_filled_in() {
        let text = Settings::default().save("");
        assert!(text.starts_with("# Settings for one game"), "{text}");
        assert!(text.contains("\nrender = native\n") && text.contains("\ndeadzone = 0.05\n"));
        assert_eq!(Settings::parse(&text), (Settings::default(), vec![]));
    }

    #[test]
    fn every_option_reads_its_stored_format() {
        let mut s = Settings::default();
        for (key, value) in [
            ("render", "2560x1440"),
            ("output", "1920x1080"),
            ("upscaler", "NIS"),
            ("sharpness", "20"),
            ("crt", "lottes"),
            ("fullscreen", "false"),
            ("deadzone", "0.3"),
        ] {
            let spec = cfg::spec::find(OPTIONS, key).unwrap();
            s.apply(key, spec.parse(value).unwrap()).unwrap();
        }
        assert_eq!(
            s,
            Settings {
                render: Some(Size::new(2560, 1440)),
                output: Size::new(1920, 1080),
                upscaler: Upscaler::Nis,
                sharpness: 20,
                crt: Crt::Lottes,
                fullscreen: false,
                deadzone: 0.3,
            }
        );
        assert_eq!(s.value("fullscreen").as_deref(), Some("false"));
        assert_eq!(s.value("deadzone").as_deref(), Some("0.30"));
    }
}
