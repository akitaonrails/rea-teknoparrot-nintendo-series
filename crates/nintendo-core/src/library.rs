//! Finding the player's games in their library folders.
//!
//! A library is a folder of game folders (any names), one game each, as
//! TeknoParrot lays them out. Each folder whose main file (or any disc
//! image) matches a known edition becomes an [`Entry`]; a known file name
//! with an unknown digest is reported, never launched.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::catalog::{self, Edition, Game, GameId};

#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Known {
        game: &'static Game,
        edition: &'static Edition,
    },
    /// A file named like a game's main file, with a digest we don't know.
    Unknown { file: String, digest: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The game folder.
    pub dir: PathBuf,
    /// The main file found in it (a renamed disc image keeps its name).
    pub file: PathBuf,
    pub found: Found,
}

impl Entry {
    pub fn game_id(&self) -> Option<GameId> {
        match self.found {
            Found::Known { game, .. } => Some(game.id),
            Found::Unknown { .. } => None,
        }
    }
}

/// Digests by (path, size, mtime), so a rescan doesn't read 500 MB images again.
#[derive(Debug, Default, Clone)]
pub struct HashCache {
    map: HashMap<PathBuf, (u64, Option<SystemTime>, String)>,
}

impl HashCache {
    fn digest(&mut self, path: &Path) -> std::io::Result<String> {
        let meta = fs::metadata(path)?;
        let stamp = (meta.len(), meta.modified().ok());
        if let Some((len, mt, d)) = self.map.get(path)
            && (*len, *mt) == stamp
        {
            return Ok(d.clone());
        }
        let d = sha256_file(path)?;
        self.map
            .insert(path.to_path_buf(), (stamp.0, stamp.1, d.clone()));
        Ok(d)
    }
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// A disc image by its extension: a Triforce dump may have any name.
fn is_disc_image(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".iso") || n.ends_with(".gcm")
}

/// The files of `dir` worth hashing: every catalog main file present, and
/// every disc image (sizes of a known disc only, so a stray ISO of another
/// system is not read).
fn candidates(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for g in catalog::GAMES {
        let f = dir.join(g.main_file);
        if f.is_file() && !out.iter().any(|(n, _)| n == g.main_file) {
            out.push((g.main_file.to_string(), f));
        }
    }
    if let Ok(rd) = fs::read_dir(dir) {
        let mut images: Vec<(String, PathBuf)> = rd
            .filter_map(Result::ok)
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .filter(|(n, p)| is_disc_image(n) && p.is_file())
            .filter(|(n, _)| !out.iter().any(|(m, _)| m == n))
            .collect();
        images.sort();
        out.extend(images);
    }
    out
}

/// Every game in `dir` itself.
pub fn probe_dir(dir: &Path, cache: &mut HashCache) -> Vec<Entry> {
    let mut out = Vec::new();
    for (name, file) in candidates(dir) {
        let Ok(digest) = cache.digest(&file) else {
            continue;
        };
        let named = catalog::GAMES.iter().any(|g| g.main_file == name);
        let found = match catalog::identify(&digest) {
            Some((game, edition)) => Found::Known { game, edition },
            // a renamed image of something else is not ours to report
            None if !named => continue,
            None => Found::Unknown {
                file: name.clone(),
                digest,
            },
        };
        out.push(Entry {
            dir: dir.to_path_buf(),
            file,
            found,
        });
    }
    out
}
/// Every game in each root and the folders directly inside it, in catalog
/// order, unknown builds last.
pub fn scan(roots: &[PathBuf], cache: &mut HashCache) -> Vec<Entry> {
    let mut out = Vec::new();
    for root in roots {
        out.extend(probe_dir(root, cache));
        if let Ok(rd) = fs::read_dir(root) {
            let mut dirs: Vec<PathBuf> = rd
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            dirs.sort();
            for d in dirs {
                out.extend(probe_dir(&d, cache));
            }
        }
    }
    out.sort_by_key(|e| (e.game_id().is_none(), e.game_id()));
    out
}

/// The first folder found for each game (a library may hold duplicates).
pub fn by_game(entries: &[Entry]) -> HashMap<GameId, &Entry> {
    let mut m = HashMap::new();
    for e in entries {
        if let Some(id) = e.game_id() {
            m.entry(id).or_insert(e);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_digest_is_reported_with_its_hash() {
        let t = tempfile::tempdir().unwrap();
        let g = t.path().join("Some Game [TP]");
        fs::create_dir(&g).unwrap();
        fs::write(g.join("MK_AGP3_FINAL.exe"), b"not the game").unwrap();
        fs::create_dir(t.path().join("empty")).unwrap();
        let found = scan(&[t.path().to_path_buf()], &mut HashCache::default());
        assert_eq!(found.len(), 1);
        match &found[0].found {
            Found::Unknown { file, digest } => {
                assert_eq!(file, "MK_AGP3_FINAL.exe");
                assert_eq!(digest, &sha256_file(&g.join("MK_AGP3_FINAL.exe")).unwrap());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_stray_disc_image_is_neither_known_nor_reported() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("holiday.iso"), b"junk").unwrap();
        fs::write(t.path().join("notes.txt"), b"x").unwrap();
        assert!(probe_dir(t.path(), &mut HashCache::default()).is_empty());
        let c = candidates(t.path());
        assert_eq!(c.len(), 1, "only the image is hashed: {c:?}");
    }

    #[test]
    fn cache_rehashes_only_when_the_file_changes() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("main");
        fs::write(&f, b"one").unwrap();
        let mut c = HashCache::default();
        let a = c.digest(&f).unwrap();
        assert_eq!(c.digest(&f).unwrap(), a);
        fs::write(&f, b"three").unwrap();
        assert_ne!(c.digest(&f).unwrap(), a);
    }

    /// The owner's real library, when present
    /// (`NINTENDO_LIBRARIES=dir1:dir2 cargo test -- --ignored`).
    #[test]
    #[ignore]
    fn real_library_finds_every_game() {
        let Ok(roots) = std::env::var("NINTENDO_LIBRARIES") else {
            return;
        };
        let roots: Vec<PathBuf> = std::env::split_paths(&roots).collect();
        let found = scan(&roots, &mut HashCache::default());
        let ids = by_game(&found);
        assert_eq!(ids.len(), catalog::GAMES.len(), "{found:#?}");
    }
}
