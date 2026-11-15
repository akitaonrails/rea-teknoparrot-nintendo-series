//! The input thread: it owns SDL, reads wheels and pads, turns them into
//! the cabinet's controls and, while a game runs, writes them to the game:
//! windowsarcadeloader's input pipe (Mario Kart DX, whose force feedback
//! comes back on the loader's output and is played on the wheel), or
//! Dolphin's pipe device (the Triforce games, which send no force feedback
//! of their own: the wheel gets the SDK's synthetic centring spring and
//! damper). Between games it only reports what it sees.

use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nintendo_core::input::{Aim, Button, DolphinPad, StickLayout, default_pad_map};
use nintendo_core::launch::DolphinWheel;
use rea_launcher::ffb::{FfbOutput, FfbSettings, SyntheticFfb, Tuning};
use rea_launcher::wheel::{
    Control, Controls as Read, DeviceId, Kind, PadSettings, WheelProfile, WheelSettings, read_pad,
};
use rea_launcher_native::policy::HapticPolicy;
use rea_launcher_native::sdl::{Devices, Sdl};

const TICK: Duration = Duration::from_millis(4);
/// The pipe is refreshed at least this often even without a change.
const REFRESH: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    pub kind: Kind,
    pub haptic: bool,
}

/// What the window shows.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub devices: Vec<DeviceInfo>,
    /// The device driving the car, by index into `devices`.
    pub driver: Option<usize>,
    pub controls: Read,
    pub feeding: bool,
    pub error: Option<String>,
    /// The wheel driving the car, for Dolphin's own SDL mapping (Windows).
    pub wheel: Option<DolphinWheel>,
}

/// What the game reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// windowsarcadeloader's virtual stick lines.
    Wal(StickLayout),
    /// Dolphin's pipe commands.
    Dolphin,
}

impl Wire {
    /// `pitch`: the handle up/down axis only Dolphin's pad carries; `aim`:
    /// a gun game's crosshair.
    pub fn line(self, r: &Read, pitch: f32, aim: Aim) -> String {
        match self {
            Wire::Wal(layout) => layout.pipe_line(r, aim) + "\n",
            Wire::Dolphin => DolphinPad::pipe_lines(r, pitch),
        }
    }
}

pub enum Command {
    /// Start feeding a game: `sink` is the loader's stdin or Dolphin's pipe.
    Attach {
        sink: Box<dyn Write + Send>,
        wire: Wire,
        deadzone: f32,
    },
    /// A force feedback line from the loader (`out <player> <id> <value>`).
    Output(String),
    Detach,
}

pub struct InputService {
    tx: Sender<Command>,
    snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}

impl InputService {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let shared = Arc::clone(&snapshot);
        let thread = std::thread::Builder::new()
            .name("input".into())
            .spawn(move || run(rx, shared))
            .expect("spawn input thread");
        Self {
            tx,
            snapshot,
            thread: Some(thread),
        }
    }

    pub fn sender(&self) -> Sender<Command> {
        self.tx.clone()
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for InputService {
    fn drop(&mut self) {
        let (tx, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.tx, tx));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The game's force feedback state, from windowsarcadeloader's outputs
/// (`wal_protocol::output`: 1 constant, 2 centering spring, 3 vibration,
/// each ±10000). The SDK has no spring effect: the spring is played as a
/// damper, which also resists the wheel.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ffb(pub FfbOutput);

impl Ffb {
    /// Apply one `out <player> <id> <value>` line; `false` if it isn't one.
    pub fn apply(&mut self, line: &str) -> bool {
        let mut w = line.split_whitespace();
        if w.next() != Some("out") {
            return false;
        }
        let (Some(_player), Some(id), Some(value)) = (
            w.next(),
            w.next().and_then(|v| v.parse::<u16>().ok()),
            w.next().and_then(|v| v.parse::<i32>().ok()),
        ) else {
            return false;
        };
        let v = value as f32 / 10000.0;
        match id {
            1 => self.0.constant = v.clamp(-1.0, 1.0),
            2 => self.0.damper = v.clamp(0.0, 1.0),
            3 => self.0.rumble = v.clamp(0.0, 1.0),
            _ => return false,
        }
        true
    }
}

struct Feed {
    sink: Box<dyn Write + Send>,
    wire: Wire,
    last: String,
    sent: Instant,
    /// Dolphin's games: the synthetic force feedback (centring, damping).
    synthetic: Option<SyntheticFfb>,
    /// A gun game's crosshair, moved by the stick (or wheel).
    aim: Aim,
}

fn run(rx: Receiver<Command>, shared: Arc<Mutex<Snapshot>>) {
    let sdl = match Sdl::init(&[]) {
        Ok(s) => s,
        Err(e) => {
            if let Ok(mut s) = shared.lock() {
                s.error = Some(format!("controllers unavailable: {e}"));
            }
            // still feed a game (neutral input) so it never waits on us
            idle_without_sdl(rx);
            return;
        }
    };
    let mut devices = Devices::new(&sdl, HapticPolicy::default());
    let mut pad_settings = PadSettings::new(default_pad_map());
    let wheel_settings = WheelSettings::default();
    let mut profile: Option<(DeviceId, WheelProfile<Button>)> = None;
    let mut feed: Option<Feed> = None;
    let mut ffb = Ffb::default();
    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::Attach {
                    sink,
                    wire,
                    deadzone,
                }) => {
                    pad_settings.stick_deadzone = stick_deadzone(deadzone);
                    ffb = Ffb::default();
                    feed = Some(Feed {
                        sink,
                        wire,
                        last: String::new(),
                        sent: Instant::now() - REFRESH,
                        synthetic: (wire == Wire::Dolphin)
                            .then(|| SyntheticFfb::new(Tuning::ARCADE)),
                        aim: Aim::default(),
                    });
                }
                Ok(Command::Output(line)) => {
                    ffb.apply(&line);
                }
                Ok(Command::Detach) => {
                    feed = None;
                    devices.release_force_feedback();
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        devices.pump();
        let list = devices.list();
        let wheel = list.iter().position(|d| d.kind == Kind::Wheel);
        let mut read = Read::default();
        if let Some(w) = wheel {
            let d = &list[w];
            let fresh = profile
                .as_ref()
                .is_none_or(|(id, _)| id.vid != d.id.vid || id.pid != d.id.pid);
            if fresh {
                profile = Some((
                    d.id.clone(),
                    WheelProfile::guess(
                        d.id.clone(),
                        &d.initial,
                        Some((Button::PaddleRight, Button::PaddleLeft)),
                    ),
                ));
            }
            if let Some((_, p)) = &profile {
                read = p.read(&wheel_settings, &d.state, None);
            }
        }
        let mut pad_driver = None;
        let mut pitch = 0.0f32;
        for (i, d) in list.iter().enumerate() {
            if let Some(pad) = d.pad {
                read = read.merge(read_pad(&pad_settings, &pad, None));
                if pad_driver.is_none() {
                    pitch = stick_axis(pad.left_y, pad_settings.stick_deadzone);
                }
                pad_driver.get_or_insert(i);
            }
        }
        let driver = wheel.or(pad_driver);
        let infos: Vec<DeviceInfo> = list
            .iter()
            .map(|d| DeviceInfo {
                name: d.id.name.clone(),
                kind: d.kind,
                haptic: d.haptic,
            })
            .collect();
        let dolphin_wheel = wheel.and_then(|w| {
            profile.as_ref().map(|(_, p)| DolphinWheel {
                name: list[w].id.name.clone(),
                steer_axis: p.steer_axis,
                gas: p.gas.map(|(_, a)| (a.axis, a.rest, a.peak)),
                brake: p.brake.map(|(_, a)| (a.axis, a.rest, a.peak)),
            })
        });
        if let Some(f) = feed.as_mut() {
            if matches!(f.wire, Wire::Wal(l) if l.aims()) {
                f.aim.step(read.steer, pitch, TICK.as_secs_f32());
            }
            let line = f.wire.line(&read, pitch, f.aim);
            if line != f.last || f.sent.elapsed() >= REFRESH {
                if f.sink
                    .write_all(line.as_bytes())
                    .and_then(|_| f.sink.flush())
                    .is_err()
                {
                    feed = None; // the loader is gone
                } else {
                    f.last = line;
                    f.sent = Instant::now();
                }
            }
            if let (Some(f), Some(d)) = (feed.as_mut(), driver) {
                let out = match f.synthetic.as_mut() {
                    // the wheel's position against its lock; no telemetry
                    // from the game, so the centring spring of a car at rest
                    Some(s) => s.update(
                        TICK.as_secs_f32(),
                        read.steer,
                        None,
                        &FfbSettings::default(),
                    ),
                    None => ffb.0,
                };
                devices.force_feedback(d, &out);
            }
        }
        if let Ok(mut s) = shared.lock() {
            *s = Snapshot {
                devices: infos,
                driver,
                controls: read,
                feeding: feed.is_some(),
                error: None,
                wheel: dolphin_wheel,
            };
        }
        std::thread::sleep(TICK);
    }
}

fn idle_without_sdl(rx: Receiver<Command>) {
    let mut feed: Option<(Box<dyn Write + Send>, Wire)> = None;
    loop {
        match rx.recv_timeout(REFRESH) {
            Ok(Command::Attach { sink, wire, .. }) => feed = Some((sink, wire)),
            Ok(Command::Detach) => feed = None,
            Ok(Command::Output(_)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if let Some((sink, wire)) = feed.as_mut() {
            let line = wire.line(&Read::default(), 0.0, Aim::default());
            if sink
                .write_all(line.as_bytes())
                .and_then(|_| sink.flush())
                .is_err()
            {
                feed = None;
            }
        }
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
impl Write for FifoSink {
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

/// A stick axis (raw −32768..32767) as −1..1, zero inside the dead zone
/// and rescaled outside it.
pub fn stick_axis(raw: i16, deadzone: u16) -> f32 {
    let v = f32::from(raw).max(-32767.0);
    let dz = f32::from(deadzone).min(32000.0);
    if v.abs() <= dz {
        0.0
    } else {
        (v.signum() * (v.abs() - dz) / (32767.0 - dz)).clamp(-1.0, 1.0)
    }
}

/// The settings' dead zone (0–0.3 of the stick's travel) in raw stick units.
pub fn stick_deadzone(fraction: f32) -> u16 {
    (fraction.clamp(0.0, 0.3) * 32767.0).round() as u16
}

/// A timed input script for tests (`--input-script`): `<seconds> <inputs…>`
/// per line, inputs held until the next line; `-` releases everything.
/// Inputs are the cabinet's buttons by key (`start`, `item`, `special`,
/// `coin`…), `accel`, `brake`, `steer=<-1..1>`.
#[derive(Clone)]
pub struct Script {
    steps: Vec<(f64, Read, f32)>,
}

impl Script {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut steps = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut words = line.split_whitespace();
            let t: f64 = words
                .next()
                .and_then(|w| w.parse().ok())
                .ok_or(format!("line {}: no time", n + 1))?;
            let mut r = Read::default();
            let mut pitch = 0.0f32;
            for w in words.filter(|w| *w != "-") {
                if w == "accel" {
                    r.accel = 1.0;
                } else if w == "brake" {
                    r.brake = 1.0;
                } else if let Some(v) = w.strip_prefix("pitch=") {
                    pitch = v.parse().map_err(|_| format!("line {}: {w}", n + 1))?;
                } else if let Some(v) = w.strip_prefix("steer=") {
                    r.steer = v.parse().map_err(|_| format!("line {}: {w}", n + 1))?;
                } else {
                    let b = Button::ALL
                        .iter()
                        .find(|b| b.key() == w)
                        .ok_or(format!("line {}: unknown input {w}", n + 1))?;
                    r.buttons |= b.bit();
                }
            }
            steps.push((t, r, pitch));
        }
        steps.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(Self { steps })
    }

    /// The controls and the handle's pitch held at `seconds`.
    pub fn at(&self, seconds: f64) -> (Read, f32) {
        self.steps
            .iter()
            .rev()
            .find(|(t, _, _)| *t <= seconds)
            .map(|(_, r, p)| (*r, *p))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        use std::io::Read as _;
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

    #[test]
    fn a_stick_axis_has_a_dead_zone_and_full_travel() {
        assert_eq!(stick_axis(1000, 1638), 0.0);
        assert_eq!(stick_axis(32767, 1638), 1.0);
        assert_eq!(stick_axis(-32768, 1638), -1.0);
        assert!(stick_axis(17000, 1638) > 0.4);
    }

    #[test]
    fn dead_zone_converts_to_raw_units() {
        assert_eq!(stick_deadzone(0.0), 0);
        assert_eq!(stick_deadzone(0.05), 1638);
        assert_eq!(stick_deadzone(0.9), 9830);
    }

    #[test]
    fn force_feedback_lines_update_the_state() {
        let mut f = Ffb::default();
        assert!(f.apply("out 0 1 -5000"));
        assert!(f.apply("out 0 2 10000"));
        assert!(f.apply("out 0 3 2500"));
        assert!(!f.apply("out 0 9 1"));
        assert!(!f.apply("wine: hello"));
        assert_eq!(f.0.constant, -0.5);
        assert_eq!(f.0.damper, 1.0);
        assert_eq!(f.0.rumble, 0.25);
    }

    #[test]
    fn a_script_holds_each_step_until_the_next() {
        let s = Script::parse("10 start\n10.5 -\n20 accel steer=-0.5 pitch=-0.25 item\n").unwrap();
        assert_eq!(s.at(5.0).0, Read::default());
        assert_eq!(s.at(10.2).0.buttons, Button::Start.bit());
        assert_eq!(s.at(12.0).0, Read::default());
        let (r, pitch) = s.at(25.0);
        assert_eq!(pitch, -0.25);
        assert_eq!((r.accel, r.steer), (1.0, -0.5));
        assert_eq!(r.buttons, Button::Item.bit());
        assert!(Script::parse("1 jump").is_err());
    }
}
