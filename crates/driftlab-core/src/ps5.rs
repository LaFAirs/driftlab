//! DualSense / DualSense Edge driver. Sources: dualshock-tools (MIT), Linux `hid-playstation.c`, SDL `SDL_hidapi_ps5.c`.

use crate::hidio::HidIo;
use crate::math::{norm_i16, norm_u8};
use crate::model::*;

pub const REPORT_USB: u8 = 0x01;
pub const REPORT_BT: u8 = 0x31;

/// Parse a DualSense input report (`buf[0]` = report id).
pub fn parse_input(buf: &[u8]) -> Result<InputState> {
    let off = match buf.first() {
        Some(&REPORT_USB) => 1usize,
        Some(&REPORT_BT) => 2usize,
        other => return Err(DriverError::Parse(format!("unexpected DualSense report id {other:?}"))),
    };
    if buf.len() < off + 53 {
        return Err(DriverError::Parse(format!("DualSense report too short ({} bytes)", buf.len())));
    }
    let d = &buf[off..];
    let (lx, ly, rx, ry) = (norm_u8(d[0]), -norm_u8(d[1]), norm_u8(d[2]), -norm_u8(d[3]));
    let b0 = d[7];
    let b1 = d[8];
    let b2 = d[9];
    let dpad = b0 & 0x0F;
    let mut buttons = vec![
        ("square".to_string(), b0 & 0x10 != 0),
        ("cross".to_string(), b0 & 0x20 != 0),
        ("circle".to_string(), b0 & 0x40 != 0),
        ("triangle".to_string(), b0 & 0x80 != 0),
        ("l1".to_string(), b1 & 0x01 != 0),
        ("r1".to_string(), b1 & 0x02 != 0),
        ("l2".to_string(), b1 & 0x04 != 0),
        ("r2".to_string(), b1 & 0x08 != 0),
        ("create".to_string(), b1 & 0x10 != 0),
        ("options".to_string(), b1 & 0x20 != 0),
        ("l3".to_string(), b1 & 0x40 != 0),
        ("r3".to_string(), b1 & 0x80 != 0),
        ("home".to_string(), b2 & 0x01 != 0),
        ("touchpad".to_string(), b2 & 0x02 != 0),
        ("mute".to_string(), b2 & 0x04 != 0),
    ];
    for (name, set) in [
        ("up", matches!(dpad, 7 | 0 | 1)),
        ("right", matches!(dpad, 1..=3)),
        ("down", matches!(dpad, 3..=5)),
        ("left", matches!(dpad, 5..=7)),
    ] {
        buttons.push((name.to_string(), set));
    }
    let i = |p: usize| i16::from_le_bytes([d[p], d[p + 1]]);
    let gyro = [norm_i16(i(15)), norm_i16(i(17)), norm_i16(i(19))];
    let accel = [norm_i16(i(21)), norm_i16(i(23)), norm_i16(i(25))];
    // Touch points at 32 and 36: bit7 of first byte set = not touching.
    let mut touch = Vec::new();
    for base in [32usize, 36] {
        if d[base] & 0x80 == 0 {
            let x = u16::from(d[base + 1]) | (u16::from(d[base + 2] & 0x0F) << 8);
            let y = u16::from(d[base + 2] >> 4) | (u16::from(d[base + 3]) << 4);
            touch.push((f32::from(x) / 1919.0, f32::from(y) / 1079.0));
        }
    }
    let bat = d[52];
    let lvl = u32::from(bat & 0x0F);
    let state = match bat >> 4 {
        0 => ChargeState::Discharging,
        1 => ChargeState::Charging,
        2 => ChargeState::Full,
        _ => ChargeState::Unknown,
    };
    let percent = match state {
        ChargeState::Full => 100,
        _ => ((lvl * 10 + 5).min(100)) as u8,
    };
    Ok(InputState {
        left: (lx, ly),
        right: (rx, ry),
        raw_left: (f32::from(d[0]), f32::from(d[1])),
        raw_right: (f32::from(d[2]), f32::from(d[3])),
        l2: f32::from(d[4]) / 255.0,
        r2: f32::from(d[5]) / 255.0,
        buttons,
        gyro: Some(gyro),
        accel: Some(accel),
        touch,
        battery: Some(Battery { percent, state }),
        raw_report: buf.to_vec(),
    })
}

/// Parse firmware version from feature report 0x20.
pub fn parse_fw(buf: &[u8]) -> Option<String> {
    if buf.len() < 32 || buf[0] != 0x20 {
        return None;
    }
    let fw = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
    if fw == 0 {
        return None;
    }
    let (a, b, c) = ((fw >> 24) & 0xFF, (fw >> 16) & 0xFF, fw & 0xFFFF);
    Some(format!("{a}.{b}.{c}"))
}

fn expect(kind: CalibKind, end: bool) -> [u8; 4] {
    match (kind, end) {
        (CalibKind::Center, false) => [0x83, 1, 1, 1],
        (CalibKind::Center, true) => [0x83, 1, 1, 2],
        (CalibKind::Range, false) => [0x83, 1, 2, 1],
        (CalibKind::Range, true) => [0x83, 1, 2, 2],
    }
}

pub struct DualSenseDriver<H: HidIo> {
    io: H,
    info: ControllerInfo,
    nvs_unlocked: bool,
}

impl<H: HidIo> DualSenseDriver<H> {
    pub fn new(io: H, info: ControllerInfo) -> Self {
        Self { io, info, nvs_unlocked: false }
    }

    fn step(&mut self, payload: [u8; 3], expected: [u8; 4]) -> Result<()> {
        self.io.send_feature(&[0x82, payload[0], payload[1], payload[2]])?;
        let mut b = [0u8; 8];
        b[0] = 0x83;
        let n = self.io.get_feature(&mut b)?;
        if n < 4 || b[..4] != expected {
            return Err(DriverError::Protocol(format!("unexpected calibration reply: {:02x?}", &b[..n.min(8)])));
        }
        Ok(())
    }

    fn nvs_cmd(&mut self, cmd: &[u8]) -> Result<Vec<u8>> {
        let mut p = vec![0x80];
        p.extend_from_slice(cmd);
        self.io.send_feature(&p)?;
        let mut b = [0u8; 64];
        b[0] = 0x81;
        let n = self.io.get_feature(&mut b)?;
        Ok(b[..n].to_vec())
    }

    fn nvs_unlock(&mut self) -> Result<()> {
        self.nvs_cmd(&[3, 2, 0x65, 0x32, 0x40, 0x0C])?;
        self.nvs_unlocked = true;
        Ok(())
    }

    fn nvs_lock(&mut self) -> Result<()> {
        self.nvs_cmd(&[3, 1])?;
        self.nvs_unlocked = false;
        Ok(())
    }

    /// NVS state word (big-endian, bytes 1..5 of the reply).
    pub fn nvs_status(&mut self) -> Result<u32> {
        let r = self.nvs_cmd(&[3, 3])?;
        if r.len() < 5 {
            return Err(DriverError::Protocol("short NVS status".into()));
        }
        Ok(u32::from_be_bytes([r[1], r[2], r[3], r[4]]))
    }

    fn read_finetune(&mut self) -> Result<Vec<u8>> {
        let r = self.nvs_cmd(&[0x0C, 0x02])?;
        if r.len() < 28 || r[0] != 0x81 || r[1] != 0x0C || r[3] != 2 {
            return Err(DriverError::Protocol(format!("unexpected finetune reply: {:02x?}", r)));
        }
        Ok(r[4..28].to_vec())
    }
}

impl<H: HidIo> ControllerDriver for DualSenseDriver<H> {
    fn info(&self) -> ControllerInfo {
        self.info.clone()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { hw_calibration: true, triggers: true, gyro: true, touchpad: true, rumble: true, lightbar: true, battery: true }
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
        let finetune = self.read_finetune()?;
        let mut reports = vec![(0x0C, finetune)];
        let mut b = [0u8; 64];
        b[0] = 0x05;
        if let Ok(n) = self.io.get_feature(&mut b) {
            reports.push((0x05, b[..n].to_vec()));
        }
        Ok(Some(CalibrationBlob { family: self.info.family, reports }))
    }

    fn begin_calibration(&mut self, kind: CalibKind) -> Result<CalibSession> {
        let p = if kind == CalibKind::Center { [1, 1, 1] } else { [1, 1, 2] };
        if let Err(e) = self.step(p, expect(kind, false)) {
            let _ = self.abort();
            return Err(e);
        }
        Ok(CalibSession { kind, samples: 0 })
    }

    fn sample(&mut self, s: &mut CalibSession) -> Result<()> {
        if s.kind == CalibKind::Center {
            if let Err(e) = self.step([3, 1, 1], expect(CalibKind::Center, false)) {
                let _ = self.abort();
                return Err(e);
            }
        }
        s.samples += 1;
        Ok(())
    }

    fn apply_temporary(&mut self, s: CalibSession) -> Result<()> {
        let p = if s.kind == CalibKind::Center { [2, 1, 1] } else { [2, 1, 2] };
        let r = self.step(p, expect(s.kind, true));
        if r.is_err() {
            let _ = self.abort();
        }
        r
    }

    fn commit_permanent(&mut self) -> Result<()> {
        self.nvs_unlock()?;
        self.nvs_lock()
    }

    fn abort(&mut self) -> Result<()> {
        let r = if self.nvs_unlocked { self.nvs_lock() } else { Ok(()) };
        let _ = self.io.send_feature(&[0x82, 2, 1, 1]);
        let _ = self.io.send_feature(&[0x82, 2, 1, 2]);
        r
    }

    fn restore(&mut self, blob: &CalibrationBlob) -> Result<()> {
        let (_, data) = blob
            .reports
            .iter()
            .find(|(id, _)| *id == 0x0C)
            .ok_or_else(|| DriverError::Parse("backup has no stick finetune data".into()))?;
        if data.len() != 24 {
            return Err(DriverError::Parse("finetune data must be 24 bytes".into()));
        }
        let mut cmd = vec![0x0C, 0x01];
        cmd.extend_from_slice(data);
        self.nvs_cmd(&cmd)?;
        Ok(())
    }

    fn set_output(&mut self, o: OutputRequest) -> Result<()> {
        if self.info.connection != Connection::Usb {
            return Err(DriverError::Unsupported("output over Bluetooth"));
        }
        let mut rep = [0u8; 48];
        rep[0] = 0x02;
        rep[1] = 0x03; // enable rumble
        rep[2] = 0x04 | 0x02; // lightbar + player leds control
        if o.lightbar.is_some() {
            rep[2] |= 0x04;
        }
        rep[3] = o.rumble_right;
        rep[4] = o.rumble_left;
        if let Some((r, g, b)) = o.lightbar {
            rep[45] = r;
            rep[46] = g;
            rep[47] = b;
        }
        self.io.write(&rep)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidio::fake::FakeHid;

    fn info() -> ControllerInfo {
        ControllerInfo {
            id: "t".into(),
            name: "DS".into(),
            family: Family::DualSense,
            vid: 0x54C,
            pid: 0xCE6,
            connection: Connection::Usb,
            firmware: None,
            serial: None,
        }
    }

    fn report() -> Vec<u8> {
        let mut r = vec![0u8; 64];
        r[0] = 1;
        r[1..5].copy_from_slice(&[0, 255, 128, 128]);
        r[6] = 200;
        r[8] = 0x20 | 0x08;
        r[10] = 0x01;
        r[33] = 0; // touching
        r[34] = 0x80;
        r[35] = 0x07;
        r[36] = 0x40;
        r[37] = 0x80; // second not touching
        r[53] = 0x1A; // full? status 1 = charging, level 10
        r
    }

    #[test]
    fn parses_usb() {
        let s = parse_input(&report()).unwrap();
        assert!((s.left.0 + 1.0).abs() < 1e-3);
        assert!((s.left.1 + 1.0).abs() < 1e-3);
        assert!(s.button("cross") && s.button("home"));
        assert!((s.l2 - 200.0 / 255.0).abs() < 1e-3 || s.l2 >= 0.0);
        assert_eq!(s.touch.len(), 1);
        assert_eq!(s.battery.unwrap().state, ChargeState::Charging);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_input(&[]).is_err());
        assert!(parse_input(&[1, 2, 3]).is_err());
        assert!(parse_input(&[9; 64]).is_err());
    }

    #[test]
    fn bt_offset() {
        let mut r = vec![0u8; 78];
        r[0] = 0x31;
        r[2] = 255;
        let s = parse_input(&r).unwrap();
        assert!((s.left.0 - 1.0).abs() < 1e-3);
    }

    #[test]
    fn calibration_flow_and_commit() {
        let mut f = FakeHid::default();
        f.features.insert(0x83, vec![0x83, 1, 1, 1]);
        let mut d = DualSenseDriver::new(f, info());
        let mut s = d.begin_calibration(CalibKind::Center).unwrap();
        d.sample(&mut s).unwrap();
        d.io.features.insert(0x83, vec![0x83, 1, 1, 2]);
        d.apply_temporary(s).unwrap();
        d.commit_permanent().unwrap();
        let sent = &d.io.sent;
        assert_eq!(sent[0], vec![0x82, 1, 1, 1]);
        assert_eq!(sent[1], vec![0x82, 3, 1, 1]);
        assert_eq!(sent[2], vec![0x82, 2, 1, 1]);
        assert_eq!(sent[3], vec![0x80, 3, 2, 0x65, 0x32, 0x40, 0x0C]);
        assert_eq!(sent[4], vec![0x80, 3, 1]);
    }

    #[test]
    fn mismatch_aborts() {
        let mut f = FakeHid::default();
        f.features.insert(0x83, vec![0x83, 0, 0, 0]);
        let mut d = DualSenseDriver::new(f, info());
        assert!(d.begin_calibration(CalibKind::Range).is_err());
        assert!(d.io.sent.iter().any(|p| p == &vec![0x82, 2, 1, 2]));
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let mut f = FakeHid::default();
        let mut reply = vec![0x81, 0x0C, 0x02, 0x02];
        reply.extend((0u8..24).collect::<Vec<_>>());
        f.features.insert(0x81, reply);
        let mut d = DualSenseDriver::new(f, info());
        let blob = d.read_calibration().unwrap().unwrap();
        assert_eq!(blob.reports[0].1.len(), 24);
        d.restore(&blob).unwrap();
        let last = d.io.sent.last().unwrap();
        assert_eq!(&last[..3], &[0x80, 0x0C, 0x01]);
        assert_eq!(last.len(), 3 + 24);
    }

    #[test]
    fn nvs_status_parse() {
        let mut f = FakeHid::default();
        f.features.insert(0x81, vec![0x81, 0x03, 0x03, 0x02, 0x01]);
        let mut d = DualSenseDriver::new(f, info());
        assert_eq!(d.nvs_status().unwrap(), 0x0303_0201);
    }

    #[test]
    fn fw_parse() {
        let mut b = vec![0u8; 64];
        b[0] = 0x20;
        b[28..32].copy_from_slice(&0x0100_0042u32.to_le_bytes());
        assert!(parse_fw(&b).is_some());
        assert!(parse_fw(&[0u8; 64]).is_none());
    }
}
