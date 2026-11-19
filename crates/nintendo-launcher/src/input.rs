//! What the SDK's input thread (`rea_launcher_native::feeder`) writes to
//! each game: windowsarcadeloader's input pipe (Mario Kart DX, whose force
//! feedback comes back on the loader's output; Luigi's Mansion, whose
//! crosshair the stick moves), or Dolphin's pipe device (the Triforce
//! games, which send no force feedback of their own: the wheel gets the
//! SDK's synthetic centring spring and damper).

use nintendo_core::input::{Aim, Button, DolphinPad, StickLayout, default_pad_map};
use nintendo_core::launch::{DolphinWheel, Feed as Wiring};
use rea_launcher::ffb::{FfbOutput, FfbSettings, SyntheticFfb, Tuning as Arcade};
use rea_launcher::hub::HubPaths;
use rea_launcher::hub::bindings::Bindings;
use rea_launcher_native::feeder::{Frame, InputService, WheelInfo, Wire};

/// The player's pad layout: the cabinets', with the changes saved in the
/// data folder's `controls.cfg`. A file that can't be read leaves the
/// cabinets' layout, and `warn` hears why.
pub fn bindings(paths: &HubPaths, warn: impl FnOnce(&str)) -> Bindings<Button> {
    let file = paths.controls_file();
    Bindings::load(&file, default_pad_map()).unwrap_or_else(|e| {
        warn(&format!(
            "{}: {e}; the default pad layout is used",
            file.display()
        ));
        Bindings::new(default_pad_map())
    })
}

/// The input thread, with the player's pad layout `bindings`; a wheel's
/// shift paddles are the paddles (right, left).
pub fn service(bindings: &Bindings<Button>) -> InputService {
    InputService::start(
        bindings.map().clone(),
        Some((Button::PaddleRight, Button::PaddleLeft)),
    )
}

/// windowsarcadeloader's stick line for a system profile's layout.
struct Wal {
    layout: StickLayout,
    /// A gun game's crosshair.
    aim: Aim,
    /// A script places the crosshair itself (steer and pitch are its x
    /// and y); a stick or wheel moves it.
    placed: bool,
}

impl Wire for Wal {
    fn encode(&mut self, f: &Frame) -> String {
        let c = &f.controls;
        if self.placed {
            self.aim = Aim {
                x: c.steer,
                y: f.pitch,
            };
        } else if self.layout.aims() {
            self.aim.step(c.steer, f.pitch, f.dt);
        }
        self.layout.pipe_line(c, self.aim) + "\n"
    }

    fn quit(&self) -> Option<&'static str> {
        Some("quit")
    }
}

/// Dolphin's pipe commands, and the synthetic force feedback of a car at
/// rest (no telemetry from the game).
struct Dolphin(SyntheticFfb);

impl Wire for Dolphin {
    fn encode(&mut self, f: &Frame) -> String {
        DolphinPad::pipe_lines(&f.controls, f.pitch)
    }

    fn force(&mut self, _game: FfbOutput, f: &Frame) -> FfbOutput {
        self.0
            .update(f.dt, f.controls.steer, None, &FfbSettings::default())
    }
}

/// The wire a game's input takes (`None`: the game reads the devices
/// itself); `scripted` for a timed script's run.
pub fn wire(wiring: &Wiring, scripted: bool) -> Option<Box<dyn Wire>> {
    match wiring {
        Wiring::Wal(layout) => Some(Box::new(Wal {
            layout: *layout,
            aim: Aim::default(),
            placed: scripted && layout.aims(),
        })),
        Wiring::DolphinPipe(_) => Some(Box::new(Dolphin(SyntheticFfb::new(Arcade::ARCADE)))),
        Wiring::None => None,
    }
}

/// The wheel the input thread found, for Dolphin's own mapping (Windows).
pub fn dolphin_wheel(w: &WheelInfo) -> DolphinWheel {
    DolphinWheel {
        name: w.name.clone(),
        steer_axis: w.steer_axis,
        gas: w.gas,
        brake: w.brake,
    }
}

/// Dolphin's pipe, written without ever blocking the input thread: opened
/// once Dolphin has opened its end (until then, and while the pipe is full,
/// a state is dropped; the next one carries the whole pad again). Writing
/// fails only once Dolphin, having read, is gone.
#[cfg(unix)]
pub struct FifoSink {
    path: std::path::PathBuf,
    file: Option<std::fs::File>,
}

#[cfg(unix)]
impl FifoSink {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path, file: None }
    }
}

#[cfg(unix)]
impl std::io::Write for FifoSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use std::os::unix::fs::OpenOptionsExt;
        if self.file.is_none() {
            // O_NONBLOCK: ENXIO while no reader has the pipe open
            match std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(0o4000)
                .open(&self.path)
            {
                Ok(f) => self.file = Some(f),
                Err(_) => return Ok(buf.len()),
            }
        }
        let f = self.file.as_mut().expect("opened above");
        match f.write(buf) {
            Ok(_) => Ok(buf.len()),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(buf.len()),
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_launcher::wheel::{Control, Controls};

    #[cfg(unix)]
    trait NonBlock {
        fn custom_flags_nonblock(&mut self) -> &mut Self;
    }

    #[cfg(unix)]
    impl NonBlock for std::fs::OpenOptions {
        fn custom_flags_nonblock(&mut self) -> &mut Self {
            use std::os::unix::fs::OpenOptionsExt;
            self.custom_flags(0o4000)
        }
    }

    #[test]
    #[cfg(unix)]
    fn the_fifo_waits_for_its_reader_then_carries_whole_lines() {
        use std::io::{Read as _, Write as _};
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("pipe");
        let ok = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap();
        assert!(ok.success());
        let mut sink = FifoSink::new(path.clone());
        // nobody reads yet: dropped, not an error, not blocked
        sink.write_all(b"SET R 1.0\n").unwrap();
        let mut reader = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags_nonblock()
            .open(&path)
            .unwrap();
        sink.write_all(b"PRESS A\n").unwrap();
        let mut got = String::new();
        let _ = reader.read_to_string(&mut got);
        assert_eq!(got, "PRESS A\n");
    }

    fn frame(steer: f32, pitch: f32, dt: f32) -> Frame {
        Frame {
            controls: Controls {
                steer,
                buttons: Button::Start.bit(),
                ..Controls::default()
            },
            pitch,
            dt,
        }
    }

    #[test]
    fn a_stick_moves_the_crosshair_and_a_script_places_it() {
        let gun = Wiring::Wal(StickLayout::SegaNuGun);
        let mut moved = wire(&gun, false).unwrap();
        assert_eq!(moved.encode(&frame(1.0, 0.0, 0.0)), "p1 1000 0 0 0 0 0 0\n");
        let line = moved.encode(&frame(1.0, 0.0, Aim::CROSS_SECONDS / 2.0));
        assert_eq!(line, "p1 1000 32767 0 0 0 0 0\n");
        let mut placed = wire(&gun, true).unwrap();
        assert_eq!(
            placed.encode(&frame(-1.0, 0.5, 0.0)),
            "p1 1000 -32767 16384 0 0 0 0\n"
        );
        assert_eq!(placed.quit(), Some("quit"));
    }

    #[test]
    fn the_kart_steers_and_dolphin_gets_its_pad_and_a_centring_force() {
        let mut kart = wire(&Wiring::Wal(StickLayout::NamcoEs3Kart), true).unwrap();
        assert_eq!(
            kart.encode(&frame(-1.0, 0.0, 0.0)),
            "p1 1000 -32767 0 0 0 0 0\n"
        );
        let mut dolphin = wire(&Wiring::DolphinPipe("/p".into()), false).unwrap();
        assert!(
            dolphin
                .encode(&frame(0.0, 0.0, 0.0))
                .contains("PRESS START\n")
        );
        assert_eq!(dolphin.quit(), None);
        let f = frame(0.8, 0.0, 0.004);
        let mut force = FfbOutput::default();
        for _ in 0..100 {
            force = dolphin.force(FfbOutput::default(), &f);
        }
        assert!(force.constant != 0.0, "{force:?}");
        assert!(wire(&Wiring::None, false).is_none());
    }
}
