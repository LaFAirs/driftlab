//! Xbox and generic gamepads through `gilrs` (XInput/WGI on Windows, evdev on Linux, IOKit on macOS).
//! Xbox controllers expose no open calibration protocol, so only reading + software correction is offered.

use crate::math::norm_i16;
use crate::model::*;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Latest state per gilrs gamepad id, written by the hub thread.
pub type SharedStates = Arc<Mutex<HashMap<usize, InputState>>>;

/// Identity of a gamepad reported by gilrs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadId {
    pub id: usize,
    pub name: String,
    pub vid: u16,
    pub pid: u16,
}

/// Driver reading from the shared hub state.
pub struct GamepadDriver {
    pad: PadId,
    family: Family,
    states: SharedStates,
    alive: Arc<Mutex<Vec<usize>>>,
}

impl GamepadDriver {
    pub fn new(pad: PadId, family: Family, states: SharedStates, alive: Arc<Mutex<Vec<usize>>>) -> Self {
        Self { pad, family, states, alive }
    }
}

impl ControllerDriver for GamepadDriver {
    fn info(&self) -> ControllerInfo {
        ControllerInfo {
            id: format!("pad-{}", self.pad.id),
            name: self.pad.name.clone(),
            family: self.family,
            vid: self.pad.vid,
            pid: self.pad.pid,
            connection: Connection::Unknown,
            firmware: None,
            serial: Some(format!("{:04X}:{:04X}:{}", self.pad.vid, self.pad.pid, self.pad.id)),
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { triggers: true, rumble: false, ..Default::default() }
    }

    fn poll(&mut self) -> Result<InputState> {
        std::thread::sleep(std::time::Duration::from_millis(4));
        if !self.alive.lock().contains(&self.pad.id) {
            return Err(DriverError::Disconnected);
        }
        self.states.lock().get(&self.pad.id).cloned().ok_or_else(|| DriverError::Parse("no data yet".into()))
    }
}

/// Run the gilrs hub on the current thread (blocking). Calls `on_change` whenever the pad list changes.
pub fn run_hub(
    states: SharedStates,
    alive: Arc<Mutex<Vec<usize>>>,
    pads_tx: crossbeam_channel::Sender<(PadId, bool)>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    use gilrs::{Axis, Button, EventType, Gilrs};
    let mut g = match Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("gilrs unavailable: {e}");
            return;
        }
    };
    let mut known: HashMap<usize, PadId> = HashMap::new();
    let sync = |g: &Gilrs, known: &mut HashMap<usize, PadId>| {
        let mut ids = Vec::new();
        for (id, pad) in g.gamepads() {
            if !pad.is_connected() {
                continue;
            }
            let uid: usize = id.into();
            ids.push(uid);
            known.entry(uid).or_insert_with(|| {
                let p =
                    PadId { id: uid, name: pad.name().to_string(), vid: pad.vendor_id().unwrap_or(0), pid: pad.product_id().unwrap_or(0) };
                let _ = pads_tx.send((p.clone(), true));
                p
            });
        }
        let gone: Vec<usize> = known.keys().copied().filter(|k| !ids.contains(k)).collect();
        for k in gone {
            if let Some(p) = known.remove(&k) {
                let _ = pads_tx.send((p, false));
            }
            states.lock().remove(&k);
        }
        *alive.lock() = ids;
    };
    sync(&g, &mut known);
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let mut changed = false;
        while let Some(ev) = g.next_event() {
            if matches!(ev.event, EventType::Connected | EventType::Disconnected) {
                changed = true;
            }
        }
        if changed {
            sync(&g, &mut known);
        }
        for (id, pad) in g.gamepads() {
            if !pad.is_connected() {
                continue;
            }
            let uid: usize = id.into();
            let a = |ax: Axis| pad.value(ax).clamp(-1.0, 1.0);
            let (lx, ly, rx, ry) = (a(Axis::LeftStickX), a(Axis::LeftStickY), a(Axis::RightStickX), a(Axis::RightStickY));
            let trig = |b: Button, ax: Axis| {
                let v = pad.button_data(b).map(|d| d.value()).unwrap_or_else(|| a(ax).max(0.0));
                v.clamp(0.0, 1.0)
            };
            let names = [
                (Button::South, "a"),
                (Button::East, "b"),
                (Button::West, "x"),
                (Button::North, "y"),
                (Button::LeftTrigger, "lb"),
                (Button::RightTrigger, "rb"),
                (Button::Select, "view"),
                (Button::Start, "menu"),
                (Button::Mode, "guide"),
                (Button::LeftThumb, "l3"),
                (Button::RightThumb, "r3"),
                (Button::DPadUp, "up"),
                (Button::DPadDown, "down"),
                (Button::DPadLeft, "left"),
                (Button::DPadRight, "right"),
            ];
            let st = InputState {
                left: (lx, ly),
                right: (rx, ry),
                raw_left: (lx, ly),
                raw_right: (rx, ry),
                l2: trig(Button::LeftTrigger2, Axis::LeftZ),
                r2: trig(Button::RightTrigger2, Axis::RightZ),
                buttons: names.iter().map(|(b, n)| ((*n).to_string(), pad.is_pressed(*b))).collect(),
                ..Default::default()
            };
            let _ = norm_i16(0);
            states.lock().insert(uid, st);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
