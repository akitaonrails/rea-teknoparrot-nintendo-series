//! What to write and what to run for a game, as plain data.
//!
//! [`plan`] turns a game, its folders and the settings into the files the
//! launcher writes and the process it starts, so tests can check the whole
//! wiring (setting → file / argument / environment) without starting
//! anything.
//!
//! The Triforce games run in Dolphin with a user folder inside the game's
//! runtime (`-u`): its configuration is written before every start, and the
//! Triforce saves (`Triforce/`, the magnetic card of Mario Kart GP 2) stay
//! there. On Linux the launcher drives Dolphin's emulated pad through a named
//! pipe (Dolphin's `Pipe` input backend), the way it drives
//! windowsarcadeloader: its own SDL input owns the wheel or pad. Dolphin's
//! Windows build has no pipes, so there it reads an XInput pad, a wheel
//! through its SDL backend, and the keyboard itself.

use std::path::{Path, PathBuf};

use crate::catalog::{Game, Host, Patch, Route};
use crate::input::{DolphinPad, StickLayout};
use crate::settings::{Crt, Settings, Size};

/// A process to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

/// How the launcher's input reaches the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feed {
    /// windowsarcadeloader's `--input-pipe`: the loader's stdin.
    Wal(StickLayout),
    /// A named pipe Dolphin reads as a pad (Linux).
    DolphinPipe(PathBuf),
    /// The game reads the devices itself (Dolphin on Windows).
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Files written (replaced) before every launch.
    pub files: Vec<(PathBuf, String)>,
    /// Symlinks made before every launch (`at` → `target`).
    pub links: Vec<(PathBuf, PathBuf)>,
    pub game: Command,
    /// Arguments for gamescope, which wraps the game.
    pub gamescope: Vec<String>,
    pub feed: Feed,
    /// The size the game draws at.
    pub game_size: Size,
    /// The `HOME` gamescope runs with, when it loads a ReShade effect:
    /// gamescope looks effects up by name only, under
    /// `$HOME/.local/share/gamescope/reshade/Shaders`, so the runtime holds
    /// such a folder with a link to the launcher's shader. The game gets the
    /// player's own `HOME` back.
    pub gamescope_home: Option<PathBuf>,
}

type Files = Vec<(PathBuf, String)>;

/// The CRT effect's file name, in the components' `shaders/` and in
/// gamescope's effect folder.
const CRT_EFFECT: &str = "crt-lottes.fx";

/// The pipe's name in Dolphin's `Pipes/` folder; Dolphin calls the device
/// `Pipe/0/<name>`.
pub const PIPE_NAME: &str = "launcher";

/// The launcher's own components.
#[derive(Debug, Clone)]
pub struct Components {
    /// Linux: Dolphin built from source (`dolphin-emu-nogui`, `Sys/`, `lib/`).
    pub dolphin: PathBuf,
    /// Windows: Dolphin's release (`Dolphin.exe`, `Sys/`).
    pub dolphin_win: PathBuf,
    /// windowsarcadeloader: `arcade-launcher`, `payloads/`, `systemprofiles/`.
    pub wal: PathBuf,
    /// The Wine runner windowsarcadeloader runs games with (`bin/wine`).
    pub runner: PathBuf,
    /// The winetricks script windowsarcadeloader installs runtimes with.
    pub winetricks: PathBuf,
    /// ReShade effects (`crt-lottes.fx`).
    pub shaders: PathBuf,
}

/// Where the launcher keeps per-game state.
#[derive(Debug, Clone)]
pub struct Places {
    /// The game's runtime folder.
    pub runtime: PathBuf,
    /// windowsarcadeloader's root (shared by every Windows game: prefix, links).
    pub wal_root: PathBuf,
    /// The Triforce test-menu program (`segaboot.gcm`), when the player has it.
    pub segaboot: Option<PathBuf>,
}

/// A wheel for Dolphin's own SDL backend (Windows): its SDL name and the
/// raw axes the launcher's wheel profile found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DolphinWheel {
    pub name: String,
    pub steer_axis: usize,
    /// Axis, raw value at rest, raw value floored.
    pub gas: Option<(usize, i16, i16)>,
    pub brake: Option<(usize, i16, i16)>,
}

/// Dolphin's internal resolution (a multiple of the 640×480 picture) for a
/// render size: the smallest that reaches its height, 1 to 8.
pub fn dolphin_scale(render: Option<Size>) -> u32 {
    render.map_or(1, |r| r.h.div_ceil(480).clamp(1, 8))
}

pub fn game_size(game: &Game, s: &Settings) -> Size {
    match game.route {
        // a 4:3 picture, 480 lines times the internal resolution
        Route::Dolphin => {
            let h = 480 * dolphin_scale(s.render);
            Size::new(h * 4 / 3, h)
        }
        Route::Wal { .. } => s.render.unwrap_or(game.native),
    }
}

fn gamescope_args(s: &Settings, size: Size, headless: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "-w",
        &size.w.to_string(),
        "-h",
        &size.h.to_string(),
        "-W",
        &s.output.w.to_string(),
        "-H",
        &s.output.h.to_string(),
        "-r",
        "60",
        "-F",
        if headless { "linear" } else { s.upscaler.key() },
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();
    if !headless && matches!(s.upscaler.key(), "fsr" | "nis") {
        a.extend(["--sharpness".into(), s.sharpness.to_string()]);
    }
    if s.crt == Crt::Lottes {
        a.extend(["--reshade-effect".into(), CRT_EFFECT.into()]);
    }
    if headless {
        a.extend(["--backend".into(), "headless".into()]);
    } else if s.fullscreen {
        a.push("-f".into());
    }
    a
}

/// Dolphin's main settings: the Triforce baseboard on the first serial port
/// and SP1 (Dolphin also sets these for a Triforce disc), no dialogs, no
/// network services, full screen.
pub fn dolphin_ini(host: Host, cheats: bool) -> String {
    let cheats = if cheats { "True" } else { "False" };
    let backend = match host {
        Host::Linux => "Vulkan",
        Host::Windows => "D3D11",
    };
    format!(
        "# Written by the launcher before every start.\n\
         [Core]\nSIDevice0 = 11\nSIDevice1 = 0\nSIDevice2 = 0\nSIDevice3 = 0\nSerialPort1 = 6\n\
         SkipIPL = True\nGFXBackend = {backend}\nEnableCheats = {cheats}\n\
         [Display]\nFullscreen = True\nRenderToMain = False\nKeepWindowOnTop = True\n\
         [Interface]\nUsePanicHandlers = False\nConfirmStop = False\nOnScreenDisplayMessages = True\n\
         PauseOnFocusLost = False\nShowActiveTitle = False\n\
         [Analytics]\nEnabled = False\nPermissionAsked = True\n\
         [AutoUpdate]\nUpdateTrack = \n\
         [NetPlay]\nUseUPNP = False\n"
    )
}

/// A game's settings file: its Gecko codes and the shipped Action Replay
/// codes it needs, all enabled.
pub fn game_settings_ini(patches: &[Patch]) -> String {
    let mut gecko = String::new();
    let mut gecko_on = String::new();
    let mut ar_on = String::new();
    for p in patches {
        match p {
            Patch::Gecko { name, lines } => {
                gecko.push_str(&format!("${name}\n"));
                for l in *lines {
                    gecko.push_str(l);
                    gecko.push('\n');
                }
                gecko_on.push_str(&format!("${name}\n"));
            }
            Patch::ShippedActionReplay(name) => ar_on.push_str(&format!("${name}\n")),
        }
    }
    let mut s = String::from("# Written by the launcher before every start.\n");
    if !gecko.is_empty() {
        s.push_str(&format!("[Gecko]\n{gecko}[Gecko_Enabled]\n{gecko_on}"));
    }
    if !ar_on.is_empty() {
        s.push_str(&format!("[ActionReplay_Enabled]\n{ar_on}"));
    }
    s
}

/// Dolphin's graphics settings: the internal resolution, v-sync, and
/// shaders compiled before the game starts (no stutter in the first race).
pub fn gfx_ini(scale: u32) -> String {
    format!(
        "# Written by the launcher before every start.\n\
         [Hardware]\nVSync = True\n\
         [Settings]\nInternalResolution = {scale}\nShaderCompilationMode = 0\n\
         WaitForShadersBeforeStarting = True\nShowFPS = False\n"
    )
}

/// One `Group/Control = expression` line per mapped input of `[GCPad1]`.
fn pad_section(device: &str, lines: &[(&str, String)]) -> String {
    let mut s = format!(
        "# Written by the launcher before every start.\n[GCPad1]\nDevice = {device}\n\
         Main Stick/Gate Size = 100.0\nMain Stick/Dead Zone = 0.0\n\
         Triggers/Threshold = 90.0\nOptions/Always Connected = True\n"
    );
    for (k, v) in lines {
        s.push_str(&format!("{k} = {v}\n"));
    }
    s.push_str("[GCPad2]\nDevice = \n[GCPad3]\nDevice = \n[GCPad4]\nDevice = \n");
    s
}

fn bt(x: &str) -> String {
    format!("`{x}`")
}

/// The pad as Dolphin's pipe device (Linux), with the keyboard of the
/// gamescope X server as a second way in: arrows steer, Up/Down are the
/// pedals, W/Q push/pull the handle (F-Zero AX Monster Ride), Z item/boost,
/// X Mario button/view, A/S paddles, Enter start, 5 coin, 9 test, 0 service. (Dolphin's nogui keeps F1–F11 for itself.)
pub fn gcpad_pipe_ini() -> String {
    let kb = |key: &str| format!("`XInput2/0/Virtual core pointer:{key}`");
    let both = |pipe: &str, key: &str| format!("{} | {}", bt(pipe), kb(key));
    let p = DolphinPad::PIPE;
    let lines: Vec<(&str, String)> = vec![
        ("Buttons/A", both(p.item, "z")),
        ("Buttons/B", both(p.special, "x")),
        ("D-Pad/Right", both(p.view, "x")),
        ("Buttons/X", both(p.paddle_left, "a")),
        ("Buttons/Y", both(p.paddle_right, "s")),
        ("Buttons/Start", both(p.start, "Return")),
        ("Triforce/Coin", both(p.coin, "5")),
        ("Triforce/Test", both(p.test, "9")),
        ("Triforce/Service", both(p.service, "0")),
        ("Main Stick/Left", both("Axis MAIN X -", "Left")),
        ("Main Stick/Right", both("Axis MAIN X +", "Right")),
        ("Main Stick/Up", both("Axis MAIN Y -", "w")),
        ("Main Stick/Down", both("Axis MAIN Y +", "q")),
        ("Triggers/R-Analog", both("Axis R +", "Up")),
        ("Triggers/L-Analog", both("Axis L +", "Down")),
    ];
    pad_section(&format!("Pipe/0/{PIPE_NAME}"), &lines)
}

/// A pedal as a 0..1 expression over Dolphin's half axes, whichever way the
/// pedal travels.
fn pedal_expr(dev: &str, (axis, rest, peak): (usize, i16, i16)) -> String {
    let plus = format!("`{dev}:Axis {axis}+`");
    let minus = format!("`{dev}:Axis {axis}-`");
    if rest.unsigned_abs() < 8000 {
        // rests in the middle: only one half moves
        if peak > rest { plus } else { minus }
    } else if peak > rest {
        format!("(({plus} - {minus}) + 1) / 2")
    } else {
        format!("(({minus} - {plus}) + 1) / 2")
    }
}

/// The pad on Windows: the first XInput pad and the keyboard, and the
/// wheel through Dolphin's SDL backend when the launcher saw one.
pub fn gcpad_windows_ini(wheel: Option<&DolphinWheel>) -> String {
    let x = |c: &str| format!("`XInput/0/Gamepad:{c}`");
    let k = |c: &str| format!("`DInput/0/Keyboard Mouse:{c}`");
    let mut steer_l = format!("{} | {}", x("Left X-"), k("LEFT"));
    let mut steer_r = format!("{} | {}", x("Left X+"), k("RIGHT"));
    let mut gas = format!("{} | {}", x("Trigger R"), k("UP"));
    let mut brake = format!("{} | {}", x("Trigger L"), k("DOWN"));
    if let Some(w) = wheel {
        let dev = format!("SDL/0/{}", w.name);
        steer_l = format!("`{dev}:Axis {}-` | {steer_l}", w.steer_axis);
        steer_r = format!("`{dev}:Axis {}+` | {steer_r}", w.steer_axis);
        if let Some(p) = w.gas {
            gas = format!("{} | {gas}", pedal_expr(&dev, p));
        }
        if let Some(p) = w.brake {
            brake = format!("{} | {brake}", pedal_expr(&dev, p));
        }
    }
    let lines: Vec<(&str, String)> = vec![
        ("Buttons/A", format!("{} | {}", x("Button A"), k("Z"))),
        ("Buttons/B", format!("{} | {}", x("Button B"), k("X"))),
        ("D-Pad/Right", format!("{} | {}", x("Button B"), k("X"))),
        ("Buttons/X", format!("{} | {}", x("Shoulder L"), k("A"))),
        ("Buttons/Y", format!("{} | {}", x("Shoulder R"), k("S"))),
        ("Buttons/Start", format!("{} | {}", x("Start"), k("RETURN"))),
        ("Triforce/Coin", format!("{} | {}", x("Back"), k("5"))),
        ("Triforce/Test", k("9")),
        ("Triforce/Service", k("0")),
        ("Main Stick/Left", steer_l),
        ("Main Stick/Right", steer_r),
        ("Main Stick/Up", format!("{} | {}", x("Left Y+"), k("W"))),
        ("Main Stick/Down", format!("{} | {}", x("Left Y-"), k("Q"))),
        ("Triggers/R-Analog", gas),
        ("Triggers/L-Analog", brake),
    ];
    // the default device is the pad; the others are named in each line
    pad_section("XInput/0/Gamepad", &lines)
}

/// The profile layer the launcher gives windowsarcadeloader: the runner,
/// the launcher's own prefix, no ReShade or gamescope of its own (the
/// launcher wraps the game).
pub fn wal_layer() -> String {
    "# Written by the launcher before every start.\n\
     runner: runner\nprefix: wine-prefix/nintendo\nreshade: false\n\
     gamescope:\n  enabled: false\n"
        .to_string()
}

fn dolphin_plan(
    game: &Game,
    game_file: &Path,
    c: &Components,
    places: &Places,
    s: &Settings,
    host: Host,
    wheel: Option<&DolphinWheel>,
) -> (Command, Files, Vec<(PathBuf, PathBuf)>, Feed) {
    let rt = &places.runtime;
    let user = rt.join("dolphin");
    let config = user.join("Config");
    let mut files = vec![
        (
            config.join("Dolphin.ini"),
            dolphin_ini(host, !game.patches.is_empty()),
        ),
        (config.join("GFX.ini"), gfx_ini(dolphin_scale(s.render))),
    ];
    if !game.patches.is_empty() {
        files.push((
            user.join("GameSettings")
                .join(format!("{}.ini", game.dolphin_id)),
            game_settings_ini(game.patches),
        ));
    }
    let mut links = Vec::new();
    if let Some(sb) = &places.segaboot {
        links.push((user.join("Triforce/segaboot.gcm"), sb.clone()));
    }
    let disc = game_file.to_string_lossy().into_owned();
    let user_arg = user.to_string_lossy().into_owned();
    let (cmd, feed) = match host {
        Host::Linux => {
            files.push((config.join("GCPadNew.ini"), gcpad_pipe_ini()));
            let pipe = user.join("Pipes").join(PIPE_NAME);
            let cmd = Command {
                program: c.dolphin.join("dolphin-emu-nogui"),
                args: vec![
                    "-u".into(),
                    user_arg,
                    "-p".into(),
                    "x11".into(),
                    "-e".into(),
                    disc,
                ],
                env: vec![(
                    "LD_LIBRARY_PATH".into(),
                    c.dolphin.join("lib").to_string_lossy().into_owned(),
                )],
                cwd: rt.to_path_buf(),
            };
            (cmd, Feed::DolphinPipe(pipe))
        }
        Host::Windows => {
            files.push((config.join("GCPadNew.ini"), gcpad_windows_ini(wheel)));
            let cmd = Command {
                program: c.dolphin_win.join("Dolphin.exe"),
                args: vec!["-u".into(), user_arg, "-b".into(), "-e".into(), disc],
                env: vec![],
                cwd: rt.to_path_buf(),
            };
            (cmd, Feed::None)
        }
    };
    (cmd, files, links, feed)
}

#[allow(clippy::too_many_arguments)]
pub fn plan(
    game: &Game,
    game_file: &Path,
    places: &Places,
    c: &Components,
    s: &Settings,
    headless: bool,
    host: Host,
    wheel: Option<&DolphinWheel>,
) -> Plan {
    let size = game_size(game, s);
    let rt = &places.runtime;
    let mut files = Vec::new();
    let mut links = Vec::new();
    let (game_cmd, feed) = match game.route {
        Route::Dolphin => {
            let (cmd, f, l, feed) = dolphin_plan(game, game_file, c, places, s, host, wheel);
            files.extend(f);
            links.extend(l);
            (cmd, feed)
        }
        Route::Wal { profile, .. } => {
            let root = &places.wal_root;
            files.push((
                rt.join(format!("{profile}.windowsloader")),
                format!("{}\n", game.loader_path()),
            ));
            let layer = root.join(format!("layer-{}.yaml", game.slug));
            files.push((layer.clone(), wal_layer()));
            links.push((root.join("systemprofiles"), c.wal.join("systemprofiles")));
            links.push((root.join("dist/payloads"), c.wal.join("payloads")));
            if host == Host::Linux {
                links.push((root.join("wine-runners/runner"), c.runner.clone()));
                links.push((root.join("wine-runners/winetricks"), c.winetricks.clone()));
            }
            let cmd = Command {
                // Windows: windowsarcadeloader's own run without Wine (patches/wal)
                program: c.wal.join(match host {
                    Host::Linux => "arcade-launcher",
                    Host::Windows => "arcade-launcher.exe",
                }),
                args: vec![
                    "run".into(),
                    rt.to_string_lossy().into_owned(),
                    "--root".into(),
                    root.to_string_lossy().into_owned(),
                    "--profile".into(),
                    layer.to_string_lossy().into_owned(),
                    "--input-pipe".into(),
                ],
                env: vec![],
                cwd: root.clone(),
            };
            (cmd, Feed::Wal(StickLayout::NamcoEs3Kart))
        }
    };
    // Windows: no gamescope (the games draw at their render size, full screen)
    let gamescope_home =
        (host == Host::Linux && s.crt == Crt::Lottes).then(|| rt.join("gamescope-home"));
    if let Some(home) = &gamescope_home {
        links.push((
            home.join(".local/share/gamescope/reshade/Shaders")
                .join(CRT_EFFECT),
            c.shaders.join(CRT_EFFECT),
        ));
    }
    Plan {
        files,
        links,
        game: game_cmd,
        gamescope: match host {
            Host::Linux => gamescope_args(s, size, headless),
            Host::Windows => Vec::new(),
        },
        gamescope_home,
        feed,
        game_size: size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{GameId, game};

    fn comps() -> Components {
        Components {
            dolphin: "/c/dolphin".into(),
            dolphin_win: "/c/dolphin-win".into(),
            wal: "/c/wal".into(),
            runner: "/c/runner".into(),
            winetricks: "/c/winetricks".into(),
            shaders: "/c/shaders".into(),
        }
    }

    fn places(slug: &str) -> Places {
        Places {
            runtime: PathBuf::from("/d/runtimes").join(slug),
            wal_root: "/d/wal".into(),
            segaboot: None,
        }
    }

    fn arg_after(args: &[String], flag: &str) -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
    }

    fn file<'a>(p: &'a Plan, name: &str) -> &'a str {
        &p.files.iter().find(|(f, _)| f.ends_with(name)).unwrap().1
    }

    fn linux(id: GameId, s: &Settings, headless: bool) -> Plan {
        plan(
            game(id),
            Path::new("/lib/x/game.iso"),
            &places(game(id).slug),
            &comps(),
            s,
            headless,
            Host::Linux,
            None,
        )
    }

    #[test]
    fn the_internal_resolution_reaches_the_render_height() {
        assert_eq!(dolphin_scale(None), 1);
        assert_eq!(dolphin_scale(Some(Size::new(1920, 1080))), 3);
        assert_eq!(dolphin_scale(Some(Size::new(2560, 1440))), 3);
        assert_eq!(dolphin_scale(Some(Size::new(3840, 2160))), 5);
        assert_eq!(dolphin_scale(Some(Size::new(15360, 8640))), 8);
    }

    #[test]
    fn a_triforce_game_runs_in_dolphin_with_its_own_user_folder() {
        let s = Settings {
            render: Some(Size::new(3840, 2160)),
            ..Settings::default()
        };
        let p = linux(GameId::MarioKartGp2, &s, false);
        let user = PathBuf::from("/d/runtimes/mkgp2/dolphin");
        assert_eq!(
            p.game.program,
            PathBuf::from("/c/dolphin/dolphin-emu-nogui")
        );
        // compared as paths: Windows joins with backslashes
        assert_eq!(
            arg_after(&p.game.args, "-u").map(PathBuf::from),
            Some(user.clone())
        );
        assert_eq!(
            arg_after(&p.game.args, "-e").as_deref(),
            Some("/lib/x/game.iso")
        );
        assert_eq!(arg_after(&p.game.args, "-p").as_deref(), Some("x11"));
        assert!(
            p.game
                .env
                .contains(&("LD_LIBRARY_PATH".into(), "/c/dolphin/lib".into()))
        );
        assert!(file(&p, "Config/Dolphin.ini").contains("SIDevice0 = 11"));
        assert!(file(&p, "Config/Dolphin.ini").contains("SerialPort1 = 6"));
        assert!(file(&p, "Config/Dolphin.ini").contains("UsePanicHandlers = False"));
        assert!(file(&p, "Config/GFX.ini").contains("InternalResolution = 5"));
        assert_eq!(p.game_size, Size::new(3200, 2400));
        assert_eq!(arg_after(&p.gamescope, "-h").as_deref(), Some("2400"));
        assert_eq!(p.feed, Feed::DolphinPipe(user.join("Pipes/launcher")));
    }

    #[test]
    fn each_triforce_game_gets_its_patches_and_cheats_on() {
        let p = linux(GameId::FZeroAx, &Settings::default(), false);
        assert!(file(&p, "Config/Dolphin.ini").contains("EnableCheats = True"));
        let gs = file(&p, "GameSettings/GHAE6E.ini");
        assert!(
            gs.contains("[Gecko]\n$Bypass CyCraft (no motion seat)\n0043137B 00000086\n"),
            "{gs}"
        );
        assert!(gs.contains("[Gecko_Enabled]\n$Bypass CyCraft (no motion seat)\n"));
        assert!(!gs.contains("ActionReplay"));
        let k = linux(GameId::MarioKartGp2, &Settings::default(), false);
        assert!(file(&k, "Config/Dolphin.ini").contains("EnableCheats = True"));
        let gs = file(&k, "GameSettings/GNLJ82.ini");
        assert!(
            gs.contains(
                "[ActionReplay_Enabled]\n$(2007/02/06 20:29:25) Skip Boot Check [mamefan2018]\n"
            ),
            "{gs}"
        );
        assert!(!gs.contains("[Gecko]"));
    }

    #[test]
    fn the_pipe_pad_maps_every_cabinet_input_and_full_steering() {
        let ini = gcpad_pipe_ini();
        assert!(ini.contains("Device = Pipe/0/launcher"));
        assert!(ini.contains("Main Stick/Gate Size = 100.0"));
        for k in [
            "Buttons/A = `Button A`",
            "Buttons/B = `Button B`",
            "Buttons/Start = `Button START`",
            "Triforce/Coin = `Button Z`",
            "Triforce/Test = `Button L`",
            "Triforce/Service = `Button R`",
            "Main Stick/Left = `Axis MAIN X -`",
            "Triggers/R-Analog = `Axis R +`",
            "Triggers/L-Analog = `Axis L +`",
        ] {
            assert!(ini.contains(k), "{k} in\n{ini}");
        }
    }

    #[test]
    fn segaboot_is_linked_into_the_triforce_folder_when_given() {
        let mut pl = places("fzax");
        pl.segaboot = Some("/bios/segaboot.gcm".into());
        let p = plan(
            game(GameId::FZeroAx),
            Path::new("/l/f.iso"),
            &pl,
            &comps(),
            &Settings::default(),
            false,
            Host::Linux,
            None,
        );
        assert!(p.links.contains(&(
            "/d/runtimes/fzax/dolphin/Triforce/segaboot.gcm".into(),
            "/bios/segaboot.gcm".into()
        )));
        assert!(
            linux(GameId::FZeroAx, &Settings::default(), false)
                .links
                .is_empty()
        );
    }

    #[test]
    fn on_windows_dolphin_reads_the_pad_the_keyboard_and_a_wheel_itself() {
        let w = DolphinWheel {
            name: "Logitech G29".into(),
            steer_axis: 0,
            gas: Some((2, 32767, -32768)),
            brake: Some((3, 0, -32768)),
        };
        let p = plan(
            game(GameId::FZeroAx),
            Path::new("C:/games/f.iso"),
            &places("fzax"),
            &comps(),
            &Settings::default(),
            false,
            Host::Windows,
            Some(&w),
        );
        assert_eq!(p.game.program, PathBuf::from("/c/dolphin-win/Dolphin.exe"));
        assert!(p.game.args.contains(&"-b".to_string()));
        assert_eq!(p.feed, Feed::None);
        assert!(p.gamescope.is_empty());
        let pad = file(&p, "GCPadNew.ini");
        assert!(pad.contains("Device = XInput/0/Gamepad"));
        assert!(pad.contains("`SDL/0/Logitech G29:Axis 0-`"), "{pad}");
        // a full-travel pedal resting at the top: both halves, inverted
        assert!(
            pad.contains(
                "((`SDL/0/Logitech G29:Axis 2-` - `SDL/0/Logitech G29:Axis 2+`) + 1) / 2 |"
            ),
            "{pad}"
        );
        // a half-travel pedal: one half
        assert!(
            pad.contains("Triggers/L-Analog = `SDL/0/Logitech G29:Axis 3-` |"),
            "{pad}"
        );
        assert!(file(&p, "Dolphin.ini").contains("GFXBackend = D3D11"));
    }

    #[test]
    fn mario_kart_dx_runs_in_windowsarcadeloader_with_the_kart_layout() {
        let s = Settings {
            render: Some(Size::new(2560, 1440)),
            ..Settings::default()
        };
        let p = linux(GameId::MarioKartDx, &s, false);
        assert_eq!(p.feed, Feed::Wal(StickLayout::NamcoEs3Kart));
        assert_eq!(p.game.program, PathBuf::from("/c/wal/arcade-launcher"));
        assert!(p.game.args.contains(&"--input-pipe".to_string()));
        assert_eq!(
            file(&p, "mario-kart-arcade-gp-dx.windowsloader"),
            "MK_AGP3_FINAL.exe\n"
        );
        assert!(file(&p, "layer-mkdx.yaml").contains("prefix: wine-prefix/nintendo"));
        assert_eq!(arg_after(&p.gamescope, "-w").as_deref(), Some("2560"));
        let runners = PathBuf::from("/d/wal/wine-runners");
        assert!(
            p.links
                .contains(&(runners.join("runner"), "/c/runner".into()))
        );
        assert!(
            p.links
                .contains(&(runners.join("winetricks"), "/c/winetricks".into()))
        );
    }

    #[test]
    fn on_windows_windowsarcadeloader_runs_its_windows_build_without_wine() {
        let p = plan(
            game(GameId::MarioKartDx),
            Path::new("/lib/mkdx/MK_AGP3_FINAL.exe"),
            &places("mkdx"),
            &comps(),
            &Settings::default(),
            false,
            Host::Windows,
            None,
        );
        assert_eq!(p.game.program, PathBuf::from("/c/wal/arcade-launcher.exe"));
        let runners = PathBuf::from("/d/wal/wine-runners");
        assert!(!p.links.iter().any(|(at, _)| at.starts_with(&runners)));
        assert!(p.gamescope.is_empty());
    }

    #[test]
    fn crt_upscaler_and_headless_reach_gamescope() {
        let s = Settings {
            crt: Crt::Lottes,
            ..Settings::default()
        };
        let p = linux(GameId::MarioKartDx, &s, false);
        assert_eq!(
            arg_after(&p.gamescope, "--reshade-effect").as_deref(),
            Some("crt-lottes.fx")
        );
        let home = p.gamescope_home.clone().unwrap();
        assert!(p.links.contains(&(
            home.join(".local/share/gamescope/reshade/Shaders/crt-lottes.fx"),
            PathBuf::from("/c/shaders/crt-lottes.fx")
        )));
        assert_eq!(arg_after(&p.gamescope, "-F").as_deref(), Some("fsr"));
        assert!(p.gamescope.contains(&"-f".to_string()));
        let h = linux(GameId::MarioKartDx, &s, true);
        assert_eq!(
            arg_after(&h.gamescope, "--backend").as_deref(),
            Some("headless")
        );
        assert_eq!(arg_after(&h.gamescope, "-F").as_deref(), Some("linear"));
        assert!(!h.gamescope.contains(&"-f".to_string()));
    }
}
