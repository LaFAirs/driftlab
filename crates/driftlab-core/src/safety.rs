//! Pre-flight safety checks that gate every write to controller flash.

use crate::model::*;

pub const MIN_BATTERY_PERCENT: u8 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub key: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preflight {
    pub checks: Vec<Check>,
    /// Hardware calibration is supported at all for this device.
    pub hw_supported: bool,
}

impl Preflight {
    pub fn blocked(&self) -> bool {
        self.checks.iter().any(|c| c.status == CheckStatus::Block)
    }
    pub fn has_warning(&self) -> bool {
        self.checks.iter().any(|c| c.status == CheckStatus::Warn)
    }
}

/// Run all pre-flight checks.
///
/// * writes require USB (Bluetooth → block with a "connect via USB" hint)
/// * battery < 30 % blocks unless the device is charging over USB
/// * unknown firmware blocks unless expert mode is on
pub fn preflight(info: &ControllerInfo, caps: &Capabilities, battery: Option<Battery>, expert: bool) -> Preflight {
    let mut checks = Vec::new();
    checks.push(Check {
        key: "hw_support",
        status: if caps.hw_calibration { CheckStatus::Pass } else { CheckStatus::Warn },
        detail: if caps.hw_calibration {
            "Hardware calibration supported".into()
        } else {
            "No open calibration protocol; software correction will be used".into()
        },
    });
    checks.push(match info.connection {
        Connection::Usb | Connection::Virtual => Check { key: "usb", status: CheckStatus::Pass, detail: "Connected via USB".into() },
        _ => Check { key: "usb", status: CheckStatus::Block, detail: "Please connect via USB cable".into() },
    });
    checks.push(match battery {
        Some(b) if b.percent < MIN_BATTERY_PERCENT && b.state != ChargeState::Charging && b.state != ChargeState::Full => Check {
            key: "battery",
            status: CheckStatus::Block,
            detail: format!("Battery at {} % (minimum {} %)", b.percent, MIN_BATTERY_PERCENT),
        },
        Some(b) if b.percent < MIN_BATTERY_PERCENT => {
            Check { key: "battery", status: CheckStatus::Warn, detail: format!("Battery at {} % but charging", b.percent) }
        }
        Some(b) => Check { key: "battery", status: CheckStatus::Pass, detail: format!("Battery at {} %", b.percent) },
        None => Check { key: "battery", status: CheckStatus::Warn, detail: "Battery level unknown".into() },
    });
    checks.push(match &info.firmware {
        Some(_) => Check { key: "firmware", status: CheckStatus::Pass, detail: "Firmware version recognised".into() },
        None if expert => Check { key: "firmware", status: CheckStatus::Warn, detail: "Unknown firmware – expert mode override".into() },
        None => {
            Check { key: "firmware", status: CheckStatus::Block, detail: "Unknown firmware – write access only in expert mode".into() }
        }
    });
    Preflight { checks, hw_supported: caps.hw_calibration }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(conn: Connection, fw: Option<&str>) -> ControllerInfo {
        ControllerInfo {
            id: "x".into(),
            name: "x".into(),
            family: Family::DualSense,
            vid: 0,
            pid: 0,
            connection: conn,
            firmware: fw.map(String::from),
            serial: None,
        }
    }
    fn caps() -> Capabilities {
        Capabilities { hw_calibration: true, ..Default::default() }
    }
    fn bat(p: u8, s: ChargeState) -> Option<Battery> {
        Some(Battery { percent: p, state: s })
    }

    #[test]
    fn all_good() {
        let p = preflight(&info(Connection::Usb, Some("1")), &caps(), bat(80, ChargeState::Charging), false);
        assert!(!p.blocked());
    }

    #[test]
    fn bluetooth_blocks() {
        assert!(preflight(&info(Connection::Bluetooth, Some("1")), &caps(), bat(80, ChargeState::Discharging), false).blocked());
    }

    #[test]
    fn low_battery_blocks_unless_charging() {
        assert!(preflight(&info(Connection::Usb, Some("1")), &caps(), bat(20, ChargeState::Discharging), false).blocked());
        assert!(!preflight(&info(Connection::Usb, Some("1")), &caps(), bat(20, ChargeState::Charging), false).blocked());
    }

    #[test]
    fn unknown_firmware_needs_expert() {
        assert!(preflight(&info(Connection::Usb, None), &caps(), bat(80, ChargeState::Full), false).blocked());
        let p = preflight(&info(Connection::Usb, None), &caps(), bat(80, ChargeState::Full), true);
        assert!(!p.blocked() && p.has_warning());
    }
}
