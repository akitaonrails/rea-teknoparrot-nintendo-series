//! The private runtime folder each game runs from, as a
//! [`rea_launcher::linkfarm`] policy: the game knowledge of which entries
//! of a dump are linked, copied, opened or seeded.
//!
//! The player's game folder is read-only (it may be on a network share,
//! and the launcher runs every game with the library bound read-only).
//! A Windows game's runtime holds a link to each top-level entry of its
//! folder, copies of the small files it rewrites, real folders on the way
//! to its program (windowsarcadeloader puts its payloads and save folder
//! beside the program) and the cabinet drives it writes. A Dolphin game
//! reads its disc image where it is: its runtime holds only Dolphin's
//! user folder.

use std::io;
use std::path::Path;

use rea_launcher::linkfarm::{self, Item, Op, Place, Policy};

use crate::catalog::{Game, Route};

/// Top-level files of a Windows game below this size are copied, not
/// linked: the games write their logs and account files there
/// (Mario Kart DX's logs and settings copies), and the library is
/// read-only while they run.
const SMALL: u64 = 4 * 1024 * 1024;

/// A small top-level file that is not code.
fn copied(it: &Item) -> bool {
    let code = [".exe", ".dll", ".manifest"]
        .iter()
        .any(|x| it.name.to_ascii_lowercase().ends_with(x));
    !it.dir && it.size < SMALL && !code
}

/// Top-level names never linked: windowsarcadeloader's dump marker and
/// the cabinet drives and folders it creates (Mario Kart DX writes its
/// operator settings to F: and G:).
fn loader_owned(name: &str) -> bool {
    name.ends_with(".windowsloader")
        || matches!(
            name,
            "AMFS" | "amfs" | "AppData" | "F" | "G" | "WindowsLoader"
        )
}

impl Policy for Game {
    fn place(&self, rel: &str, it: &Item) -> Place {
        if self.route == Route::Dolphin {
            return Place::Skip;
        }
        let top = !rel.contains('/');
        let program_dir = Path::new(self.main_file).parent().unwrap_or(Path::new(""));
        if top && (linkfarm::ntfs_leftover(&it.name) || loader_owned(&it.name)) {
            Place::Skip
        } else if it.dir && program_dir.starts_with(rel) {
            // the folders on the way to the program (Luigi's Mansion
            // Arcade runs exe/x64/VACUUM.exe)
            Place::Open
        } else if top && copied(it) {
            Place::CopyOnce
        } else {
            Place::Link
        }
    }

    fn seed(&self, rel: &str) -> Option<&str> {
        self.seeds
            .iter()
            .find(|(dir, _)| *dir == rel)
            .map(|(_, into)| *into)
    }
}

/// The runtime `rt` of `game` from its folder `src`, listed with `ls`
/// ([`linkfarm::list`], or a fake in tests).
pub fn plan(
    game: &Game,
    src: &Path,
    rt: &Path,
    ls: &dyn Fn(&Path) -> io::Result<Vec<Item>>,
) -> io::Result<Vec<Op>> {
    linkfarm::plan(src, rt, game, ls)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{GameId, game};
    use rea_launcher::linkfarm::{Occupied, apply, list};
    use std::fs;

    fn real(game_id: GameId, src: &Path, rt: &Path) -> Vec<Op> {
        plan(game(game_id), src, rt, &list).unwrap()
    }

    #[test]
    fn wal_runtime_links_the_dump_but_not_drives_or_markers() {
        let t = tempfile::tempdir().unwrap();
        let (src, rt) = (t.path().join("mkdx"), t.path().join("rt"));
        fs::create_dir_all(src.join("F")).unwrap();
        fs::create_dir_all(src.join("Data")).unwrap();
        fs::write(src.join("MK_AGP3_FINAL.exe"), vec![0; 100]).unwrap();
        fs::write(src.join("old.windowsloader"), b"x").unwrap();
        fs::write(src.join("init.ps1"), b"x").unwrap();
        let ops = real(GameId::MarioKartDx, &src, &rt);
        let linked: Vec<_> = ops
            .iter()
            .filter_map(|o| match o {
                Op::Link { at, .. } => Some(at.file_name().unwrap().to_str().unwrap()),
                _ => None,
            })
            .collect();
        assert_eq!(linked, ["Data", "MK_AGP3_FINAL.exe"]);
        assert!(
            ops.iter()
                .any(|o| matches!(o, Op::CopyOnce { to, .. } if to.ends_with("init.ps1")))
        );
    }

    #[test]
    #[cfg(unix)]
    fn small_files_of_a_windows_game_are_copied_once_and_kept() {
        let t = tempfile::tempdir().unwrap();
        let (src, rt) = (t.path().join("dump"), t.path().join("rt"));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("error.log"), b"old").unwrap();
        fs::write(src.join("bngrw.dll"), b"dll").unwrap();
        let ops = real(GameId::MarioKartDx, &src, &rt);
        // an older launcher linked the log: replaced by a copy
        fs::create_dir_all(&rt).unwrap();
        std::os::unix::fs::symlink(src.join("error.log"), rt.join("error.log")).unwrap();
        apply(&ops, Occupied::Refuse).unwrap();
        let log = rt.join("error.log");
        assert!(!fs::symlink_metadata(&log).unwrap().file_type().is_symlink());
        assert!(
            fs::symlink_metadata(rt.join("bngrw.dll"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::write(&log, b"the game's").unwrap();
        apply(&ops, Occupied::Refuse).unwrap();
        assert_eq!(fs::read(&log).unwrap(), b"the game's");
        assert_eq!(fs::read(src.join("error.log")).unwrap(), b"old");
    }

    #[test]
    fn mario_kart_dx_gets_the_dumps_test_settings_once() {
        let t = tempfile::tempdir().unwrap();
        let (src, rt) = (t.path().join("dump"), t.path().join("rt"));
        fs::create_dir_all(src.join("FTestMode")).unwrap();
        fs::create_dir_all(src.join("GTestMode")).unwrap();
        fs::write(src.join("FTestMode/testmode_1.bin"), b"camera off").unwrap();
        fs::write(src.join("GTestMode/testmode_1.bin"), b"camera off").unwrap();
        let ops = real(GameId::MarioKartDx, &src, &rt);
        apply(&ops, Occupied::Refuse).unwrap();
        let f = rt.join("F/TestMode/testmode_1.bin");
        assert_eq!(fs::read(&f).unwrap(), b"camera off");
        assert!(rt.join("G/TestMode/testmode_1.bin").is_file());
        // the game rewrites its settings: they are its own from then on
        fs::remove_file(&f).unwrap();
        fs::write(rt.join("F/TestMode/testmode_2.bin"), b"the player's").unwrap();
        apply(&ops, Occupied::Refuse).unwrap();
        assert!(!f.exists());
        assert_eq!(
            fs::read(src.join("FTestMode/testmode_1.bin")).unwrap(),
            b"camera off"
        );
    }

    #[test]
    fn a_dolphin_runtime_links_nothing() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("fzax");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("F-Zero AX.iso"), b"disc").unwrap();
        let rt = t.path().join("rt");
        assert_eq!(real(GameId::FZeroAx, &src, &rt), [Op::Dir(rt)]);
    }

    #[test]
    #[cfg(unix)]
    fn the_folders_on_the_way_to_the_program_are_real() {
        let t = tempfile::tempdir().unwrap();
        let (src, rt) = (t.path().join("dump"), t.path().join("rt"));
        fs::create_dir_all(src.join("exe/x64")).unwrap();
        fs::create_dir_all(src.join("DataGlobal")).unwrap();
        fs::write(src.join("exe/x64/VACUUM.exe"), b"game").unwrap();
        fs::write(src.join("exe/readme.txt"), b"r").unwrap();
        // an older run linked the whole folder
        fs::create_dir_all(&rt).unwrap();
        std::os::unix::fs::symlink(src.join("exe"), rt.join("exe")).unwrap();
        let ops = real(GameId::LuigisMansion, &src, &rt);
        apply(&ops, Occupied::Refuse).unwrap();
        let is_link = |p: &str| {
            fs::symlink_metadata(rt.join(p))
                .unwrap()
                .file_type()
                .is_symlink()
        };
        assert!(!is_link("exe") && !is_link("exe/x64"));
        assert!(
            is_link("exe/x64/VACUUM.exe") && is_link("exe/readme.txt") && is_link("DataGlobal")
        );
        // the loader's save folder lands in the runtime, not in the dump
        fs::create_dir_all(rt.join("exe/x64/sv")).unwrap();
        assert!(!src.join("exe/x64/sv").exists());
        apply(&ops, Occupied::Refuse).unwrap();
        assert!(rt.join("exe/x64/sv").is_dir());
    }
}
