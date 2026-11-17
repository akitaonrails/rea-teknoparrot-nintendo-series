# Runtime notes

What was run against the real games, how, and what was seen. Observations
only; anything not listed here has **not** been verified.

Machine: Arch (Omarchy) host, `gaming` distrobox, NVIDIA RTX 5090. Game
runs are headless: the launcher's own `--play <game> --headless`
(gamescope 3.17 `--backend headless`), screenshots with
`GAMESCOPE_WAYLAND_DISPLAY=<its socket> gamescopectl screenshot`, a private
PipeWire null sink for sound, inputs from `--input-script` through the
launcher's own pipe. Every run is wrapped in `bwrap --dev-bind / /
--ro-bind <library> <library>`: the library is read-only for the game.
Probe script: `.work/probe/run.sh` (not committed). Dolphin: 2609b built
by `scripts/build-components.sh dolphin` (`dolphin-emu-nogui`, Vulkan).

## F-Zero AX Monster Ride — Dolphin (2026-11-14)

| Check | Seen |
|---|---|
| Boots, id `GHAE6E` | yes: "Motor Initializing", then the handle/pedal adjustment screen (about 42 s after start) |
| Steering through the pipe, full travel | `HANDLE LEFT 127`, `HANDLE RIGHT 127` with `steer=-1` / `steer=1` (stick gate at 100 %) |
| Pedals through the pipe | `GAS 255`, `BRAKE 255` |
| Start after the adjustment | **not taken** while `HANDLE UP/DOWN` read 0; taken once the pitch axis (stick Y) moved both ways |
| After the adjustment, without the seat | stays on "Searching cycraft" (2 min) |
| With the "Bypass Cycraft" Gecko code (`GameSettings/GHAE6E.ini`, cheats on) | attract ("F-ZERO AX MONSTER RIDE", INSERT COIN) at about 70 s |
| Coin (pipe's Z → `Triforce/Coin`), Start, gas | **race played**: 1/8 lap, 596 then 681 km/h, energy bar, position 30/30 → 28 |
| Stop (`--seconds`) | Dolphin ends on SIGTERM ("A signal was received"), gamescope with it, no leftover process |

The adjustment is asked on every start so far (Dolphin writes the
baseboard backup when it shuts down; the runs here are stopped by
signal): a player does it with the wheel or stick, pushes the pad's left
stick (or W/Q) up and down, presses both pedals, then Start.

## Mario Kart Arcade GP 2 — Dolphin (2026-11-14)

| Check | Seen |
|---|---|
| Boots, id `GNLJ82` | yes: Device Check HANDLE OK, BUTTON OK, CARD OK, CAMERA CHECKING |
| Without a camera | "Please call an attendant — CAMERA ERROR" (80 s) |
| With Dolphin's shipped "Skip Boot Check" code | link check of PCB 2–4 (about a minute, counts down), then the attract ("Please insert 1 coin") |
| Dolphin's "Skip Link Check" code | no change to the link check: not used |
| Coins, gas pedal to start | CREDIT(S) 2/1, "Step on gas pedal to start" taken |
| Menus | card prompt → NO (steer right + Start), class select, cup select; the menus decide with the gas pedal |
| Race | **race played**: Mario, Yoshi Cup round 1, lap 1/3, 89 km/h, steering both ways |

## Mario Kart Arcade GP DX — windowsarcadeloader (2026-11-14)

Components: windowsarcadeloader at the pinned commit with `patches/wal`,
GE-Proton 11-12, winetricks 20260125.

| Check | Seen |
|---|---|
| First start with windowsarcadeloader's default prefix | stopped: winetricks `dmusic32` downloads `directx_apr2006_redist.exe` from web.archive.org, unreachable that day (Microsoft's own URL is gone, 404) |
| Prefix with only the profile's `tricks` (vcrun2010, d3dx9_43, d3dx10_43) | the game starts, then reads a null pointer at `MK_AGP3_FINAL.exe+0x435e60` a second after its window opens (the stack holds the font name "ＭＳ ゴシック"); `fakejapanese` + `xact` alone did not change it |
| Default prefix list without `dmusic32` and `gmdls` (the two archive.org verbs; `WAL_PREFIX_TRICKS`) | prefix built in about 3 minutes from cached and Microsoft/GitHub downloads; the game boots to its own boot check (power on, drive unit, JVS I/O connected) |
| Headless gamescope with the game presenting through gamescope's Vulkan layer | gamescope aborts on the first frame (`vkImportSemaphoreFdKHR failed`, `vkAllocateMemory failed`, assertion in `acquire_pooled_texture`), three runs in a row; Dolphin in the same gamescope is fine |
| `ENABLE_GAMESCOPE_WSI=0` (the launcher's environment for windowsarcadeloader on Linux) | no abort |
| The game's own operator settings | boot stops on **E08-01 NAMCAM (CAMERA) ERROR** |
| The dump's `FTestMode`/`GTestMode` settings seeded once onto the F:/G: drives (camera and IC card units off) | **race played**: free play, character select with the item button (pipe `b5`), item preview, "KartKing" unlocked profile from windowsarcadeloader's card server, lap 1/2 holding a Spiny Shell |

## Luigi's Mansion Arcade — windowsarcadeloader (2026-11-15)

Components: windowsarcadeloader at the pinned commit with `patches/wal`
0001–0003 (`arcade-launcher` built locally), the same Wine prefix as Mario
Kart DX.

| Check | Seen |
|---|---|
| Runtime with `exe` linked to the dump | stopped: windowsarcadeloader creates its save folder next to the program (`exe/x64/sv`: "Read-only file system") |
| The folders on the way to the program real, their entries linked | starts: payload connected, "Loading...", then the game's attract and FREE PLAY (about 90 s) |
| windowsarcadeloader's input port 33700 | "Address already in use" twice while a sister launcher's game ran on the same machine; the launcher's own port (33790) since; Mario Kart DX's payload connects on it too |
| A run with mice read (no `guns_enabled: false`) | a mouse at the desk (not the test's input) joined as **Player 2**, picked a stage and played: windowsarcadeloader's mouse reading reaches the game over the pipe (0003). Aim not checked on purpose. Headless runs read no mouse since |
| The launcher's pipe only (script: start, the trigger through the menus, then the aim swept while the trigger is held) | **stage played**: "Player 1" joined, Normal Controls, the stage's intro, then Gloomy Manor's first room: the crosshair where the script put it (x 0.5, y −0.25 → about 1440, 405 of 1920×1080), a ghost vacuumed, score 18 |

The live pad crosshair (a stick moving it, `input::Aim`) is covered by
tests only; the script places it directly.

## The release package (2026-11-14)

The Linux package from the Release workflow (run 43645268157, Debian
bookworm, Dolphin built there) unpacked and run as is, with a fresh data
folder: the package's own `nintendo-launcher`, `components/dolphin` and
its `lib/` → F-Zero AX Monster Ride calibrated and **raced** (664 km/h).
Its programs need glibc 2.35 at most; `ldd` finds every library Dolphin
needs on the system or in `components/dolphin/lib` (libevdev, libbluetooth,
libusb).

## On rea-launcher-sdk's hub modules (2026-11-17)

The launcher after its move onto the SDK's hub (window, command line,
process group and run loop, input thread, runtime folder of links), built
against the SDK's `feat/hub` branch, with a fresh data folder: the
launcher's `--play <game> --headless` in the `gaming` box, the library
bound read-only, inputs from `--input-script`. After every run the box
was checked for the session's programs (gamescope, Dolphin,
windowsarcadeloader, the launcher's Wine server).

| Game, how it ended | Seen |
|---|---|
| F-Zero AX, `--seconds 120` (the window's Stop) | calibration, attract, coin and Start: the race's start line (1/8 lap, 120 s); Dolphin alone got the signal ("A signal was received"), saved and quit, then gamescope; `# exit: … after 120 s; stopped 0 leftover process(es)`; nothing left |
| F-Zero AX, Dolphin ended by itself at 45 s (the game window closed) | the session ended on its own, `stopped 0 leftover`; no early-end error (after 20 s); nothing left |
| F-Zero AX, the launcher killed (SIGKILL) at 45 s | gamescope and Dolphin gone 3 s later (the session dies with the launcher) |
| Mario Kart DX, first start, `--seconds 450` | the Wine prefix built (winetricks), payload connected on port 33790 (the first free block), attract in FREE PLAY at 420 s; `stopped 0 leftover`; nothing left |
| Mario Kart DX, the input script, SIGTERM to the launcher at 150 s (quit from a terminal) | coins, Start, character and cup chosen through the launcher's pipe: "Select Round", Toad Cup; the launcher stopped the session on the signal, `stopped 0 leftover`; nothing left |
| Mario Kart DX, `--no-input`, the launcher killed at 100 s | the game kept running with no input (the loader's input is held open; it quit at once before), "Press item button" at 90 s; after the kill no windowsarcadeloader, Wine server or gamescope was left |

Looks (2026-11-17): `--set fzax --look steam_deck`, then `--play fzax
--headless`: gamescope `-w 1280 -h 960 -W 1280 -H 800 -F linear`, Dolphin
`InternalResolution = 2`; `--look original`: `-w 640 -h 480 -W 1920 -H
1080 -F linear`, `InternalResolution = 1`. Both runs were stopped at 25 s,
while Dolphin was still booting: it didn't answer the stop within the 8 s
grace and was ended, nothing left behind (stopped at 120 s, in a race, it
saves and quits by itself).

Not run headless: closing the hub window itself (it needs a display);
the window's close-stops-the-game path is covered by the SDK's window
tests, and the quit from the game's own menu doesn't exist on these
cabinets (the window-close row above is Dolphin ending by itself).

## Not checked yet

- A real wheel, and the synthetic centring force on it (Triforce games).
- Luigi's Mansion with a real pad, a mouse on purpose, or a lightgun; its
  second player.
- Dolphin on Windows (Dolphin.exe, the XInput/SDL mapping).
- Sound on speakers (only a null sink).
