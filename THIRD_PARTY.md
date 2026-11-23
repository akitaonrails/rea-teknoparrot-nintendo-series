# Third-party components and credits

Nothing from the games is in this repository or its packages. The player
points the launcher at their own dumps; the launcher reads them in place,
read-only.

## Components the launcher uses

| Component | What it does here | Licence | How it reaches the player | Pin |
|---|---|---|---|---|
| [Dolphin](https://github.com/dolphin-emu/dolphin) | Runs F-Zero AX Monster Ride and Mario Kart Arcade GP 2 (Sega Triforce emulation, mainline since 2603) | GPL-2.0-or-later (parts BSD-3-Clause, MIT, CC0; its `LICENSES/`) | Linux: `dolphin-emu-nogui` built from source, with the libraries it needs that a desktop may lack, inside the package; Windows: the project's release, unmodified, inside the package | tag `2609b` (commit `3a5b3743b18da3e8f04cbc44f20dc50fbd3a9ef3`); Windows `dolphin-2609b-x64.7z` sha256 `926eb7594d885b4fba8fd9807058d90618efab4b3d75d1ef297022ba14925133` |
| [windowsarcadeloader](https://github.com/aderumier/windowsarcadeloader) (aderumier) | Runs Mario Kart Arcade GP DX and Luigi's Mansion Arcade (under Wine on Linux; on Windows through its Windows host build, `patches/wal/0002`): the dongles, the I/O boards (Luigi's Mansion's vacuum guns), an unlocked-profile card server, force feedback; on Linux it reads mice and lightguns (`patches/wal/0003`) | GPL-3.0 | built from source with `patches/wal` (`scripts/build-components.sh wal`), inside both packages | commit `4f42c6d86ddaaa9d2e4e8c21e0046bb3027b111d` |
| [GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom) 11-13 | The Wine windowsarcadeloader runs the Windows games with | BSD-3-Clause / LGPL (Wine) and others, see its `LICENSE` | inside the Linux package (downloaded at build time, checked by SHA-256: `packaging/components.pins`) | `GE-Proton11-13-x86_64.tar.gz` sha256 `2761e8efec78e37f12812269d6157c3dd52f8bfed34568a2d4fcac3ca1b9ff22` |
| [winetricks](https://github.com/Winetricks/winetricks) | Installs the game's Microsoft runtimes (Visual C++ 2010, D3DX, DirectMusic) into the Wine prefix on first start (windowsarcadeloader runs it) | LGPL-2.1 | inside the Linux package | tag `20260125`, sha256 `431f82fc74000e6c864409f1d8fb495d696c03928808e3e8acffc45179312a7b` |
| Microsoft Visual C++ 2010, DirectX (D3DX, DirectMusic) runtimes | The Windows game's own dependencies | Microsoft redistributables | downloaded by winetricks on first start (Linux) | winetricks verbs |
| SDL 2.32.10 (MinGW), Debian's libsdl2 | The launcher's pads and wheels | zlib | `SDL2.dll` (Windows) / `lib/libSDL2-2.0.so.0` (Linux) inside the package | the SDK's `sdk/packaging/sdl2.pins` |
| Barlow, Barlow Condensed (Jeremy Tribby) | The window's type, embedded in the program by rea-launcher-sdk | SIL OFL 1.1 | inside the program; the licence in the package's `licenses/Barlow-OFL.txt` | rea-launcher-sdk |
| gamescope, bubblewrap | Full-screen display, scaling (FSR/NIS); the library bound read-only around every game | BSD-2-Clause, LGPL-2.0 | the player's system | — |

## Code we wrote on top of others

| What | Where | Licence |
|---|---|---|
| `--input-pipe` for windowsarcadeloader's launcher (the hub feeds the virtual stick and receives force feedback) | `patches/wal/0001-launcher-input-pipe.patch` (from the Initial D launcher) | GPL-3.0 (as upstream) |
| windowsarcadeloader's Windows host build (its run on Windows itself) | `patches/wal/0002-windows-host.patch` (from the Initial D launcher) | GPL-3.0 (as upstream) |
| CRT effect | `assets/shaders/crt-lottes.fx` (from the Initial D launcher) | GPL-3.0-or-later |

## Codes and data we follow

| Project | What we used | Licence |
|---|---|---|
| "F-ZERO AX Monster Ride (Decrypted/for Dolphin)", Internet Archive | The "Bypass Cycraft" Gecko code (`0043137B 00000086`) the launcher enables for Monster Ride | community code, a single byte write |
| Dolphin `Data/Sys/GameSettings/GNLJ82.ini` (mamefan2018) | Its "Skip Boot Check" code for Mario Kart GP 2, enabled by name | GPL-2.0-or-later (Dolphin) |
| TeknoParrotUI GameProfiles (teknogods) | Which inputs each cabinet has (read to choose ours; nothing copied) | GPL-3.0 |

## Thanks

The Dolphin team and crediar (Triforce emulation), aderumier
(windowsarcadeloader), GloriousEggroll (GE-Proton), the bananatools
authors, and teknogods.
