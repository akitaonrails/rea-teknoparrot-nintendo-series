Nintendo Arcade Launcher
========================

One launcher for F-Zero AX Monster Ride, Mario Kart Arcade GP 2 (Sega
Triforce), Mario Kart Arcade GP DX (Namco ES3) and Luigi's Mansion Arcade
(Sega Nu). It contains no game
files: you need your own dumps. Your game folders are only read, never
changed.

Start it
--------

Linux:    ./nintendo-launcher
Windows:  nintendo-launcher.exe

The first window asks for your games folder (the TeknoParrot layout works
as is) and, optionally, segaboot.gcm (only the Triforce operator test menu
needs it). Then pick a game and press Play.

What it needs on Linux
----------------------

- gamescope and bubblewrap (bwrap): every game runs in them.
- A Vulkan driver, 32-bit OpenGL, and cabextract (the Windows games'
  Microsoft runtimes are installed with winetricks the first time; that
  start needs the network).
- For a mouse or lightgun in Luigi's Mansion: your user in the input
  group (a pad works without it).
- Dolphin, windowsarcadeloader and its Wine are in this package.

What it needs on Windows
------------------------

- Windows 10 or 11, 64-bit. Dolphin and windowsarcadeloader are in this
  package.

Controls
--------

Pad: left stick steers (up/down tilts F-Zero AX Monster Ride's handle),
RT/LT gas and brake, A item/boost, B Mario button/view, LB/RB paddles, X
Enter (Mario Kart DX), Start, Back coin, Y card. Wheels are recognised.
Luigi's Mansion: left stick moves the crosshair, RT or A vacuums, B the
gun's button, Start joins; on Linux a mouse (left button) or lightgun aims
too.
Keyboard (Triforce games): arrows, W/Q handle tilt, Z, X, A/S, Enter, 5
coin, 9 test, 0 service.

Monster Ride asks for its handle and pedal adjustment at boot: turn the
wheel both ways, push the stick (or W/Q) up and down, press both pedals,
then Start. Mario Kart GP 2's menus decide with the gas pedal.

Where things are kept
---------------------

Linux:    ~/.local/share/rea-nintendo/
Windows:  %LOCALAPPDATA%\rea-nintendo\

Settings, logs, and runtimes/<game>/ (each game's private folder, your
saves included). To uninstall, delete this folder and that one (back up
runtimes/ first).

Licences: licenses/. Source: https://github.com/akitaonrails/rea-teknoparrot-nintendo-series
