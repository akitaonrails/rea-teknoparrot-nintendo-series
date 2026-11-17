# Changelog

All notable changes to this launcher. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

### Changed

- Updated rea-launcher-sdk to v0.10.0: the window, the command line, the
  game sessions, the controller input and the packaging are now the SDK's
  shared hub, the same as the Initial D, Sega and Wangan launchers'.
- The window works with a pad: the d-pad or left stick moves, A chooses,
  B goes back, Start plays, Y opens Settings, the shoulder buttons change
  tabs. While a game runs the pad belongs to the game.
- On Windows the Settings page shows the screen-size, upscaler, sharpness
  and full-screen rows greyed out with the reason (they are for gamescope
  on Linux) instead of hiding them.
- A new game settings file starts with a comment above each option.
- New `--check [<game>]` says whether a game (or every game) can start
  without starting it; `--set <game> <options>` saves options (`--game`
  still works, and now refuses to run without options instead of doing
  nothing).
- Only one launcher runs at a time; `--list` still works while the window
  is open.

### Fixed

- Closing the launcher (or killing it) on Linux no longer leaves the game
  running.
- Stopping a game on Windows asks Dolphin to close its window, so it saves;
  it was ended after 8 seconds. On Linux Dolphin is asked on its own and
  gets the time to save before anything else stops.
- `--no-input` no longer makes Mario Kart DX and Luigi's Mansion quit at
  once.
- A game that closes by itself right after starting (Windows refusing a
  program, a missing runtime) now says so, with the log's error lines.
- Mario Kart DX and Luigi's Mansion take a free input port at each start
  (a port held by another program stopped them).
- A settings file that can't be read is no longer overwritten with the
  defaults; a value the launcher can't read stays in the file as you wrote
  it (and is reported in `launcher.log`); a file saved by Notepad with a
  byte-order mark no longer loses its first setting; `deadzone = 0.125`
  stays as written.
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
- A failed Windows job assignment no longer stops a game from starting;
  the force feedback lets go of the wheel when a game's input breaks.
- A download cut short while building the packages no longer wedges the
  next build; a release now waits for the checks and the Windows tests.

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
