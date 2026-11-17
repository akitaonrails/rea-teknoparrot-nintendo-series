# rea-teknoparrot-nintendo-series — instructions for coding agents

One launcher (a hub, like the Initial D and Wangan Midnight ones) for the
Nintendo arcade games the owner dumped: F-Zero AX Monster Ride and Mario
Kart Arcade GP 2 (Sega Triforce), Mario Kart Arcade GP DX (Namco ES3),
Luigi's Mansion Arcade (Sega Nu, a gun game; added 2026-11-15 on the
owner's request).
Built on rea-launcher-sdk (vendored in `sdk/` by its `scripts/sdk-sync`).
Read sdk/docs/STANDARDS.md before any work; the playbooks are in
sdk/docs/playbooks/. Then `docs/RESEARCH.md` here. Sister projects:
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

## Layout and checks (2026-11-17: moved onto the SDK's hub modules)

- `crates/nintendo-core`: the game knowledge, host-tested: `catalog`
  (games as `rea_launcher::catalog::Title`, `library()` with the disc-image
  size bound), `settings` (the `cfg::spec` OPTIONS table; stored formats
  kept), `runtime` (the `linkfarm::Policy`), `input` (cabinet buttons, the
  windowsarcadeloader bit tables, Dolphin's pipe pad), `launch` (plans).
- `crates/nintendo-launcher`: `ui.rs` (the `rea_launcher_ui::hub`
  backend: theme, form, session, controls), `session.rs` (prepare, and the
  run through `rea_launcher_native::{group, supervise}` with the
  Dolphin-asked-alone watcher), `input.rs` (the feeder `Wire`s and
  Dolphin's FIFO sink), `paths.rs`, `main.rs` (`hub::cli`, instance lock,
  `hub::fatal`). Everything else is the SDK's: fix it there, never copy it.
- Checks before a commit: `cargo fmt --all --check`, `cargo clippy
  --workspace --all-targets -- -D warnings` (and with `--target
  x86_64-pc-windows-gnu`), `cargo test --workspace`,
  `sdk/scripts/wine-test --target x86_64-pc-windows-gnu --locked
  --workspace`, `sdk/scripts/launcher-check`. Golden snapshots
  (`crates/nintendo-launcher/tests/snapshots/`): regenerate with
  `REA_UPDATE_SNAPSHOTS=1` only after reading every diff.
- Packaging: `sdk/packaging/build-launcher.sh`, `scripts/build-components.sh`
  (downloads pinned in `packaging/components.pins`, fetched with the SDK's
  `pins.sh`), `scripts/package.sh`. CI and releases: `.github/workflows/build.yml`
  (the SDK's launcher-ci and launcher-release; a release waits for CI, the
  Wine tests and the packages).
