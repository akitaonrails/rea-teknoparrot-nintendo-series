# Changelog

All notable changes to this launcher. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

### Added

- Controls: change a button. Choose a row of the pad layout (Gas,
  Brake, a cabinet button), then press the pad button or trigger it
  should use; Reset goes back to the launcher's layout. The layout is
  saved in the data folder's `controls.cfg` and used by the window and
  `--play`.

### Changed

- Updated rea-launcher-sdk to v0.11.1.

### Fixed

- Controls: pressing the pad's buttons to try them on the Controls page
  switched pages (LB/RB) or left it (B). The pad's presses now only light
  up the live check there; press Back and Start together to move through
  the menu with the pad, and "Test the pad" to try the buttons again.
- A PXN wheel (the VD6 and other models with PXN's own USB ID) was
  listed as "Other" and nothing drove the car ("No controller"); it is a
  wheel now.

## 0.2.1 - 2026-11-18

The first published release with everything in 0.2.0 below: 0.2.0 was
tagged, but its release build stopped on CI setup problems (no
`deny.toml` for the dependency audit, and the Windows tests needed a Wine
tool the pinned GE-Proton doesn't ship), so it was never published.

### Fixed

- CI: the dependency audit has its configuration, and the Windows tests
  run through the pinned GE-Proton's `wine`, as the Sega launcher's do.

## 0.2.0 - 2026-11-18

### Fixed

- Closing the launcher (or killing it) on Linux no longer leaves the game
  running.
- Stopping a game asks it to close first, so it saves: on Windows every
  window of the game gets the close (Dolphin saved nothing and every stop
  waited 8 seconds); on Linux Dolphin is asked on its own and gets the
  time to save before anything else stops.
- `--no-input` no longer makes Mario Kart DX and Luigi's Mansion quit at
  once.
- A game that closes by itself right after starting (Windows refusing a
  program, a missing runtime) now says so, with the log's error lines.
- Mario Kart DX and Luigi's Mansion take a free input port at each start
  (a port held by another program stopped them).
- A settings file that can't be read is no longer overwritten with the
  defaults; a value the launcher can't read stays in the file as you wrote
  it (and is reported in `launcher.log`); a file saved by Notepad with a
  byte-order mark no longer loses its first setting.
- `launcher.cfg` keeps your comments and other lines, and a folder listed
  twice is saved once.
- Starting a game or `--list` no longer reads every disc image in your
  folders in full: only images the size of a known disc are checked. A
  rescan during a scan no longer starts over.
- Every error is also written to `launcher.log`, which the messages name;
  an error before the window opens is shown in a window too.
- Paths show the system's separator on Windows; `~` in a typed path works
  on Windows too; an empty or relative `XDG_DATA_HOME`/`LOCALAPPDATA` no
  longer puts the launcher's data in the current folder.
- A library folder that holds the launcher's own data (your home folder)
  no longer stops the games from writing their saves.
- A failed Windows job assignment no longer stops a game from starting;
  the force feedback lets go of the wheel when a game's input breaks.

### Added

- The window works with a pad: the d-pad or left stick moves, A chooses,
  B goes back, Start plays, Y opens Settings, the shoulder buttons change
  tabs. While a game runs the pad belongs to the game.
- Looks, as in the family's other launchers: **Original** (the game as
  shipped), **Recommended** (as large as your screen allows, with FSR) and
  **Steam Deck**, in the Settings page and as `--look`. The first time the
  window reads a game's settings, the look for your screen is chosen and
  saved (on Windows the command line does this too). A Linux command line
  doesn't know your screen: it plays the Recommended look for 1920×1080
  for that run and saves no look, so the window chooses it later.
  Changing a picture option by hand, or giving a picture option with
  `--look` (`--look steam_deck --upscaler nis`), makes the look Custom.
- On Windows the Settings page shows the screen-size, upscaler, sharpness
  and full-screen rows greyed out with the reason (they are for gamescope
  on Linux) instead of hiding them.
- `--check [<game>]` says whether a game (or every game) can start without
  starting it; `--set <game> <options>` saves options (`--game` still
  works, and now refuses to run without options instead of doing
  nothing).
- Only one launcher runs at a time; `--list` still works while the window
  is open.

### Changed

- If you have no settings file for a game yet, it starts with the
  Recommended look (or Steam Deck on a Deck) instead of the old fixed
  defaults (4K output, the cabinet's own size).
- A settings file holds only what you changed, the rest as commented lines
  with a note above each option; `--set` saves only the options you gave.
- A dead zone written with more than two decimals is used rounded to two
  (`deadzone = 0.125` plays as 0.13); the line stays as you wrote it.
- On Windows, a file in a game's private folder that differs from your
  game folder and wasn't copied there by the launcher is now refused, with
  a message naming it, instead of being overwritten.
- Packages are built from pinned downloads that a cut-short download can
  no longer wedge, and a release waits for the checks and the Windows
  tests.
- Updated rea-launcher-sdk to v0.10.0: the window, the command line, the
  game sessions, the controller input and the packaging are now the SDK's
  shared hub, the same as the Initial D, Sega and Wangan launchers'.

## 0.1.0 - 2026-11-15

The first release: one launcher for F-Zero AX Monster Ride, Mario Kart
Arcade GP 2, Mario Kart Arcade GP DX and Luigi's Mansion Arcade.

### Added

- No TeknoParrot: mainline Dolphin (Triforce) and windowsarcadeloader, from
  pinned sources, inside the packages (with GE-Proton and winetricks on
  Linux).
- Monster Ride without its motion seat, both Mario Karts without their
  camera.
- Luigi's Mansion with a pad (the stick moves the crosshair); on Linux a
  mouse or a lightgun too.
- Pads and wheels read once for every game; on Linux the Triforce games get
  them through Dolphin's pipe input, with full steering lock and a centring
  spring on force-feedback wheels.
- Render sizes up to 4K; on Linux FSR 1, NIS and full screen through
  gamescope.
