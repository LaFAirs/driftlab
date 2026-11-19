//! Shared data types for controllers and input.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from drivers and calibration.
#[derive(Debug, Error)]
pub enum DriverError {
    #[error("HID error: {0}")]
    Hid(String),
    #[error("device disconnected")]
    Disconnected,
    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),
    #[error("malformed report: {0}")]
    Parse(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("safety check failed: {0}")]
    Safety(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DriverError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Family {
    DualSense,
    DualSenseEdge,
    DualShock4,
    Xbox,
    XboxElite,
    Generic,
}

impl Family {
    pub fn theme_key(self) -> &'static str {
        match self {
            Family::DualSense => "ps5",
            Family::DualSenseEdge => "ps5_edge",
            Family::DualShock4 => "ps4",
            Family::Xbox | Family::XboxElite => "xbox",
            Family::Generic => "neutral",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Connection {
    Usb,
    Bluetooth,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerInfo {
    /// Unique, stable id for the session (path or serial based).
    pub id: String,
    pub name: String,
    pub family: Family,
    pub vid: u16,
    pub pid: u16,
    pub connection: Connection,
    pub firmware: Option<String>,
    /// Serial number or MAC address.
    pub serial: Option<String>,
}

impl ControllerInfo {
    /// Serial/MAC with the middle masked, e.g. `A4:••:••:••:3F:1C`.
    pub fn masked_serial(&self) -> String {
        mask_serial(self.serial.as_deref().unwrap_or("unknown"))
    }
}

pub fn mask_serial(s: &str) -> String {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() >= 4 {
        let n = parts.len();
        parts
            .iter()
            .enumerate()
            .map(|(i, p)| if i == 0 || i >= n - 2 { (*p).to_string() } else { "••".to_string() })
            .collect::<Vec<_>>()
            .join(":")
    } else if s.chars().count() > 6 {
        let c: Vec<char> = s.chars().collect();
        let head: String = c[..2].iter().collect();
        let tail: String = c[c.len() - 2..].iter().collect();
        format!("{head}••••{tail}")
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub hw_calibration: bool,
    pub triggers: bool,
    pub gyro: bool,
    pub touchpad: bool,
    pub rumble: bool,
    pub lightbar: bool,
    pub battery: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChargeState {
    Discharging,
    Charging,
    Full,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Battery {
    pub percent: u8,
    pub state: ChargeState,
}

/// Snapshot of controller input. Sticks/triggers normalised; raw values kept for diagnostics.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct InputState {
    pub left: (f32, f32),
    pub right: (f32, f32),
    pub raw_left: (f32, f32),
    pub raw_right: (f32, f32),
    pub l2: f32,
    pub r2: f32,
    /// Button name → pressed.
    pub buttons: Vec<(String, bool)>,
    pub gyro: Option<[f32; 3]>,
    pub accel: Option<[f32; 3]>,
    pub touch: Vec<(f32, f32)>,
    pub battery: Option<Battery>,
    pub raw_report: Vec<u8>,
}

impl InputState {
    pub fn button(&self, name: &str) -> bool {
        self.buttons.iter().any(|(n, p)| n == name && *p)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalibKind {
    Center,
    Range,
}

/// Opaque calibration backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationBlob {
    pub family: Family,
    pub reports: Vec<(u8, Vec<u8>)>,
}

/// Handle of a running hardware calibration session.
#[derive(Debug)]
pub struct CalibSession {
    pub kind: CalibKind,
    pub samples: u32,
}

/// Rumble / lightbar output request.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OutputRequest {
    pub rumble_left: u8,
    pub rumble_right: u8,
    pub lightbar: Option<(u8, u8, u8)>,
}

pub trait ControllerDriver: Send {
    fn info(&self) -> ControllerInfo;
    fn capabilities(&self) -> Capabilities;
    fn poll(&mut self) -> Result<InputState>;
    fn read_calibration(&mut self) -> Result<Option<CalibrationBlob>> {
        Ok(None)
    }
    fn begin_calibration(&mut self, _kind: CalibKind) -> Result<CalibSession> {
        Err(DriverError::Unsupported("hardware calibration"))
    }
    /// Take one calibration sample (device-side averaging step).
    fn sample(&mut self, _s: &mut CalibSession) -> Result<()> {
        Err(DriverError::Unsupported("hardware calibration"))
    }
    /// Finish the current calibration step; result is applied temporarily (until controller reset).
    fn apply_temporary(&mut self, _s: CalibSession) -> Result<()> {
        Err(DriverError::Unsupported("hardware calibration"))
    }
    /// Persist the temporarily applied calibration to flash.
    fn commit_permanent(&mut self) -> Result<()> {
        Err(DriverError::Unsupported("permanent calibration"))
    }
    /// Abort and return to a safe state.
    fn abort(&mut self) -> Result<()> {
        Ok(())
    }
    fn restore(&mut self, _blob: &CalibrationBlob) -> Result<()> {
        Err(DriverError::Unsupported("restore"))
    }
    fn set_output(&mut self, _o: OutputRequest) -> Result<()> {
        Err(DriverError::Unsupported("output"))
    }
}
