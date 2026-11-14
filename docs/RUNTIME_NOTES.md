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

| Check | Seen |
|---|---|
| First start | the prefix is built, then winetricks downloads Microsoft's runtimes; `dmusic32` timed out downloading `directx_apr2006_redist.exe` from web.archive.org (the archive was unreachable that afternoon) and the start ended there |

## Not checked yet

- A real wheel, and the synthetic centring force on it (Triforce games).
- Dolphin on Windows (Dolphin.exe, the XInput/SDL mapping).
- Sound on speakers (only a null sink).
