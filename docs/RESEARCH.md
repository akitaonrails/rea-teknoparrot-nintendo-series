# Research — Nintendo arcade launcher

Phase 1 of `rea-launcher-sdk/docs/playbooks/new-launcher.md`. Observations,
inferences and decisions are kept apart. Read from the files, upstream
repositories and profiles named in each row on 2026-11-14. Nothing in
`.work/` is committed.

## 1. The owner's games (read-only, never copied)

Library: the owner's TeknoParrot folder (NFS).

| Game | Board | Main file | SHA-256 | Size |
|---|---|---|---|---|
| F-Zero AX Monster Ride (Japan, SBHA6W, 2004) | Sega Triforce | `F-Zero AX (Triforce) (Monster) [JAP][BHA6].iso` (header id `SBHA6W`) | `280011bf67604a8041bfac3e54a142b56531fad67323bf4a52dc9547a4d33961` | 436 029 440 |
| Mario Kart Arcade GP 2 (MK21 Ver. A, 2007-08-20) | Sega Triforce | `Mario Kart Arcade GP 2 (MK2100-4-NA-MPRO-B00) (20-08-2007 094721) (E).iso` (header id `RELSAB`, "Sample Game Name") | `bf14a7c35efe1bda690adc5afdf30a6a0106cf89c4b61e8048f449f8c94ace01` | 536 870 912 |
| Mario Kart Arcade GP DX (1.18.16, English mod 1.0) | Namco ES3A (Win32, DXVK) | `MK_AGP3_FINAL.exe` | `3c4862309b8a2943b46d9b917f1a28fb900691bbf6201280429e6817e861b785` | 7 432 704 |

Dolphin gives the two discs the ids `GHAE6E` and `GNLJ82` (read from its
`TimePlayed.ini` after a run). The library scan identifies a disc image by
SHA-256 under any `.iso`/`.gcm` name.

## 2. How TeknoParrot runs them (GameProfiles from TeknoParrotUI 1.0.0.2296)

| Profile | EmulatorType | Notes |
|---|---|---|
| `FZeroAXMonster` | Dolphin (TeknoParrot's Triforce build) | wheel, gas, brake, an **Analog Y** (the Monster cabinet's handle tilts), boost, four view buttons, two paddles |
| `MarioKartGP2` | Dolphin | wheel, gas, brake, item, VS-cancel |
| `MKDX118` | TeknoParrot (closed core) | resolution, online matching, card ids |

## 3. Community projects found

| Project | Licence | Covers | Notes | Decision |
|---|---|---|---|---|
| [Dolphin](https://github.com/dolphin-emu/dolphin) (mainline) | GPL-2.0-or-later | both Triforce games; Triforce merged Feb 2026, in releases since 2603 ([blog](https://dolphin-emu.org/blog/2026/02/16/rise-of-the-triforce/)) | Per-game I/O (`HW/Triforce/FZeroAX.cpp`, `MarioKartGP.cpp`) on the emulated pad; `segaboot.gcm` only for the test menu; **no force feedback to the host**; no camera, no CyCraft seat; `dolphin-emu-nogui` boots Triforce; a `Pipe` input backend on Unix | **used**: 2609b, built from source for Linux, release build for Windows |
| [windowsarcadeloader](https://github.com/aderumier/windowsarcadeloader) | GPL-3.0 | Mario Kart DX v1.10.22 and **v1.18.16** (`namcoes3` profile) | dongle skip, offline router, unlocked-profile card server, JVS steering/pedals, F:/G: drives, force feedback from the game's steering value; status "works (user: 100%)" | **used**, as in the Initial D launcher (same commit, `patches/wal` 0001 and 0002) |
| [bananatools](https://gitea.tendokyu.moe/TeamTofuShop/bananatools) | public domain | MKDX hooks | windowsarcadeloader's reference | reference |
| TeknoParrot | closed | all | parked under Wine in the sister projects | fallback only |

## 4. Cabinet hardware a PC does not have

| Game | What stops it | What the launcher does | Source |
|---|---|---|---|
| F-Zero AX Monster Ride | The CyCraft motion seat (on the cabinet's network; Dolphin only redirects `192.168.11.0/24` to localhost): the boot stays on "Searching cycraft" | Gecko code `0043137B 00000086` ("Bypass Cycraft") in the runtime's `GameSettings/GHAE6E.ini`, cheats on for this game | [Internet Archive, "F-ZERO AX Monster Ride (Decrypted/for Dolphin)"](https://archive.org/details/fzeroax_monster_ride); seen working, RUNTIME_NOTES |
| F-Zero AX Monster Ride | First boot asks to move the handle left/right **and up/down**, both pedals, then Start | the launcher carries a pitch axis (a pad's left stick Y, keys W/Q) on the pad's stick Y | seen, RUNTIME_NOTES |
| Mario Kart GP 2 | The camera (it photographs the player): CAMERA ERROR at the device check | Dolphin's own shipped Action Replay code "Skip Boot Check" for `GNLJ82`, enabled in `GameSettings/GNLJ82.ini` | Dolphin `Data/Sys/GameSettings/GNLJ82.ini`; seen working |
| Mario Kart DX | The Namcam camera: E08-01 NAMCAM (CAMERA) ERROR with the game's own operator settings | the dump's `FTestMode`/`GTestMode` files (TeknoParrot's settings: camera and IC card units off) seeded once onto the F:/G: drives | seen working |
| Mario Kart DX | windowsarcadeloader's shared prefix installs about forty winetricks verbs; `dmusic32` and `gmdls` download from web.archive.org only | the launcher's layer lists the same verbs without those two | seen |
| Mario Kart GP 2 | Link check of three other cabinets (about a minute) | nothing yet: Dolphin's "Skip Link Check" code made no difference | seen |

## 5. Decisions

Taken on 2026-11-14 as defaults from the sister projects' owner decisions
(the owner asked for "a similar multi-game launcher with similar
features"):

| # | Decision |
|---|---|
| D1 | DRM exception as in the sister projects: windowsarcadeloader's dongle and card emulation for Mario Kart DX; nothing of our own. |
| D2 | Hybrid hub: Dolphin for Triforce, windowsarcadeloader for Mario Kart DX, one SDL input thread owning pads and wheels for both (Dolphin's pipe device on Linux, windowsarcadeloader's stdin). |
| D3 | Everything a game needs ships in the package (owner, 2026-11-14: "build the packages with everything necessary to run bundled in"). |
| D4 | GPL-3.0 and original illustrations. |

## 6. Input and force feedback

- Dolphin's Triforce I/O reads the first emulated GameCube pad: stick X
  steering, stick Y the Monster handle's pitch, R/L analog gas/brake, A
  item/boost, B the Mario button, D-pad F-Zero's views, X/Y its paddles,
  Start; Test/Service/Coin are the pad's `Triforce` group.
- The stick's gate (79 % of travel by default) is set to 100 % so the
  steering reaches 0..255 (seen: 127 both ways at calibration).
- Linux: the launcher writes the whole pad state to `Pipes/launcher`
  (`SET MAIN x y`, `SET R`, `SET L`, `PRESS`/`RELEASE`), so wheels, dead
  zones and the SDK's wheel table apply as for the other games. The
  keyboard of gamescope's X server is a second way in.
- Windows: Dolphin has no pipes; the launcher writes a mapping for the
  first XInput pad, the keyboard and (when the input thread sees one) the
  wheel through Dolphin's SDL backend, with the axes the SDK's wheel
  profile found.
- Force feedback: Dolphin emits none for these games; the wheel gets the
  SDK's synthetic centring spring and damper (`rea_launcher::ffb`, no
  telemetry: the at-rest spring). Mario Kart DX: windowsarcadeloader's
  force feedback lines are played on the wheel.
