//! Device manager: hotplug detection and one input thread per device. Never blocks the UI thread.

use crate::db::ControllerDb;
use crate::gamepad::{self, GamepadDriver, PadId, SharedStates};
use crate::model::*;
use crate::{ps4, ps5};
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Events delivered to the UI.
pub enum Event {
    Added(ControllerInfo, Capabilities, DeviceHandle),
    Removed(String),
    Input(String, Box<InputState>),
    Error(String, String),
}

/// Upper bound for the per-device polling rate in Hz (0 = as fast as the device delivers).
pub static POLL_HZ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(250);

type Job = Box<dyn FnOnce(&mut dyn ControllerDriver) + Send>;

/// Handle to run closures on the device thread.
#[derive(Clone)]
pub struct DeviceHandle {
    tx: Sender<Job>,
}

impl DeviceHandle {
    /// Run `f` on the device thread; its result is returned through the channel (non-blocking for the caller).
    pub fn exec<T: Send + 'static>(&self, f: impl FnOnce(&mut dyn ControllerDriver) -> T + Send + 'static) -> Receiver<T> {
        let (rtx, rrx) = crossbeam_channel::bounded(1);
        let _ = self.tx.send(Box::new(move |d| {
            let _ = rtx.send(f(d));
        }));
        rrx
    }
}

/// Spawn the input thread for a driver. Used for hardware devices and the demo mock.
pub fn spawn_device(mut driver: Box<dyn ControllerDriver>, events: Sender<Event>, stop: Arc<AtomicBool>) -> ControllerInfo {
    let info = driver.info();
    let caps = driver.capabilities();
    let (jtx, jrx) = unbounded::<Job>();
    let handle = DeviceHandle { tx: jtx };
    let _ = events.send(Event::Added(info.clone(), caps, handle));
    let id = info.id.clone();
    std::thread::Builder::new()
        .name(format!("driftlab-input-{id}"))
        .spawn(move || {
            let mut errs = 0u32;
            let mut last = std::time::Instant::now();
            while !stop.load(Ordering::Relaxed) {
                while let Ok(job) = jrx.try_recv() {
                    job(driver.as_mut());
                }
                match driver.poll() {
                    Ok(s) => {
                        errs = 0;
                        let _ = events.send(Event::Input(id.clone(), Box::new(s)));
                        let hz = POLL_HZ.load(Ordering::Relaxed);
                        if hz > 0 {
                            let tick = Duration::from_micros(1_000_000 / u64::from(hz));
                            if let Some(rest) = tick.checked_sub(last.elapsed()) {
                                std::thread::sleep(rest);
                            }
                            last = std::time::Instant::now();
                        }
                    }
                    Err(DriverError::Disconnected) | Err(DriverError::Hid(_)) => {
                        errs += 1;
                        if errs > 25 {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(2)),
                }
            }
            let _ = driver.abort();
            let _ = events.send(Event::Removed(id));
        })
        .ok();
    info
}

pub struct Manager {
    pub events: Receiver<Event>,
    tx: Sender<Event>,
    stop: Arc<AtomicBool>,
}

impl Manager {
    pub fn start() -> Self {
        let (tx, rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let (s2, tx2) = (stop.clone(), tx.clone());
        std::thread::Builder::new().name("driftlab-hotplug".into()).spawn(move || hotplug_loop(tx2, s2)).ok();
        Self { events: rx, tx, stop }
    }

    /// Event sender, e.g. to attach a demo device to the same event stream.
    pub fn sender(&self) -> Sender<Event> {
        self.tx.clone()
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.stop();
    }
}

fn connection_of(iface: &hidapi::DeviceInfo) -> Connection {
    match iface.bus_type() {
        hidapi::BusType::Usb => Connection::Usb,
        hidapi::BusType::Bluetooth => Connection::Bluetooth,
        _ => {
            // Fallback heuristic: Bluetooth HID paths on Windows contain "bth"/"{00001124".
            let p = iface.path().to_string_lossy().to_lowercase();
            if p.contains("bth") || p.contains("00001124") || p.contains("bluetooth") {
                Connection::Bluetooth
            } else {
                Connection::Usb
            }
        }
    }
}

fn hotplug_loop(tx: Sender<Event>, stop: Arc<AtomicBool>) {
    let db = ControllerDb::builtin();
    let mut api = match hidapi::HidApi::new() {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::warn!("hidapi init failed: {e}");
            None
        }
    };
    // Gamepad hub (Xbox + generic).
    let states: SharedStates = Arc::new(Mutex::new(HashMap::new()));
    let alive = Arc::new(Mutex::new(Vec::new()));
    let (ptx, prx) = unbounded::<(PadId, bool)>();
    {
        let (s, a, st) = (states.clone(), alive.clone(), stop.clone());
        std::thread::Builder::new().name("driftlab-gilrs".into()).spawn(move || gamepad::run_hub(s, a, ptx, st)).ok();
    }
    let mut running: HashMap<String, Arc<AtomicBool>> = HashMap::new();
    let mut pad_stops: HashMap<usize, (String, Arc<AtomicBool>)> = HashMap::new();
    while !stop.load(Ordering::Relaxed) {
        if let Some(api) = api.as_mut() {
            if api.refresh_devices().is_ok() {
                let mut seen = Vec::new();
                for d in api.device_list() {
                    let Some(entry) = db.lookup(d.vendor_id(), d.product_id()) else { continue };
                    if !matches!(entry.family, Family::DualSense | Family::DualSenseEdge | Family::DualShock4) {
                        continue; // Xbox handled by gilrs
                    }
                    // Only the gamepad collection (usage page 1, usage 5).
                    if d.usage_page() != 0 && !(d.usage_page() == 1 && d.usage() == 5) {
                        continue;
                    }
                    let path = d.path().to_string_lossy().to_string();
                    seen.push(path.clone());
                    if running.contains_key(&path) {
                        continue;
                    }
                    match api.open_path(d.path()) {
                        Ok(dev) => {
                            let conn = connection_of(d);
                            let serial = d.serial_number().map(|s| s.to_string());
                            let info = ControllerInfo {
                                id: path.clone(),
                                name: entry.name.clone(),
                                family: entry.family,
                                vid: d.vendor_id(),
                                pid: d.product_id(),
                                connection: conn,
                                firmware: None,
                                serial: serial.map(|s| format_mac(&s)),
                            };
                            let mut info = info;
                            let driver: Box<dyn ControllerDriver> = match entry.family {
                                Family::DualShock4 => {
                                    info.firmware = read_fw(&dev, 0xA3, 49).and_then(|b| ps4::parse_fw(&b));
                                    Box::new(ps4::DualShock4Driver::new(dev, info))
                                }
                                _ => {
                                    info.firmware = read_fw(&dev, 0x20, 64).and_then(|b| ps5::parse_fw(&b));
                                    Box::new(ps5::DualSenseDriver::new(dev, info))
                                }
                            };
                            let s = Arc::new(AtomicBool::new(false));
                            running.insert(path.clone(), s.clone());
                            spawn_device(driver, tx.clone(), s);
                        }
                        Err(e) => {
                            let _ = tx.send(Event::Error(path.clone(), format!("Cannot open controller: {e}")));
                            running.insert(path, Arc::new(AtomicBool::new(true)));
                        }
                    }
                }
                // Anything that vanished from the device list: stop its thread.
                running.retain(|p, s| {
                    if seen.contains(p) {
                        true
                    } else {
                        s.store(true, Ordering::Relaxed);
                        false
                    }
                });
            }
        }
        while let Ok((pad, added)) = prx.try_recv() {
            if added {
                let Some(entry) = db.lookup(pad.vid, pad.pid) else {
                    // Unknown generic pad (but skip Sony pads, which hidapi handles).
                    if pad.vid == 0x054C {
                        continue;
                    }
                    let s = Arc::new(AtomicBool::new(false));
                    pad_stops.insert(pad.id, (format!("pad-{}", pad.id), s.clone()));
                    spawn_device(Box::new(GamepadDriver::new(pad, Family::Generic, states.clone(), alive.clone())), tx.clone(), s);
                    continue;
                };
                if matches!(entry.family, Family::DualSense | Family::DualSenseEdge | Family::DualShock4) {
                    continue;
                }
                let s = Arc::new(AtomicBool::new(false));
                pad_stops.insert(pad.id, (format!("pad-{}", pad.id), s.clone()));
                spawn_device(Box::new(GamepadDriver::new(pad.clone(), entry.family, states.clone(), alive.clone())), tx.clone(), s);
            } else if let Some((_, s)) = pad_stops.remove(&pad.id) {
                s.store(true, Ordering::Relaxed);
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    for s in running.values() {
        s.store(true, Ordering::Relaxed);
    }
    for (_, s) in pad_stops.values() {
        s.store(true, Ordering::Relaxed);
    }
}

fn read_fw(dev: &hidapi::HidDevice, id: u8, len: usize) -> Option<Vec<u8>> {
    let mut b = vec![0u8; len];
    b[0] = id;
    let n = dev.get_feature_report(&mut b).ok()?;
    b.truncate(n);
    Some(b)
}

/// Format 12 hex chars as `AA:BB:CC:DD:EE:FF`; pass anything else through.
pub fn format_mac(s: &str) -> String {
    let h: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if h.len() == 12 && h.len() == s.len() {
        h.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_uppercase()).collect::<Vec<_>>().join(":")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_formatting() {
        assert_eq!(format_mac("a411223344ff"), "A4:11:22:33:44:FF");
        assert_eq!(format_mac("SERIAL-1"), "SERIAL-1");
    }

    #[test]
    fn mock_device_lifecycle() {
        let (tx, rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        spawn_device(Box::new(crate::mock::MockDriver::new(Family::DualSense)), tx, stop.clone());
        let mut got_added = false;
        let mut got_input = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline && !(got_added && got_input) {
            if let Ok(ev) = rx.recv_timeout(Duration::from_millis(100)) {
                match ev {
                    Event::Added(..) => got_added = true,
                    Event::Input(..) => got_input = true,
                    _ => {}
                }
            }
        }
        assert!(got_added && got_input);
        stop.store(true, Ordering::Relaxed);
        let mut removed = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline && !removed {
            if let Ok(Event::Removed(_)) = rx.recv_timeout(Duration::from_millis(100)) {
                removed = true;
            }
        }
        assert!(removed);
    }
}
