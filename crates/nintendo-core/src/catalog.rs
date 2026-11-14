//! The games, what runs each one, and the builds we know.
//!
//! An edition is recognised by the SHA-256 of its main file (the disc image
//! of a Triforce game, the executable of the Namco ES3 game). Only builds
//! listed here are launched; anything else is reported with its digest so
//! the player can file it (STANDARDS: "Exact binary identity").

use crate::settings::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameId {
    FZeroAx,
    MarioKartGp2,
    MarioKartDx,
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
    },
}

/// Whether the launcher can start a game today, and why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Verified in the real game (a race played), see docs/RUNTIME_NOTES.md.
    Ready,
    /// Starts, but no race was verified yet; the text says what is known.
    Unverified(&'static str),
    /// Known not to work yet; the text says why, for the player.
    Blocked(&'static str),
}

/// The system the launcher runs on: what it can start depends on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Linux,
    Windows,
}

impl Host {
    pub const fn current() -> Host {
        if cfg!(windows) {
            Host::Windows
        } else {
            Host::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edition {
    /// SHA-256 of the main file.
    pub sha256: &'static str,
    pub label: &'static str,
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
        self.main_file.to_string()
    }
}

const fn sha(d: &'static str, label: &'static str) -> Edition {
    Edition { sha256: d, label }
}

pub const GAMES: &[Game] = &[
    Game {
        id: GameId::FZeroAx,
        slug: "fzax",
        title: "F-Zero AX Monster Ride",
        short: "F-ZERO",
        year: 2004,
        board: "Sega Triforce",
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
        route: Route::Wal {
            system: "namcoes3",
            profile: "mario-kart-arcade-gp-dx",
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

/// The game and edition a digest belongs to.
pub fn identify(digest: &str) -> Option<(&'static Game, &'static Edition)> {
    GAMES.iter().find_map(|g| {
        g.editions
            .iter()
            .find(|e| e.sha256.eq_ignore_ascii_case(digest))
            .map(|e| (g, e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_slugs_and_digests_are_unique_and_well_formed() {
        let ids: HashSet<_> = GAMES.iter().map(|g| g.id).collect();
        let slugs: HashSet<_> = GAMES.iter().map(|g| g.slug).collect();
        assert_eq!(ids.len(), GAMES.len());
        assert_eq!(slugs.len(), GAMES.len());
        let mut digests = HashSet::new();
        for g in GAMES {
            for e in g.editions {
                assert_eq!(e.sha256.len(), 64, "{}", g.slug);
                assert!(e.sha256.chars().all(|c| c.is_ascii_hexdigit()));
                assert!(digests.insert(e.sha256), "duplicate digest");
            }
        }
    }

    #[test]
    fn identify_finds_the_right_game() {
        let (g, _) =
            identify("BF14A7C35EFE1BDA690ADC5AFDF30A6A0106CF89C4B61E8048F449F8C94ACE01").unwrap();
        assert_eq!(g.id, GameId::MarioKartGp2);
        assert!(identify("00").is_none());
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
    }
}
