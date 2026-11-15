# rea-teknoparrot-nintendo-series — instructions for coding agents

One launcher (a hub, like the Initial D and Wangan Midnight ones) for the
Nintendo arcade games the owner dumped: F-Zero AX Monster Ride and Mario
Kart Arcade GP 2 (Sega Triforce), Mario Kart Arcade GP DX (Namco ES3),
Luigi's Mansion Arcade (Sega Nu, a gun game; added 2026-11-15 on the
owner's request).
Built on `~/Projects/rea-launcher-sdk`; read its `docs/STANDARDS.md`
(binding) and `docs/RESEARCH.md` here first. Sister projects:
`rea-teknoparrot-initial-d-series` (this repository started from its
code), `rea-teknoparrot-wangan-series`.

## Decisions (2026-11-14, see docs/RESEARCH.md §5)

- **Runtime**: Dolphin (mainline, Triforce since 2603) for the Triforce
  games, driven through its pipe input on Linux; windowsarcadeloader under
  Wine for Mario Kart DX and Luigi's Mansion, fed over `--input-pipe`
  (patches/wal; 0003 lets its own mouse/lightgun reading aim over the
  pipe). No
  TeknoParrot.
- **Packages carry everything**: Dolphin is built from its tag for Linux and
  taken from the project's release for Windows; GE-Proton, winetricks and
  windowsarcadeloader as in the sister projects.
- **Cabinet hardware a PC lacks** is skipped with Dolphin game settings
  (a Gecko code, a code Dolphin ships), each listed in RESEARCH §4.
- **DRM exception, licence, art**: as in the sister projects (maintained
  community loaders that emulate dongles and card readers; GPL-3.0;
  original illustrations from `scripts/art/scenes.py`).

## Rules that bite here

- The library (`/mnt/terachad/Emulators/EmuDeck/roms_rare/TeknoParrot`) is
  NFS and read-only. Runtime = per-game private folder (Dolphin's user
  folder, or links to the dump + real files for what the game writes).
- No option ships until its effect was seen in the real game; record it in
  `docs/RUNTIME_NOTES.md`. Game runs are headless (`--headless`) unless the
  owner is told first; `.work/probe/run.sh` (not committed) takes the
  screenshots.
