//! The private runtime folder each game runs from.
//!
//! The player's game folder is read-only (it may be on a network share,
//! and the launcher runs every game with the library bound read-only).
//! The runtime holds a symlink to each top-level entry of the game folder,
//! plus the files the launcher writes and the folders the loaders create
//! (saves, cabinet drives) as real files. [`plan`] decides from a listing;
//! [`apply`] carries it out without ever touching the game folder.
//!
//! On Windows, where a symlink needs administrator rights or developer
//! mode, a linked folder is a directory junction (a directory symlink for a
//! folder on a network share, which a junction cannot reach) and a linked
//! file is a copy, refreshed when the game folder's file changes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::catalog::{Game, Route};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Dir(PathBuf),
    /// Symlink `at` → `target` (replaced when it points elsewhere).
    Link {
        target: PathBuf,
        at: PathBuf,
    },
    /// A copy the game owns from then on: made once (replacing a link an
    /// older launcher left), never overwritten.
    CopyOnce {
        from: PathBuf,
        to: PathBuf,
    },
    /// The files of folder `from` copied into folder `to` while `to` holds
    /// no file yet (the first start); never again after that.
    Seed {
        from: PathBuf,
        to: PathBuf,
    },
    /// A real folder `to` holding a link to each entry of folder `from`,
    /// except the folders on the way down `below`, mirrored the same way:
    /// the folders on the way to a game's program, where windowsarcadeloader
    /// puts its payloads and save folder (Luigi's Mansion Arcade runs
    /// `exe/x64/VACUUM.exe`).
    Mirror {
        from: PathBuf,
        to: PathBuf,
        below: PathBuf,
    },
}

/// Top-level files of a Windows game below this size are copied, not
/// linked: the games write their logs and account files there
/// (Mario Kart DX's logs and settings copies), and the library is
/// read-only while they run.
const SMALL: u64 = 4 * 1024 * 1024;

fn copied(route: Route, it: &Item) -> bool {
    let code = [".exe", ".dll", ".manifest"]
        .iter()
        .any(|x| it.name.to_ascii_lowercase().ends_with(x));
    matches!(route, Route::Wal { .. }) && !it.dir && it.size < SMALL && !code
}

/// One entry of the game folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub dir: bool,
    pub size: u64,
}

pub fn list(dir: &Path) -> io::Result<Vec<Item>> {
    let mut v = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        let m = fs::metadata(e.path())?;
        v.push(Item {
            name: e.file_name().to_string_lossy().into_owned(),
            dir: m.is_dir(),
            size: m.len(),
        });
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(v)
}

/// Names never linked: files the loader writes in the runtime, and NTFS
/// leftovers in the dumps.
fn skip(route: Route, name: &str) -> bool {
    if name == "System Volume Information" || name.starts_with('$') {
        return true;
    }
    match route {
        // windowsarcadeloader's dump marker and the cabinet drives it creates
        // (Mario Kart DX writes its operator settings to F: and G:)
        Route::Wal { .. } => {
            name.ends_with(".windowsloader")
                || matches!(
                    name,
                    "AMFS" | "amfs" | "AppData" | "F" | "G" | "WindowsLoader"
                )
        }
        Route::Dolphin => false,
    }
}

/// The runtime of a Windows game: every entry of the game folder linked.
/// A Dolphin game reads its disc image where it is; its runtime holds only
/// Dolphin's user folder.
pub fn plan(game: &Game, src: &Path, items: &[Item], rt: &Path) -> Vec<Op> {
    let mut ops = vec![Op::Dir(rt.to_path_buf())];
    if game.route == Route::Dolphin {
        return ops;
    }
    for it in items {
        if skip(game.route, &it.name) {
            continue;
        }
        let (from, to) = (src.join(&it.name), rt.join(&it.name));
        let program = Path::new(game.main_file);
        if let Ok(below) = program
            .parent()
            .unwrap_or(Path::new(""))
            .strip_prefix(&it.name)
            && it.dir
        {
            ops.push(Op::Mirror {
                from,
                to,
                below: below.to_path_buf(),
            });
            continue;
        }
        if let Some((_, into)) = game.seeds.iter().find(|(dir, _)| *dir == it.name) {
            ops.push(Op::Seed {
                from: from.clone(),
                to: rt.join(into),
            });
        }
        if copied(game.route, it) {
            ops.push(Op::CopyOnce { from, to });
        } else {
            ops.push(Op::Link {
                target: from,
                at: to,
            });
        }
    }
    ops
}

/// Carry out a plan. A link is replaced only when it points elsewhere; a
/// real file or folder where a link should be is an error (it would be
/// the player's data), never removed.
pub fn apply(ops: &[Op]) -> io::Result<()> {
    for op in ops {
        match op {
            Op::Dir(d) => fs::create_dir_all(d)?,
            Op::Link { target, at } => link(target, at)?,
            Op::Seed { from, to } => seed(from, to)?,
            Op::Mirror { from, to, below } => mirror(from, to, below)?,
            Op::CopyOnce { from, to } => match fs::symlink_metadata(to) {
                Ok(m) if !m.file_type().is_symlink() => {}
                found => {
                    if found.is_ok() {
                        fs::remove_file(to)?;
                    }
                    fs::copy(from, to)?;
                }
            },
        }
    }
    Ok(())
}

/// A real folder `to` with a link to each entry of `from`; the first
/// folder of `below` is mirrored in turn. A link an older launcher left at
/// `to` is replaced.
fn mirror(from: &Path, to: &Path, below: &Path) -> io::Result<()> {
    if fs::symlink_metadata(to).is_ok_and(|m| m.file_type().is_symlink()) {
        remove_link(to)?;
    }
    fs::create_dir_all(to)?;
    let mut parts = below.components();
    let next = parts.next().map(|c| c.as_os_str().to_owned());
    for e in fs::read_dir(from)? {
        let name = e?.file_name();
        if Some(&name) == next.as_ref() {
            mirror(&from.join(&name), &to.join(&name), parts.as_path())?;
        } else {
            link(&from.join(&name), &to.join(&name))?;
        }
    }
    Ok(())
}

/// Copies the files of `from` into `to` unless `to` already has a file.
fn seed(from: &Path, to: &Path) -> io::Result<()> {
    let has_file = fs::read_dir(to)
        .is_ok_and(|mut rd| rd.any(|e| e.is_ok_and(|e| e.file_type().is_ok_and(|t| t.is_file()))));
    if has_file || !from.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        if e.file_type()?.is_file() {
            fs::copy(e.path(), to.join(e.file_name()))?;
        }
    }
    Ok(())
}

fn real_where_a_link_goes(at: &Path) -> io::Error {
    io::Error::other(format!(
        "{} is a real file or folder where the launcher expects a link; move it away",
        at.display()
    ))
}

/// A symlink `at` → `target`, replaced when it points elsewhere.
#[cfg(unix)]
fn link(target: &Path, at: &Path) -> io::Result<()> {
    match fs::symlink_metadata(at) {
        Ok(m) if m.file_type().is_symlink() => {
            if fs::read_link(at)? != *target {
                fs::remove_file(at)?;
                std::os::unix::fs::symlink(target, at)?;
            }
            Ok(())
        }
        Ok(_) => Err(real_where_a_link_goes(at)),
        Err(_) => std::os::unix::fs::symlink(target, at),
    }
}

/// Windows: a folder is a directory junction, a file a copy kept in step.
#[cfg(windows)]
fn link(target: &Path, at: &Path) -> io::Result<()> {
    let existing = fs::symlink_metadata(at).ok();
    if target.is_dir() {
        match existing {
            Some(m) if m.file_type().is_symlink() => {
                if junction::get_target(at).ok().as_deref() == Some(target) {
                    return Ok(());
                }
                remove_link(at)?;
            }
            Some(_) => return Err(real_where_a_link_goes(at)),
            None => {}
        }
        junction::create(target, at).or_else(|e| {
            std::os::windows::fs::symlink_dir(target, at).map_err(|_| {
                io::Error::new(
                    e.kind(),
                    format!(
                        "cannot link {} into the runtime folder ({e}); a game folder on a \
                         network share needs Windows developer mode",
                        target.display()
                    ),
                )
            })
        })
    } else {
        match existing {
            Some(m) if m.file_type().is_symlink() => remove_link(at)?,
            Some(m) if m.is_dir() => return Err(real_where_a_link_goes(at)),
            _ => {}
        }
        mirror_file(target, at)
    }
}

/// Keeps `at` a copy of `target`: copied when missing or when the size or
/// modification time differ; the copy gets the source's modification time.
#[cfg(windows)]
fn mirror_file(target: &Path, at: &Path) -> io::Result<()> {
    let src = fs::metadata(target)?;
    if let Ok(m) = fs::metadata(at)
        && m.len() == src.len()
        && m.modified().ok() == src.modified().ok()
    {
        return Ok(());
    }
    fs::copy(target, at)?;
    fs::File::options()
        .write(true)
        .open(at)?
        .set_modified(src.modified()?)
}

/// Removes a link the launcher made (on Windows a junction is a folder).
pub fn remove_link(at: &Path) -> io::Result<()> {
    fs::remove_file(at).or_else(|e| fs::remove_dir(at).map_err(|_| e))
}

/// Links in a runtime folder whose target is gone (a dump moved): removed
/// so a re-plan can link the new place.
pub fn dangling(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut v = Vec::new();
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if fs::symlink_metadata(&p)?.file_type().is_symlink() && fs::metadata(&p).is_err() {
            v.push(p);
        }
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{GameId, game};

    fn items(names: &[&str]) -> Vec<Item> {
        names
            .iter()
            .map(|n| Item {
                name: n.to_string(),
                dir: false,
                size: if n.ends_with(".exe") { 30 << 20 } else { 100 },
            })
            .collect()
    }

    #[test]
    fn wal_runtime_links_the_dump_but_not_drives_or_markers() {
        let g = game(GameId::MarioKartDx);
        let ops = plan(
            g,
            Path::new("/lib/mkdx"),
            &items(&[
                "MK_AGP3_FINAL.exe",
                "F",
                "old.windowsloader",
                "Data",
                "init.ps1",
            ]),
            Path::new("/rt/mkdx"),
        );
        let linked: Vec<_> = ops
            .iter()
            .filter_map(|o| match o {
                Op::Link { at, .. } => Some(at.file_name().unwrap().to_str().unwrap()),
                _ => None,
            })
            .collect();
        assert_eq!(linked, ["MK_AGP3_FINAL.exe"]);
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
        let ops = plan(game(GameId::MarioKartDx), &src, &list(&src).unwrap(), &rt);
        // an older launcher linked the log: replaced by a copy
        fs::create_dir_all(&rt).unwrap();
        std::os::unix::fs::symlink(src.join("error.log"), rt.join("error.log")).unwrap();
        apply(&ops).unwrap();
        let log = rt.join("error.log");
        assert!(!fs::symlink_metadata(&log).unwrap().file_type().is_symlink());
        assert!(
            fs::symlink_metadata(rt.join("bngrw.dll"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::write(&log, b"the game's").unwrap();
        apply(&ops).unwrap();
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
        let ops = plan(game(GameId::MarioKartDx), &src, &list(&src).unwrap(), &rt);
        apply(&ops).unwrap();
        let f = rt.join("F/TestMode/testmode_1.bin");
        assert_eq!(fs::read(&f).unwrap(), b"camera off");
        assert!(rt.join("G/TestMode/testmode_1.bin").is_file());
        // the game rewrites its settings: they are its own from then on
        fs::remove_file(&f).unwrap();
        fs::write(rt.join("F/TestMode/testmode_2.bin"), b"the player's").unwrap();
        apply(&ops).unwrap();
        assert!(!f.exists());
        assert_eq!(
            fs::read(src.join("FTestMode/testmode_1.bin")).unwrap(),
            b"camera off"
        );
    }

    #[test]
    fn a_dolphin_runtime_links_nothing() {
        let ops = plan(
            game(GameId::FZeroAx),
            Path::new("/lib/fzax"),
            &items(&["F-Zero AX (Triforce) (Monster) [JAP][BHA6].iso"]),
            Path::new("/rt/fzax"),
        );
        assert_eq!(ops, [Op::Dir("/rt/fzax".into())]);
    }

    #[test]
    fn apply_links_relinks_and_never_replaces_real_files() {
        let t = tempfile::tempdir().unwrap();
        let (a, b, rt) = (t.path().join("a"), t.path().join("b"), t.path().join("rt"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let at = rt.join("data");
        apply(&[
            Op::Dir(rt.clone()),
            Op::Link {
                target: a.clone(),
                at: at.clone(),
            },
        ])
        .unwrap();
        apply(&[Op::Link {
            target: b.clone(),
            at: at.clone(),
        }])
        .unwrap();
        // the link follows the new target (a symlink, or a junction on Windows)
        fs::write(b.join("in-b"), b"b").unwrap();
        assert_eq!(fs::read(at.join("in-b")).unwrap(), b"b");
        fs::write(rt.join("save.bin"), b"mine").unwrap();
        let err = apply(&[Op::Link {
            target: a,
            at: rt.join("save.bin"),
        }]);
        assert!(err.is_err());
        assert_eq!(fs::read(rt.join("save.bin")).unwrap(), b"mine");
    }

    /// Windows: a linked file is a copy, refreshed when the game's file
    /// changes; a copy is never mistaken for the player's file.
    #[test]
    #[cfg(windows)]
    fn on_windows_a_linked_file_is_a_copy_kept_in_step() {
        let t = tempfile::tempdir().unwrap();
        let (src, at) = (t.path().join("game.exe"), t.path().join("rt-game.exe"));
        fs::write(&src, b"v1").unwrap();
        let op = Op::Link {
            target: src.clone(),
            at: at.clone(),
        };
        apply(std::slice::from_ref(&op)).unwrap();
        assert!(!fs::symlink_metadata(&at).unwrap().file_type().is_symlink());
        assert_eq!(fs::read(&at).unwrap(), b"v1");
        fs::write(&src, b"v2, longer").unwrap();
        apply(std::slice::from_ref(&op)).unwrap();
        assert_eq!(fs::read(&at).unwrap(), b"v2, longer");
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
        let ops = plan(game(GameId::LuigisMansion), &src, &list(&src).unwrap(), &rt);
        apply(&ops).unwrap();
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
        apply(&ops).unwrap();
        assert!(rt.join("exe/x64/sv").is_dir());
    }
}
