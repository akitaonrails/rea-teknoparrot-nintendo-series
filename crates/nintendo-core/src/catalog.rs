//! The games, what runs each one, and the builds we know.
//!
//! An edition is recognised by the SHA-256 of its main file (the disc image
//! of a Triforce game, the executable of the Namco ES3 game). Only builds
//! listed here are launched; anything else is reported with its digest so
//! the player can file it (STANDARDS: "Exact binary identity").

use std::ops::RangeInclusive;

use rea_launcher::catalog::{Edition, Host, Support, Title};
use rea_launcher::library::{Extra, Library};

use crate::input::StickLayout;
use rea_launcher::gamescope::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameId {
    FZeroAx,
    MarioKartGp2,
    MarioKartDx,
    LuigisMansion,
}

/// How a game is run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// A Sega/Namco/Nintendo Triforce disc image in Dolphin (Triforce
    /// emulation is in mainline Dolphin since release 2603).
    Dolphin,
    /// Windows build under Wine with windowsarcadeloader's payload for this
    /// system profile (`systemprofiles/<system>/<profile>.yaml`).
    Wal {
        system: &'static str,
        profile: &'static str,
        /// What the profile's virtual stick buttons and axes are.
        layout: StickLayout,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Game {
    pub id: GameId,
    /// Stable id for files and the command line.
    pub slug: &'static str,
    pub title: &'static str,
    /// Short name on a tile.
    pub short: &'static str,
    pub year: u16,
    pub board: &'static str,
    /// Short trivia/context shown over the top-right quarter of the hero art.
    pub blurb: &'static str,
    pub route: Route,
    /// The file that identifies the edition, relative to the game folder.
    pub main_file: &'static str,
    pub editions: &'static [Edition],
    /// The size the cabinet's screen had.
    pub native: Size,
    pub support: Support,
    /// The id Dolphin gives the disc (`G`, two letters of its `boot.id`,
    /// the region letter, the maker code; Dolphin `DiscIO/VolumeDisc.cpp`),
    /// as seen running this build; empty for a Windows game.
    pub dolphin_id: &'static str,
    /// Dolphin game settings for the cabinet hardware a PC lacks (see
    /// [`Patch`]); written into the runtime's Dolphin folder.
    pub patches: &'static [Patch],
    /// Folders of the dump whose files seed a folder of the runtime the
    /// first time (dump folder, runtime folder): settings a game keeps on
    /// its own drives, which it then rewrites itself.
    pub seeds: &'static [(&'static str, &'static str)],
}

/// A change Dolphin applies to the game's memory, for cabinet hardware
/// Dolphin does not emulate. Each is listed in docs/RESEARCH.md with where
/// it comes from and what it skips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Patch {
    /// A Gecko code the launcher writes.
    Gecko {
        name: &'static str,
        lines: &'static [&'static str],
    },
    /// An Action Replay code Dolphin ships for the game
    /// (`Sys/GameSettings/<id>.ini`), turned on by its exact name.
    ShippedActionReplay(&'static str),
}

impl Game {
    pub fn support_on(&self, _host: Host) -> Support {
        self.support
    }

    /// The program for windowsarcadeloader's `.windowsloader` file, a
    /// Windows path relative to the runtime.
    pub fn loader_path(&self) -> String {
        self.main_file.replace('/', "\\")
    }

    /// A gun game: the stick moves a crosshair.
    pub fn aims(&self) -> bool {
        matches!(self.route, Route::Wal { layout, .. } if layout.aims())
    }
}

impl Title for Game {
    type Id = GameId;

    fn id(&self) -> GameId {
        self.id
    }

    fn main_file(&self) -> &'static str {
        self.main_file
    }

    fn editions(&self) -> &'static [Edition] {
        self.editions
    }
}

const fn sha(d: &'static str, label: &'static str) -> Edition {
    Edition::sha256(d, label)
}

pub const GAMES: &[Game] = &[
    Game {
        id: GameId::FZeroAx,
        slug: "fzax",
        title: "F-Zero AX Monster Ride",
        short: "F-ZERO",
        year: 2004,
        board: "Sega Triforce",
        blurb: "Amusement Vision's arcade twin to F-Zero GX on Sega/Nintendo/Namco Triforce hardware — 10 exclusive pilots, 6 tracks and hydraulic Monster Ride motion.",
        route: Route::Dolphin,
        main_file: "F-Zero AX (Triforce) (Monster) [JAP][BHA6].iso",
        editions: &[sha(
            "280011bf67604a8041bfac3e54a142b56531fad67323bf4a52dc9547a4d33961",
            "Japan (SBHA6W)",
        )],
        native: Size::new(640, 480),
        support: Support::Unverified("not yet raced through the launcher"),
        // The Monster Ride cabinet's motion seat (CyCraft, on the cabinet's
        // network) is not emulated: without it the game stays on
        // "Searching cycraft". The community's "Bypass Cycraft" code
        // (Internet Archive, "F-ZERO AX Monster Ride (Decrypted/for Dolphin)").
        dolphin_id: "GHAE6E",
        seeds: &[],
        patches: &[Patch::Gecko {
            name: "Bypass CyCraft (no motion seat)",
            lines: &["0043137B 00000086"],
        }],
    },
    Game {
        id: GameId::MarioKartGp2,
        slug: "mkgp2",
        title: "Mario Kart Arcade GP 2",
        short: "MKGP2",
        year: 2007,
        board: "Sega Triforce",
        blurb: "Namco's second Triforce Mario Kart with Waluigi, Mametchi, commentary, 24 courses across 6 cups and a built-in cabinet camera.",
        route: Route::Dolphin,
        main_file: "Mario Kart Arcade GP 2 (MK2100-4-NA-MPRO-B00) (20-08-2007 094721) (E).iso",
        editions: &[sha(
            "bf14a7c35efe1bda690adc5afdf30a6a0106cf89c4b61e8048f449f8c94ace01",
            "MK21 Ver. A (2007-08-20)",
        )],
        native: Size::new(640, 480),
        support: Support::Unverified("not yet raced through the launcher"),
        dolphin_id: "GNLJ82",
        // The cabinet's camera (it photographs the player for the card) is not
        // emulated: the boot's device check stops on CAMERA ERROR. Dolphin's
        // own "Skip Boot Check" code for this disc goes past that check.
        seeds: &[],
        patches: &[Patch::ShippedActionReplay(
            "(2007/02/06 20:29:25) Skip Boot Check [mamefan2018]",
        )],
    },
    Game {
        id: GameId::MarioKartDx,
        slug: "mkdx",
        title: "Mario Kart Arcade GP DX",
        short: "MKDX",
        year: 2013,
        board: "Namco ES3",
        blurb: "HD widescreen arcade Mario Kart on Namco ES3 hardware, introducing gliders, underwater driving, fusion karts and Don-chan.",
        route: Route::Wal {
            system: "namcoes3",
            profile: "mario-kart-arcade-gp-dx",
            layout: StickLayout::NamcoEs3Kart,
        },
        main_file: "MK_AGP3_FINAL.exe",
        editions: &[sha(
            "3c4862309b8a2943b46d9b917f1a28fb900691bbf6201280429e6817e861b785",
            "1.18.16 (English mod 1.0)",
        )],
        native: Size::new(1360, 768),
        support: Support::Unverified("not yet raced through the launcher"),
        dolphin_id: "",
        patches: &[],
        // The dump's operator settings (TeknoParrot's, with the Namcam
        // camera and the IC card units off) on the cabinet's F: and G:
        // drives: with the game's own defaults it stops on E08-01 NAMCAM
        // (CAMERA) ERROR, and windowsarcadeloader emulates no camera.
        seeds: &[("FTestMode", "F/TestMode"), ("GTestMode", "G/TestMode")],
    },
    Game {
        id: GameId::LuigisMansion,
        slug: "luigi",
        title: "Luigi's Mansion Arcade",
        short: "LUIGI",
        year: 2015,
        board: "Sega Nu",
        blurb: "Capcom and Sega's lightgun Poltergust rail shooter based on Luigi's Mansion: Dark Moon, built on Sega Nu hardware.",
        route: Route::Wal {
            system: "seganu",
            profile: "luigis-mansion-arcade",
            layout: StickLayout::SegaNuGun,
        },
        main_file: "exe/x64/VACUUM.exe",
        editions: &[sha(
            "3dfef015e8657e21046af512561618609b0d230bfe8d5cbd2bf54c101c76e2db",
            "SDCF (Nu 1.1)",
        )],
        native: Size::new(1920, 1080),
        support: Support::Unverified("not yet played through the launcher"),
        dolphin_id: "",
        patches: &[],
        seeds: &[],
    },
];

pub fn game(id: GameId) -> &'static Game {
    GAMES
        .iter()
        .find(|g| g.id == id)
        .expect("every id is in GAMES")
}

pub fn by_slug(slug: &str) -> Option<&'static Game> {
    GAMES.iter().find(|g| g.slug == slug)
}

/// The sizes of the known Triforce disc images (F-Zero AX's 436 029 440
/// bytes, Mario Kart GP 2's 536 870 912): an image under another name is
/// hashed only at such a size, so a stray disc image of another system is
/// never read whole.
pub const DISC_SIZES: RangeInclusive<u64> = 436_029_440..=536_870_912;

/// The scan of a player's library folders: each game by its main file,
/// and a Triforce disc image renamed by the player by its digest.
pub fn library() -> Library<Game> {
    Library::new(GAMES).with_extra(Extra {
        extensions: &[".iso", ".gcm"],
        sizes: DISC_SIZES,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;

    use rea_launcher::catalog::{check_table, identify};
    use rea_launcher::library::Found;

    #[test]
    fn ids_slugs_and_digests_are_unique_and_well_formed() {
        assert_eq!(check_table(GAMES), Ok(()));
        let slugs: HashSet<_> = GAMES.iter().map(|g| g.slug).collect();
        assert_eq!(slugs.len(), GAMES.len());
    }

    #[test]
    fn identify_finds_the_right_game() {
        let (g, _) = identify(
            GAMES,
            "BF14A7C35EFE1BDA690ADC5AFDF30A6A0106CF89C4B61E8048F449F8C94ACE01",
        )
        .unwrap();
        assert_eq!(g.id, GameId::MarioKartGp2);
        assert!(identify(GAMES, "00").is_none());
    }

    #[test]
    fn an_unknown_build_is_reported_with_its_digest() {
        let t = tempfile::tempdir().unwrap();
        let g = t.path().join("Some Game [TP]");
        fs::create_dir(&g).unwrap();
        fs::write(g.join("MK_AGP3_FINAL.exe"), b"not the game").unwrap();
        fs::create_dir(t.path().join("empty")).unwrap();
        let scan = library().scan(&[t.path().to_path_buf()]);
        assert_eq!(scan.entries.len(), 1);
        assert!(
            matches!(&scan.entries[0].found, Found::Unknown { file, .. } if file == "MK_AGP3_FINAL.exe")
        );
    }

    /// A disc image under another name is read only at a known disc's
    /// size: every .iso and .gcm of a library was hashed whole on each
    /// start and `--list`. An unreadable stray image shows it: a read
    /// would be reported as a problem.
    #[test]
    #[cfg(unix)]
    fn a_stray_disc_image_is_not_read() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let stray = t.path().join("holiday.iso");
        fs::write(&stray, b"junk").unwrap();
        fs::set_permissions(&stray, fs::Permissions::from_mode(0o000)).unwrap();
        let scan = library().scan(&[t.path().to_path_buf()]);
        assert!(scan.entries.is_empty(), "{scan:?}");
        assert!(scan.problems.is_empty(), "it was read: {scan:?}");
        assert!(DISC_SIZES.contains(&436_029_440) && DISC_SIZES.contains(&536_870_912));
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
        let scan = library().scan(&roots);
        assert_eq!(scan.by_game().len(), GAMES.len(), "{scan:#?}");
    }

    #[test]
    fn the_triforce_games_run_in_dolphin_and_dx_in_windowsarcadeloader() {
        assert_eq!(game(GameId::FZeroAx).route, Route::Dolphin);
        assert_eq!(game(GameId::MarioKartGp2).route, Route::Dolphin);
        assert!(matches!(
            game(GameId::MarioKartDx).route,
            Route::Wal {
                system: "namcoes3",
                ..
            }
        ));
        assert_eq!(game(GameId::MarioKartDx).loader_path(), "MK_AGP3_FINAL.exe");
        assert_eq!(
            game(GameId::LuigisMansion).loader_path(),
            "exe\\x64\\VACUUM.exe"
        );
        assert!(game(GameId::LuigisMansion).aims() && !game(GameId::MarioKartDx).aims());
    }
}
