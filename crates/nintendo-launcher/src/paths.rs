//! Where the launcher keeps things (`rea_launcher::hub::HubPaths`).
//!
//! - data (`$NINTENDO_DATA`, else `$XDG_DATA_HOME/rea-nintendo` or
//!   `~/.local/share/rea-nintendo`; on Windows `%LOCALAPPDATA%\rea-nintendo`):
//!   `launcher.cfg`, `settings/`, `runtimes/` (one per game: Dolphin's user
//!   folder and saves, or the Windows game's linked folder), `wal/`
//!   (windowsarcadeloader's root: its Wine prefix and profile layers),
//!   `launcher.log`;
//! - components (`$NINTENDO_COMPONENTS`, else `components/` beside the
//!   program): `dolphin/` (Linux) or `dolphin-win/` (Windows), `wal/`,
//!   `runner/`, `winetricks`, `shaders/`, `art/`.

use std::path::PathBuf;

use nintendo_core::launch::Components;
use rea_launcher::hub::{HubDirs, HubPaths, PathField, PathKind};

pub const DIRS: HubDirs = HubDirs {
    env_prefix: "NINTENDO",
    name: "rea-nintendo",
    dev_assets: if cfg!(debug_assertions) {
        Some(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
    } else {
        None
    },
};

/// The Triforce test-menu program, optional (`launcher.cfg`'s `segaboot`).
pub const SEGABOOT: PathField = PathField {
    key: "segaboot",
    aliases: &[],
    kind: PathKind::File,
    flag: "--segaboot",
    flag_aliases: &[],
    help: "the Triforce test-menu program (segaboot.gcm), optional",
    heading: "Triforce test menu (segaboot.gcm), optional: F-Zero AX and Mario Kart GP 2 play \
              without it",
    label: "segaboot.gcm file",
    placeholder: "/path/to/segaboot.gcm",
    button: "Use file",
    empty: "Type the path of segaboot.gcm first.",
    saved: "Triforce test menu saved.",
};

/// The launcher's paths besides the library folders.
pub static FIELDS: [PathField; 1] = [SEGABOOT];

/// windowsarcadeloader's root (shared by every Windows game: prefix, links).
pub fn wal_root(p: &HubPaths) -> PathBuf {
    p.data.join("wal")
}

pub fn components(p: &HubPaths) -> Components {
    let c = &p.components;
    Components {
        dolphin: c.join("dolphin"),
        dolphin_win: c.join("dolphin-win"),
        wal: c.join("wal"),
        runner: c.join("runner"),
        winetricks: c.join("winetricks"),
        shaders: c.join("shaders"),
    }
}
