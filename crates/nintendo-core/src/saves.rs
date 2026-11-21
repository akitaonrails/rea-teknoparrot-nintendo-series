//! Per-game progress, card, and cabinet saves, mirrored between
//! `<data>/saves/<slug>/` (clean, backup-friendly, player-owned) and the
//! game's private runtime folder (`rea_launcher::saves::Mirror`).

use std::io;
use std::path::{Path, PathBuf};

use rea_launcher::saves::{Deleted, FirstRun, Mirror, Moved};

use crate::catalog::{Game, Route};

/// The player's backup-friendly saves folder for `slug` (`<data>/saves/<slug>`).
pub fn dir(data: &Path, slug: &str) -> PathBuf {
    data.join("saves").join(slug)
}

/// The folder inside `runtime` where `game` keeps its save files.
pub fn runtime_dir(game: &Game, runtime: &Path) -> PathBuf {
    match game.route {
        Route::Dolphin => runtime.join("dolphin"),
        Route::Wal { .. } => runtime.to_path_buf(),
    }
}

/// Whether `rel` (relative to [`runtime_dir`], `/` separators) is a save or
/// cabinet backup file of `game` that belongs in `saves/<slug>/`.
pub fn include(game: &Game, rel: &str) -> bool {
    let lower = rel.replace('\\', "/").to_ascii_lowercase();
    if lower.is_empty() || lower.ends_with(".log") || lower.ends_with(".dmp") {
        return false;
    }
    match game.route {
        Route::Dolphin => {
            lower.starts_with("gc/")
                || lower.starts_with("triforce/")
                || lower.starts_with("statesaves/")
                || (!lower.contains('/')
                    && [".raw", ".gci", ".sram", ".bin"]
                        .iter()
                        .any(|ext| lower.ends_with(ext)))
        }
        Route::Wal { .. } => {
            lower.starts_with("appdata/")
                || lower.starts_with("amfs/")
                || lower.starts_with("f/")
                || lower.starts_with("g/")
        }
    }
}

fn scan_dirs(game: &Game) -> &'static [&'static str] {
    match game.route {
        Route::Dolphin => &["", "GC", "Triforce", "StateSaves"],
        Route::Wal { .. } => &["AppData", "AMFS", "F", "G"],
    }
}

fn with_mirror<T>(
    game: &Game,
    runtime: &Path,
    saves_dir: &Path,
    f: impl for<'a> FnOnce(&Mirror<'a>) -> io::Result<T>,
) -> io::Result<T> {
    let rt_dir = runtime_dir(game, runtime);
    let state = runtime.join(".saves-state");
    let keep = |rel: &str| include(game, rel);
    let mirror = Mirror::folder(&rt_dir, saves_dir, &state, FirstRun::SavesWin)
        .deleted(Deleted::Restore)
        .scan(scan_dirs(game))
        .only(&keep);
    f(&mirror)
}

/// Mirrors `game`'s saves before the game starts.
pub fn sync_before(game: &Game, runtime: &Path, saves_dir: &Path) -> io::Result<Moved> {
    with_mirror(game, runtime, saves_dir, |m| m.before())
}

/// Copies save changes written by `game` back into `saves_dir`.
pub fn sync_after(game: &Game, runtime: &Path, saves_dir: &Path) -> io::Result<Vec<String>> {
    with_mirror(game, runtime, saves_dir, |m| m.after())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{GameId, game};
    use std::fs;

    #[test]
    fn each_route_keeps_saves_and_skips_config_and_logs() {
        let fz = game(GameId::FZeroAx);
        assert!(include(fz, "GC/MemoryCardA.USA.raw"));
        assert!(include(fz, "Triforce/sram.bin"));
        assert!(!include(fz, "Config/Dolphin.ini"));

        let mk = game(GameId::MarioKartDx);
        assert!(include(mk, "F/TestMode/testmode_1.bin"));
        assert!(include(mk, "AppData/profile.sav"));
        assert!(!include(mk, "AppData/debug.log"));
    }

    #[test]
    fn round_trip_mirrors_saves_out_and_restores_player_backup() {
        let t = tempfile::tempdir().unwrap();
        let rt = t.path().join("runtimes/fzax");
        let sv = dir(t.path(), "fzax");
        fs::create_dir_all(rt.join("dolphin/GC")).unwrap();

        sync_before(game(GameId::FZeroAx), &rt, &sv).unwrap();
        fs::write(rt.join("dolphin/GC/card.raw"), b"v1").unwrap();
        let after = sync_after(game(GameId::FZeroAx), &rt, &sv).unwrap();
        assert_eq!(after, vec!["GC/card.raw"]);
        assert_eq!(fs::read(sv.join("GC/card.raw")).unwrap(), b"v1");

        fs::write(sv.join("GC/card.raw"), b"backup").unwrap();
        let before = sync_before(game(GameId::FZeroAx), &rt, &sv).unwrap();
        assert_eq!(before.to_runtime, vec!["GC/card.raw"]);
        assert_eq!(fs::read(rt.join("dolphin/GC/card.raw")).unwrap(), b"backup");
    }
}
