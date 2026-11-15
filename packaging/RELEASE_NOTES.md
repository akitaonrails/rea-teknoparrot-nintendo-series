The first release: one launcher for F-Zero AX Monster Ride, Mario Kart
Arcade GP 2, Mario Kart Arcade GP DX and Luigi's Mansion Arcade. No game files are included: point
it at your own dumps.

**Linux** (`NintendoArcadeLauncher-<version>-linux-x86_64.tar.gz`): all three
racers played a race, and Luigi's Mansion a stage, through the launcher.
Dolphin, windowsarcadeloader, GE-Proton and winetricks are inside; you need
gamescope, bubblewrap and cabextract.

**Windows** (`NintendoArcadeLauncher-<version>-windows-x86_64.zip`): Dolphin
and windowsarcadeloader inside. Not yet run on a Windows PC: reports are
welcome.

What's in it:

- No TeknoParrot: mainline Dolphin (Triforce) and windowsarcadeloader, from
  pinned sources.
- Monster Ride without its motion seat, both Mario Karts without their camera.
- Luigi's Mansion with a pad (the stick moves the crosshair); on Linux a
  mouse or a lightgun too.
- Pads and wheels read once for every game; on Linux the Triforce games get
  them through Dolphin's pipe input, with full steering lock and a
  centring spring on force-feedback wheels.
- Render sizes up to 4K; on Linux FSR 1, NIS and full screen through
  gamescope.

Not checked yet: a real wheel, a Windows PC. See
README.md and docs/RUNTIME_NOTES.md.
