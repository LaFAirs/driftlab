//! DualShock 4 driver. Protocol sources: dualshock-tools (MIT), Linux `hid-sony.c`, SDL `SDL_hidapi_ps4.c`.

use crate::hidio::HidIo;
use crate::math::{norm_i16, norm_u8};
use crate::model::*;

pub const REPORT_USB: u8 = 0x01;
pub const REPORT_BT: u8 = 0x11;

/// Parse a DS4 input report. `buf[0]` is the report id (0x01 for USB, 0x11 for Bluetooth).
pub fn parse_input(buf: &[u8]) -> Result<InputState> {
    let (off, usb) = match buf.first() {
        Some(&REPORT_USB) => (1usize, true),
        Some(&REPORT_BT) => (3usize, false),
        other => return Err(DriverError::Parse(format!("unexpected DS4 report id {other:?}"))),
    };
    if buf.len() < off + 30 {
        return Err(DriverError::Parse(format!("DS4 report too short ({} bytes)", buf.len())));
    }
    let d = &buf[off..];
    let axis = |i: usize| norm_u8(d[i]);
    // DS4 Y axis is down-positive; flip so up is +1.
    let (lx, ly, rx, ry) = (axis(0), -axis(1), axis(2), -axis(3));
    let b1 = d[4];
    let b2 = d[5];
    let b3 = d[6];
    let dpad = b1 & 0x0F;
    let mut buttons = vec![
        ("square".to_string(), b1 & 0x10 != 0),
        ("cross".to_string(), b1 & 0x20 != 0),
        ("circle".to_string(), b1 & 0x40 != 0),
        ("triangle".to_string(), b1 & 0x80 != 0),
        ("l1".to_string(), b2 & 0x01 != 0),
        ("r1".to_string(), b2 & 0x02 != 0),
        ("l2".to_string(), b2 & 0x04 != 0),
        ("r2".to_string(), b2 & 0x08 != 0),
        ("share".to_string(), b2 & 0x10 != 0),
        ("options".to_string(), b2 & 0x20 != 0),
        ("l3".to_string(), b2 & 0x40 != 0),
        ("r3".to_string(), b2 & 0x80 != 0),
        ("home".to_string(), b3 & 0x01 != 0),
        ("touchpad".to_string(), b3 & 0x02 != 0),
    ];
    for (name, set) in [
        ("up", matches!(dpad, 7 | 0 | 1)),
        ("right", matches!(dpad, 1..=3)),
        ("down", matches!(dpad, 3..=5)),
        ("left", matches!(dpad, 5..=7)),
    ] {
        buttons.push((name.to_string(), set));
    }
    let gyro_i = |i: usize| i16::from_le_bytes([d[i], d[i + 1]]);
    let gyro = [norm_i16(gyro_i(12)), norm_i16(gyro_i(14)), norm_i16(gyro_i(16))];
    let accel = [norm_i16(gyro_i(18)), norm_i16(gyro_i(20)), norm_i16(gyro_i(22))];
    let status = d[29];
    let cable = status & 0x10 != 0;
    let lvl = u32::from(status & 0x0F);
    let battery = if usb || cable {
        let pct = (lvl * 10).min(100) as u8;
        Some(Battery { percent: pct, state: if lvl >= 11 { ChargeState::Full } else { ChargeState::Charging } })
    } else {
        Some(Battery { percent: ((lvl * 100) / 8).min(100) as u8, state: ChargeState::Discharging })
    };
    Ok(InputState {
        left: (lx, ly),
        right: (rx, ry),
        raw_left: (f32::from(d[0]), f32::from(d[1])),
        raw_right: (f32::from(d[2]), f32::from(d[3])),
        l2: f32::from(d[7]) / 255.0,
        r2: f32::from(d[8]) / 255.0,
        buttons,
        gyro: Some(gyro),
        accel: Some(accel),
        touch: Vec::new(),
        battery,
        raw_report: buf.to_vec(),
    })
}

/// Parse firmware (hw, fw) from feature report 0xA3.
pub fn parse_fw(buf: &[u8]) -> Option<String> {
    if buf.len() < 45 || buf[0] != 0xA3 {
        return None;
    }
    let hw = u16::from_le_bytes([buf[35], buf[36]]);
    let fw = u16::from_le_bytes([buf[41], buf[42]]);
    if fw == 0 {
        return None;
    }
    Some(format!("fw {fw:#06x} / hw {hw:#06x}"))
}

/// Expected replies (0x91, 0x92) for a calibration step.
fn expect(kind: CalibKind, step: u8) -> ([u8; 4], [u8; 4]) {
    // step: 1 begin/sample, 2 end
    let k = if kind == CalibKind::Center { 1 } else { 2 };
    match (kind, step) {
        (CalibKind::Center, 2) => ([0x91, 1, 1, 2], [0x92, 1, 1, 1]),
        (CalibKind::Range, 2) => ([0x91, 1, 2, 2], [0x92, 1, 2, 1]),
        _ => ([0x91, 1, k, 1], [0x92, 1, k, 0xFF]),
    }
}

pub struct DualShock4Driver<H: HidIo> {
    io: H,
    info: ControllerInfo,
    in_calibration: bool,
    nvs_unlocked: bool,
}

impl<H: HidIo> DualShock4Driver<H> {
    pub fn new(io: H, info: ControllerInfo) -> Self {
        Self { io, info, in_calibration: false, nvs_unlocked: false }
    }

    fn step(&mut self, payload: [u8; 3], kind: CalibKind, step: u8) -> Result<()> {
        self.io.send_feature(&[0x90, payload[0], payload[1], payload[2]])?;
        let (e1, e2) = expect(kind, step);
        for exp in [e1, e2] {
            let mut b = [0u8; 8];
            b[0] = exp[0];
            let n = self.io.get_feature(&mut b)?;
            if n < 4 || b[..4] != exp {
                return Err(DriverError::Protocol(format!(
                    "unexpected calibration reply for report {:#04x}: {:02x?}",
                    exp[0],
                    &b[..n.min(8)]
                )));
            }
        }
        Ok(())
    }

    fn nvs_unlock(&mut self) -> Result<()> {
        self.io.send_feature(&[0xA0, 10, 2, 0x3E, 0x71, 0x7F, 0x89])?;
        self.nvs_unlocked = true;
        Ok(())
    }

    fn nvs_lock(&mut self) -> Result<()> {
        self.io.send_feature(&[0xA0, 10, 1, 0])?;
        self.nvs_unlocked = false;
        Ok(())
    }

    /// Report whether flash is currently unlocked (permanent) or locked (temporary).
    pub fn nvs_is_locked(&mut self) -> Result<bool> {
        self.io.send_feature(&[0x08, 0xFF, 0, 12])?;
        let mut b = [0u8; 16];
        b[0] = 0x11;
        let n = self.io.get_feature(&mut b)?;
        if n < 2 {
            return Err(DriverError::Protocol("short NVS status".into()));
        }
        Ok(b[1] == 1)
    }
}

impl<H: HidIo> ControllerDriver for DualShock4Driver<H> {
    fn info(&self) -> ControllerInfo {
        self.info.clone()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            hw_calibration: self.info.connection == Connection::Usb || self.info.connection == Connection::Bluetooth,
            triggers: true,
            gyro: true,
            touchpad: true,
            rumble: true,
            lightbar: true,
            battery: true,
        }
    }

    fn poll(&mut self) -> Result<InputState> {
        let mut buf = [0u8; 78];
        let n = self.io.read_timeout(&mut buf, 20)?;
        if n == 0 {
            return Err(DriverError::Parse("timeout".into()));
        }
        parse_input(&buf[..n])
    }

    fn read_calibration(&mut self) -> Result<Option<CalibrationBlob>> {
        // DS4 stick calibration is write-only through the known protocol; back up what is readable.
        let mut reports = Vec::new();
        for id in [0x02u8, 0xA3] {
            let mut b = [0u8; 64];
            b[0] = id;
            let n = self.io.get_feature(&mut b)?;
            reports.push((id, b[..n].to_vec()));
        }
        Ok(Some(CalibrationBlob { family: Family::DualShock4, reports }))
    }

    fn begin_calibration(&mut self, kind: CalibKind) -> Result<CalibSession> {
        let p = if kind == CalibKind::Center { [1, 1, 1] } else { [1, 1, 2] };
        if let Err(e) = self.step(p, kind, 1) {
            let _ = self.abort();
            return Err(e);
        }
        self.in_calibration = true;
        Ok(CalibSession { kind, samples: 0 })
    }

    fn sample(&mut self, s: &mut CalibSession) -> Result<()> {
        if s.kind == CalibKind::Center {
            if let Err(e) = self.step([3, 1, 1], CalibKind::Center, 1) {
                let _ = self.abort();
                return Err(e);
            }
        }
        s.samples += 1;
        Ok(())
    }

    fn apply_temporary(&mut self, s: CalibSession) -> Result<()> {
        let p = if s.kind == CalibKind::Center { [2, 1, 1] } else { [2, 1, 2] };
        let r = self.step(p, s.kind, 2);
        if r.is_err() {
            let _ = self.abort();
        }
        self.in_calibration = false;
        r
    }

    fn commit_permanent(&mut self) -> Result<()> {
        self.nvs_unlock()?;
        // Always try to relock even if something goes wrong in between.
        self.nvs_lock()
    }

    fn abort(&mut self) -> Result<()> {
        self.in_calibration = false;
        let mut result = Ok(());
        if self.nvs_unlocked {
            result = self.nvs_lock();
        }
        // Best-effort: end both calibration modes so the device returns to normal operation.
        let _ = self.io.send_feature(&[0x90, 2, 1, 1]);
        let _ = self.io.send_feature(&[0x90, 2, 1, 2]);
        result
    }

    fn set_output(&mut self, o: OutputRequest) -> Result<()> {
        if self.info.connection != Connection::Usb {
            return Err(DriverError::Unsupported("output over Bluetooth"));
        }
        let (r, g, b) = o.lightbar.unwrap_or((0, 0, 255));
        let mut rep = [0u8; 32];
        rep[0] = 0x05;
        rep[1] = 0xFF;
        rep[4] = o.rumble_right;
        rep[5] = o.rumble_left;
        rep[6] = r;
        rep[7] = g;
        rep[8] = b;
        self.io.write(&rep)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidio::fake::FakeHid;

    fn info(conn: Connection) -> ControllerInfo {
        ControllerInfo {
            id: "t".into(),
            name: "DS4".into(),
            family: Family::DualShock4,
            vid: 0x54C,
            pid: 0x9CC,
            connection: conn,
            firmware: None,
            serial: None,
        }
    }

    fn usb_report() -> Vec<u8> {
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[1..5].copy_from_slice(&[128, 128, 255, 0]);
        r[5] = 0x08 | 0x20;
        r[8] = 255;
        r[30] = 0x18;
        r
    }

    #[test]
    fn parses_usb_report() {
        let s = parse_input(&usb_report()).unwrap();
        assert!(s.left.0.abs() < 0.01 && s.left.1.abs() < 0.01);
        assert!((s.right.0 - 1.0).abs() < 1e-3);
        assert!((s.right.1 - 1.0).abs() < 1e-3, "y axis is inverted");
        assert!(s.button("cross"));
        assert!(!s.button("square"));
        assert!((s.l2 - 1.0).abs() < 1e-6);
        assert!(!s.button("up"));
        assert_eq!(s.battery.unwrap().state, ChargeState::Charging);
    }

    #[test]
    fn parses_bt_report_offset() {
        let mut r = vec![0u8; 78];
        r[0] = 0x11;
        r[3..7].copy_from_slice(&[0, 128, 128, 128]);
        let s = parse_input(&r).unwrap();
        assert!((s.left.0 + 1.0).abs() < 1e-3);
    }

    #[test]
    fn rejects_bad_reports() {
        assert!(parse_input(&[]).is_err());
        assert!(parse_input(&[0x01, 0, 0]).is_err());
        assert!(parse_input(&[0x55; 64]).is_err());
    }

    #[test]
    fn dpad_decoding() {
        let mut r = usb_report();
        r[5] = 1; // up-right
        let s = parse_input(&r).unwrap();
        assert!(s.button("up") && s.button("right") && !s.button("down"));
    }

    fn scripted() -> FakeHid {
        let mut f = FakeHid::default();
        f.features.insert(0x91, vec![0x91, 1, 1, 1]);
        f.features.insert(0x92, vec![0x92, 1, 1, 0xFF]);
        f
    }

    #[test]
    fn center_calibration_sequence() {
        let mut d = DualShock4Driver::new(scripted(), info(Connection::Usb));
        let mut s = d.begin_calibration(CalibKind::Center).unwrap();
        d.sample(&mut s).unwrap();
        assert_eq!(d.io.sent[0], vec![0x90, 1, 1, 1]);
        assert_eq!(d.io.sent[1], vec![0x90, 3, 1, 1]);
        d.io.features.insert(0x91, vec![0x91, 1, 1, 2]);
        d.io.features.insert(0x92, vec![0x92, 1, 1, 1]);
        d.apply_temporary(s).unwrap();
        assert_eq!(d.io.sent.last().unwrap(), &vec![0x90, 2, 1, 1]);
    }

    #[test]
    fn unexpected_reply_aborts_safely() {
        let mut f = FakeHid::default();
        f.features.insert(0x91, vec![0x91, 9, 9, 9]);
        let mut d = DualShock4Driver::new(f, info(Connection::Usb));
        assert!(d.begin_calibration(CalibKind::Center).is_err());
        // abort sent end-of-calibration commands
        assert!(d.io.sent.iter().any(|p| p == &vec![0x90, 2, 1, 1]));
        assert!(!d.in_calibration);
    }

    #[test]
    fn commit_unlocks_then_locks() {
        let mut d = DualShock4Driver::new(FakeHid::default(), info(Connection::Usb));
        d.commit_permanent().unwrap();
        assert_eq!(d.io.sent[0], vec![0xA0, 10, 2, 0x3E, 0x71, 0x7F, 0x89]);
        assert_eq!(d.io.sent[1], vec![0xA0, 10, 1, 0]);
        assert!(!d.nvs_unlocked);
    }

    #[test]
    fn nvs_status() {
        let mut f = FakeHid::default();
        f.features.insert(0x11, vec![0x11, 1, 0, 0]);
        let mut d = DualShock4Driver::new(f, info(Connection::Usb));
        assert!(d.nvs_is_locked().unwrap());
    }

    #[test]
    fn firmware_parse() {
        let mut b = vec![0u8; 49];
        b[0] = 0xA3;
        b[41] = 0x2C;
        assert!(parse_fw(&b).unwrap().contains("0x002c"));
        assert!(parse_fw(&[0xA3]).is_none());
    }
}
