use std::cell::RefCell;
use std::rc::Rc;

use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::prelude::*;

pub const BUTTON_NAMES: [&str; 17] = [
    "a", "b", "x", "y", "lb", "rb", "lt", "rt", "view", "menu", "l3", "r3", "up", "down", "left",
    "right", "home",
];

pub const BUTTON_A: u32 = 1 << 0;
pub const BUTTON_X: u32 = 1 << 2;
pub const BUTTON_LT: u32 = 1 << 6;
pub const BUTTON_RT: u32 = 1 << 7;
pub const BUTTON_R3: u32 = 1 << 11;
pub const BUTTON_UP: u32 = 1 << 12;
pub const BUTTON_DOWN: u32 = 1 << 13;
pub const DPAD_BUTTONS: u32 = 0xF << 12;

const BUTTON_MAP: [(GamepadButton, u32); 15] = [
    (GamepadButton::South, 1 << 0),
    (GamepadButton::East, 1 << 1),
    (GamepadButton::West, 1 << 2),
    (GamepadButton::North, 1 << 3),
    (GamepadButton::LeftTrigger, 1 << 4),
    (GamepadButton::RightTrigger, 1 << 5),
    (GamepadButton::Select, 1 << 8),
    (GamepadButton::Start, 1 << 9),
    (GamepadButton::LeftThumb, 1 << 10),
    (GamepadButton::RightThumb, 1 << 11),
    (GamepadButton::DPadUp, 1 << 12),
    (GamepadButton::DPadDown, 1 << 13),
    (GamepadButton::DPadLeft, 1 << 14),
    (GamepadButton::DPadRight, 1 << 15),
    (GamepadButton::Mode, 1 << 16),
];

const SONY_VENDOR: u16 = 0x054C;
pub const TRIGGER_PRESSED: f32 = 0.45;
const ACTIVITY: f32 = 0.35;
pub const MOVE_DEADZONE: f32 = 0.24;
pub const LOOK_DEADZONE: f32 = 0.20;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    pub lt: f32,
    pub rt: f32,
    pub buttons: u32,
}

impl PadState {
    pub fn is_active(&self) -> bool {
        self.buttons != 0
            || [self.lx, self.ly, self.rx, self.ry]
                .iter()
                .any(|v| v.abs() > ACTIVITY)
            || self.lt > TRIGGER_PRESSED
            || self.rt > TRIGGER_PRESSED
    }

    pub fn with_trigger_buttons(mut self) -> Self {
        if self.lt >= TRIGGER_PRESSED {
            self.buttons |= BUTTON_LT;
        }
        if self.rt >= TRIGGER_PRESSED {
            self.buttons |= BUTTON_RT;
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PadKind {
    #[default]
    None,
    Xbox,
    PlayStation,
}

impl PadKind {
    pub fn name(self) -> Option<&'static str> {
        match self {
            PadKind::None => None,
            PadKind::Xbox => Some("xbox"),
            PadKind::PlayStation => Some("ps"),
        }
    }

    fn of_vendor(vendor: Option<u16>) -> Self {
        if vendor == Some(SONY_VENDOR) {
            PadKind::PlayStation
        } else {
            PadKind::Xbox
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadOptions {
    pub yaw_speed: f32,
    pub pitch_speed: f32,
    pub cursor_speed: f32,
    pub move_deadzone: f32,
    pub look_deadzone: f32,
    pub invert_pitch: bool,
}

impl Default for PadOptions {
    fn default() -> Self {
        Self {
            yaw_speed: 1.0,
            pitch_speed: 1.0,
            cursor_speed: 1.0,
            move_deadzone: MOVE_DEADZONE,
            look_deadzone: LOOK_DEADZONE,
            invert_pitch: false,
        }
    }
}

impl PadOptions {
    pub fn set(&mut self, name: &str, value: f32, flag: bool) {
        match name {
            "yawSpeed" => self.yaw_speed = value.clamp(0.25, 3.0),
            "pitchSpeed" => self.pitch_speed = value.clamp(0.25, 3.0),
            "cursorSpeed" => self.cursor_speed = value.clamp(0.25, 3.0),
            "deadzone" => {
                self.move_deadzone = value.clamp(0.05, 0.6);
                self.look_deadzone = self.move_deadzone * LOOK_DEADZONE / MOVE_DEADZONE;
            }
            "invertPitch" => self.invert_pitch = flag,
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadEvent {
    Button { name: &'static str, down: bool },
    Device(Option<&'static str>),
}

#[derive(Default)]
pub struct Controller {
    pub connected: PadKind,
    pub in_use: bool,
    pub capture: bool,
    pub scroll_mode: bool,
    pub cursor_mode: bool,
    pub options: PadOptions,
    pub simulated: Option<(PadState, PadKind)>,
    pub pad: PadState,
    pub active_at: f64,
    previous_buttons: u32,
    reported: PadKind,
    pub events: Vec<PadEvent>,
}

impl Controller {
    pub fn shown_kind(&self) -> PadKind {
        if self.in_use {
            self.connected
        } else {
            PadKind::None
        }
    }

    pub fn take_input(&mut self, read: Option<(PadState, PadKind)>, now: f64) -> u32 {
        let (raw, kind) = self.simulated.or(read).unwrap_or_default();
        self.connected = kind;
        let pad = raw.with_trigger_buttons();
        self.pad = raw;
        if pad.is_active() && kind != PadKind::None {
            self.active_at = now;
            self.in_use = true;
        }
        let changed = pad.buttons ^ self.previous_buttons;
        self.previous_buttons = pad.buttons;
        if self.shown_kind() != self.reported {
            self.reported = self.shown_kind();
            self.events.push(PadEvent::Device(self.reported.name()));
        }
        for (index, name) in BUTTON_NAMES.iter().enumerate() {
            let bit = 1 << index;
            if changed & bit != 0 {
                self.events.push(PadEvent::Button {
                    name,
                    down: pad.buttons & bit != 0,
                });
            }
        }
        changed
    }

    pub fn use_keyboard_and_mouse(&mut self) {
        self.in_use = false;
        self.cursor_mode = false;
    }
}

#[derive(Clone, Default)]
pub struct SharedController(pub Rc<RefCell<Controller>>);

pub fn read_gamepad(pad: &Gamepad) -> (PadState, PadKind) {
    let axis = |a: GamepadAxis| pad.get(a).unwrap_or(0.0);
    let button = |b: GamepadButton| pad.get(b).unwrap_or(0.0);
    let buttons = BUTTON_MAP
        .iter()
        .filter(|(b, _)| pad.pressed(*b))
        .fold(0, |acc, (_, bit)| acc | bit);
    let state = PadState {
        lx: axis(GamepadAxis::LeftStickX),
        ly: axis(GamepadAxis::LeftStickY),
        rx: axis(GamepadAxis::RightStickX),
        ry: axis(GamepadAxis::RightStickY),
        lt: button(GamepadButton::LeftTrigger2),
        rt: button(GamepadButton::RightTrigger2),
        buttons,
    };
    (state, PadKind::of_vendor(pad.vendor_id()))
}

pub fn radial_deadzone(x: f32, y: f32, deadzone: f32) -> Vec2 {
    let v = Vec2::new(x, y);
    let length = v.length();
    if length <= deadzone {
        return Vec2::ZERO;
    }
    let scaled = ((length - deadzone) / (1.0 - deadzone)).min(1.0);
    v / length * scaled
}

pub fn poll_pads(time: Res<Time<Real>>, pads: Query<&Gamepad>, shared: NonSend<SharedController>) {
    let mut chosen: Option<(PadState, PadKind)> = None;
    for pad in &pads {
        let read = read_gamepad(pad);
        let active = read.0.with_trigger_buttons().is_active();
        if chosen.is_none() || active {
            chosen = Some(read);
        }
        if active {
            break;
        }
    }
    shared
        .0
        .borrow_mut()
        .take_input(chosen, time.elapsed_secs_f64());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_become_button_events_and_the_device_is_announced() {
        let mut controller = Controller::default();
        let pressed = PadState {
            buttons: BUTTON_A,
            ..Default::default()
        };
        controller.take_input(Some((pressed, PadKind::PlayStation)), 1.0);
        assert_eq!(
            controller.events,
            vec![
                PadEvent::Device(Some("ps")),
                PadEvent::Button {
                    name: "a",
                    down: true
                }
            ]
        );
        controller.events.clear();
        controller.take_input(Some((PadState::default(), PadKind::PlayStation)), 1.1);
        assert_eq!(
            controller.events,
            vec![PadEvent::Button {
                name: "a",
                down: false
            }]
        );
    }

    #[test]
    fn a_pulled_trigger_counts_as_its_button() {
        let mut controller = Controller::default();
        let pulled = PadState {
            rt: 0.6,
            ..Default::default()
        };
        controller.take_input(Some((pulled, PadKind::Xbox)), 0.0);
        assert!(controller.events.contains(&PadEvent::Button {
            name: "rt",
            down: true
        }));
    }

    #[test]
    fn keyboard_use_hides_the_pad_kind() {
        let mut controller = Controller::default();
        let moved = PadState {
            lx: 0.9,
            ..Default::default()
        };
        controller.take_input(Some((moved, PadKind::Xbox)), 0.0);
        assert_eq!(controller.shown_kind(), PadKind::Xbox);
        controller.use_keyboard_and_mouse();
        assert_eq!(controller.shown_kind(), PadKind::None);
        assert_eq!(controller.connected, PadKind::Xbox);
    }

    #[test]
    fn the_deadzone_rescales_from_its_edge() {
        assert_eq!(radial_deadzone(0.1, 0.1, 0.24), Vec2::ZERO);
        let full = radial_deadzone(1.0, 0.0, 0.24);
        assert!((full.x - 1.0).abs() < 1e-6);
        let half = radial_deadzone(0.62, 0.0, 0.24);
        assert!((half.x - 0.5).abs() < 1e-3);
    }
}
