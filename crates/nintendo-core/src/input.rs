//! The cabinets' buttons, the default pad layout, and the step from the
//! SDK's device-independent controls to what each loader reads.
//!
//! The three cabinets share a wheel, two pedals, Start, coin, test and
//! service. Their own buttons differ in name more than in place, so one
//! set covers them: the item button of the Mario Kart cabinets is the boost
//! button of F-Zero AX, the Mario button (Mario Kart) sits where F-Zero AX
//! changes the view, and F-Zero AX's two paddles behind the wheel are Mario
//! Kart DX's menu up / down.
//!
//! windowsarcadeloader's payloads read a virtual arcade stick per player
//! (its `wal_protocol`): buttons as bits, six axes. Which bit is which
//! cabinet button comes from the system profile's `controls` section
//! (`systemprofiles/namcoes3/mario-kart-arcade-gp-dx.yaml` at the pinned
//! commit).

use rea_launcher::wheel::{Action, Control, Controls as Read, PadInput, PadMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Button {
    /// Mario Kart: use the item. F-Zero AX: boost.
    Item,
    /// Mario Kart: the Mario button (cancel, horn). F-Zero AX: view change.
    Special,
    /// F-Zero AX: left paddle. Mario Kart DX: menu up.
    PaddleLeft,
    /// F-Zero AX: right paddle. Mario Kart DX: menu down.
    PaddleRight,
    /// Mario Kart DX: the Enter switch on the panel.
    Enter,
    Start,
    Card,
    Coin,
    Test,
    Service,
}

impl Control for Button {
    const ALL: &'static [Self] = &[
        Button::Item,
        Button::Special,
        Button::PaddleLeft,
        Button::PaddleRight,
        Button::Enter,
        Button::Start,
        Button::Card,
        Button::Coin,
        Button::Test,
        Button::Service,
    ];

    fn key(self) -> &'static str {
        match self {
            Button::Item => "item",
            Button::Special => "special",
            Button::PaddleLeft => "paddle_left",
            Button::PaddleRight => "paddle_right",
            Button::Enter => "enter",
            Button::Start => "start",
            Button::Card => "card",
            Button::Coin => "coin",
            Button::Test => "test",
            Button::Service => "service",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Button::Item => "Item / boost",
            Button::Special => "Mario button / view change",
            Button::PaddleLeft => "Left paddle / menu up",
            Button::PaddleRight => "Right paddle / menu down",
            Button::Enter => "Enter (Mario Kart DX)",
            Button::Start => "Start",
            Button::Card => "Insert / remove card",
            Button::Coin => "Insert coin",
            Button::Test => "Operator test menu",
            Button::Service => "Service",
        }
    }

    fn bit(self) -> u32 {
        1 << Self::ALL.iter().position(|&b| b == self).expect("in ALL")
    }
}

/// The pad layout a player gets before changing anything: triggers for the
/// pedals, the left stick steers, A uses the item (boost), B the Mario
/// button (view), the shoulders are the paddles.
pub fn default_pad_map() -> PadMap<Button> {
    PadMap::with(&[
        (Action::Accelerate, &[PadInput::RightTrigger]),
        (Action::Brake, &[PadInput::LeftTrigger]),
        (Action::Press(Button::Item), &[PadInput::A]),
        (Action::Press(Button::Special), &[PadInput::B]),
        (Action::Press(Button::PaddleLeft), &[PadInput::LeftShoulder]),
        (
            Action::Press(Button::PaddleRight),
            &[PadInput::RightShoulder],
        ),
        (Action::Press(Button::Enter), &[PadInput::X]),
        (Action::Press(Button::Start), &[PadInput::Start]),
        (Action::Press(Button::Card), &[PadInput::Y]),
        (Action::Press(Button::Coin), &[PadInput::Back]),
    ])
}

/// windowsarcadeloader's virtual stick bits (`wal_protocol::button`).
pub mod wal {
    pub const B1: u32 = 1 << 4;
    pub const B2: u32 = 1 << 5;
    pub const B5: u32 = 1 << 8;
    pub const B7: u32 = 1 << 10;
    pub const B8: u32 = 1 << 11;
    pub const START: u32 = 1 << 12;
    pub const COIN: u32 = 1 << 13;
    pub const SERVICE: u32 = 1 << 14;
    pub const TEST: u32 = 1 << 15;
    pub const CARD: u32 = 1 << 16;
}

/// Which virtual stick buttons a system profile reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickLayout {
    /// Namco ES3 Mario Kart DX: b1 Enter switch, b2 Mario button, b5 item,
    /// b7/b8 menu up/down.
    NamcoEs3Kart,
}

impl StickLayout {
    fn bit(self, b: Button) -> u32 {
        match b {
            Button::Enter => wal::B1,
            Button::Special => wal::B2,
            Button::Item => wal::B5,
            Button::PaddleLeft => wal::B7,
            Button::PaddleRight => wal::B8,
            Button::Start => wal::START,
            Button::Card => wal::CARD,
            Button::Coin => wal::COIN,
            Button::Test => wal::TEST,
            Button::Service => wal::SERVICE,
        }
    }

    /// Player 1's line for `arcade-launcher --input-pipe`
    /// (`p1 <buttons hex> <lx> <ly> <rx> <ry> <accel> <brake>`).
    pub fn pipe_line(self, r: &Read) -> String {
        let buttons = Button::ALL
            .iter()
            .filter(|b| r.buttons & b.bit() != 0)
            .fold(0u32, |acc, &b| acc | self.bit(b));
        let axis = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0).round() as i32;
        let pedal = |v: f32| (v.clamp(0.0, 1.0) * 32767.0).round() as i32;
        format!(
            "p1 {buttons:x} {} 0 0 0 {} {}",
            axis(r.steer),
            pedal(r.accel),
            pedal(r.brake)
        )
    }
}

/// Dolphin's pipe device (its `Pipe` input backend): which of its inputs
/// carries each cabinet button in `GCPadNew.ini`, and the commands that
/// drive them. The emulated pad's Triforce wiring (Dolphin
/// `HW/Triforce/FZeroAX.cpp`, `MarioKartGP.cpp`): A item/boost, B the Mario
/// button, D-pad right F-Zero AX's first view, X/Y its paddles, Start;
/// test, service and coin are the pad's `Triforce` group, here on the
/// pipe's Z, L and R buttons (no game reads the pad's digital L/R or Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DolphinPad {
    pub item: &'static str,
    pub special: &'static str,
    pub view: &'static str,
    pub paddle_left: &'static str,
    pub paddle_right: &'static str,
    pub start: &'static str,
    pub coin: &'static str,
    pub test: &'static str,
    pub service: &'static str,
}

impl DolphinPad {
    pub const PIPE: DolphinPad = DolphinPad {
        item: "Button A",
        special: "Button B",
        view: "Button D_RIGHT",
        paddle_left: "Button X",
        paddle_right: "Button Y",
        start: "Button START",
        coin: "Button Z",
        test: "Button L",
        service: "Button R",
    };

    /// The pipe's button tokens each cabinet button presses.
    fn tokens(b: Button) -> &'static [&'static str] {
        match b {
            Button::Item => &["A"],
            Button::Special => &["B", "D_RIGHT"],
            Button::PaddleLeft => &["X"],
            Button::PaddleRight => &["Y"],
            Button::Start => &["START"],
            Button::Coin => &["Z"],
            Button::Test => &["L"],
            Button::Service => &["R"],
            Button::Enter | Button::Card => &[],
        }
    }

    /// The whole pad state as pipe commands: the stick (steering on X;
    /// `pitch`, F-Zero AX Monster Ride's handle pushed up −1 or pulled down
    /// +1, on Y; 0..1 with 0.5 centred), the two analog triggers (gas on R,
    /// brake on L), and every button pressed or released.
    pub fn pipe_lines(r: &Read, pitch: f32) -> String {
        let mut s = format!(
            "SET MAIN {:.4} {:.4}\nSET R {:.4}\nSET L {:.4}\n",
            (r.steer.clamp(-1.0, 1.0) + 1.0) / 2.0,
            (pitch.clamp(-1.0, 1.0) + 1.0) / 2.0,
            r.accel.clamp(0.0, 1.0),
            r.brake.clamp(0.0, 1.0)
        );
        let mut pressed: Vec<&str> = Vec::new();
        for &b in Button::ALL {
            if r.buttons & b.bit() != 0 {
                pressed.extend(Self::tokens(b));
            }
        }
        for tok in ["A", "B", "X", "Y", "Z", "START", "L", "R", "D_RIGHT"] {
            let verb = if pressed.contains(&tok) {
                "PRESS"
            } else {
                "RELEASE"
            };
            s.push_str(&format!("{verb} {tok}\n"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rea_launcher::wheel::{PadSettings, PadState, read_pad};

    #[test]
    fn bits_are_distinct() {
        let mut seen = 0u32;
        for b in Button::ALL {
            assert_eq!(seen & b.bit(), 0);
            seen |= b.bit();
        }
    }

    #[test]
    fn the_kart_layout_maps_every_button_to_its_own_bit() {
        let mut seen = 0u32;
        for &b in Button::ALL {
            let bit = StickLayout::NamcoEs3Kart.bit(b);
            assert_eq!(seen & bit, 0, "{b:?}");
            seen |= bit;
        }
    }

    #[test]
    fn item_and_mario_button_follow_the_profile() {
        let r = Read {
            buttons: Button::Item.bit() | Button::Special.bit(),
            ..Read::default()
        };
        assert_eq!(
            StickLayout::NamcoEs3Kart.pipe_line(&r),
            "p1 120 0 0 0 0 0 0"
        );
    }

    #[test]
    fn the_pipe_carries_the_stick_the_triggers_and_every_button() {
        let r = Read {
            steer: -1.0,
            accel: 1.0,
            brake: 0.25,
            buttons: Button::Special.bit() | Button::Coin.bit(),
        };
        let s = DolphinPad::pipe_lines(&r, 0.0);
        assert!(
            s.starts_with("SET MAIN 0.0000 0.5000\nSET R 1.0000\nSET L 0.2500\n"),
            "{s}"
        );
        assert!(s.contains("PRESS B\n") && s.contains("PRESS D_RIGHT\n"));
        assert!(s.contains("PRESS Z\n") && s.contains("RELEASE A\n"));
        assert!(s.ends_with('\n'));
        assert!(DolphinPad::pipe_lines(&r, -1.0).starts_with("SET MAIN 0.0000 0.0000\n"));
        // the ini names the same inputs the commands drive
        for name in [
            DolphinPad::PIPE.item,
            DolphinPad::PIPE.coin,
            DolphinPad::PIPE.view,
        ] {
            let tok = name.strip_prefix("Button ").unwrap();
            assert!(s.contains(&format!(" {tok}\n")), "{tok}");
        }
    }

    #[test]
    fn a_pad_steers_and_presses_the_pedals() {
        let settings = PadSettings::new(default_pad_map());
        let pad = PadState {
            left_x: -32768,
            right_trigger: 32767,
            start: true,
            ..PadState::default()
        };
        let read = read_pad(&settings, &pad, None);
        let line = StickLayout::NamcoEs3Kart.pipe_line(&read);
        assert_eq!(line, "p1 1000 -32767 0 0 0 32767 0");
    }
}
