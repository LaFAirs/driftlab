//! Simulated controller with adjustable drift, for tests and demo mode.

use crate::model::*;
use parking_lot::Mutex;
use std::sync::Arc;

/// Externally adjustable simulation parameters.
#[derive(Debug, Clone)]
pub struct MockControl {
    /// Physical stick positions the "user" holds (before drift).
    pub left: (f32, f32),
    pub right: (f32, f32),
    pub drift_left: (f32, f32),
    pub drift_right: (f32, f32),
    pub noise: f32,
    pub battery: u8,
    pub l2: f32,
    pub r2: f32,
    pub pressed: Vec<String>,
}

impl Default for MockControl {
    fn default() -> Self {
        Self {
            left: (0.0, 0.0),
            right: (0.0, 0.0),
            drift_left: (0.06, -0.04),
            drift_right: (0.02, 0.03),
            noise: 0.002,
            battery: 82,
            l2: 0.0,
            r2: 0.0,
            pressed: vec![],
        }
    }
}

pub struct MockDriver {
    pub control: Arc<Mutex<MockControl>>,
    family: Family,
    connection: Connection,
    /// Hardware centre correction (what "calibration" changes), left/right.
    hw_center: [(f32, f32); 2],
    saved_center: [(f32, f32); 2],
    pending: Option<[(f32, f32); 2]>,
    seed: u32,
    pub fail_commit: bool,
    uid: u32,
}

impl MockDriver {
    pub fn new(family: Family) -> Self {
        Self::with_control(family, Arc::new(Mutex::new(MockControl::default())))
    }

    pub fn with_control(family: Family, control: Arc<Mutex<MockControl>>) -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let uid = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            control,
            family,
            connection: Connection::Usb,
            hw_center: [(0.0, 0.0); 2],
            saved_center: [(0.0, 0.0); 2],
            pending: None,
            seed: 0x1234_5678,
            fail_commit: false,
            uid,
        }
    }

    pub fn set_connection(&mut self, c: Connection) {
        self.connection = c;
    }

    fn rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((self.seed >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
    }
}

impl ControllerDriver for MockDriver {
    fn info(&self) -> ControllerInfo {
        ControllerInfo {
            id: format!("mock-{}", self.uid),
            name: "Demo Controller".into(),
            family: self.family,
            vid: 0,
            pid: 0,
            connection: self.connection,
            firmware: Some("demo 1.0".into()),
            serial: Some("DE:MO:00:00:3F:1C".into()),
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            hw_calibration: true,
            triggers: true,
            gyro: true,
            touchpad: matches!(self.family, Family::DualSense | Family::DualSenseEdge | Family::DualShock4),
            rumble: true,
            lightbar: matches!(self.family, Family::DualSense | Family::DualSenseEdge | Family::DualShock4),
            battery: true,
        }
    }

    fn poll(&mut self) -> Result<InputState> {
        std::thread::sleep(std::time::Duration::from_millis(4));
        let c = self.control.lock().clone();
        let n = c.noise;
        let (n1, n2, n3, n4) = (self.rand() * n, self.rand() * n, self.rand() * n, self.rand() * n);
        let raw_l = ((c.left.0 + c.drift_left.0 + n1).clamp(-1.0, 1.0), (c.left.1 + c.drift_left.1 + n2).clamp(-1.0, 1.0));
        let raw_r = ((c.right.0 + c.drift_right.0 + n3).clamp(-1.0, 1.0), (c.right.1 + c.drift_right.1 + n4).clamp(-1.0, 1.0));
        let l = ((raw_l.0 - self.hw_center[0].0).clamp(-1.0, 1.0), (raw_l.1 - self.hw_center[0].1).clamp(-1.0, 1.0));
        let r = ((raw_r.0 - self.hw_center[1].0).clamp(-1.0, 1.0), (raw_r.1 - self.hw_center[1].1).clamp(-1.0, 1.0));
        Ok(InputState {
            left: l,
            right: r,
            raw_left: raw_l,
            raw_right: raw_r,
            l2: c.l2,
            r2: c.r2,
            buttons: c.pressed.iter().map(|b| (b.clone(), true)).collect(),
            gyro: Some([0.0; 3]),
            accel: Some([0.0, 0.0, 1.0]),
            touch: vec![],
            battery: Some(Battery { percent: c.battery, state: ChargeState::Charging }),
            raw_report: vec![],
        })
    }

    fn read_calibration(&mut self) -> Result<Option<CalibrationBlob>> {
        let mut data = Vec::new();
        for (x, y) in self.saved_center {
            data.extend_from_slice(&x.to_le_bytes());
            data.extend_from_slice(&y.to_le_bytes());
        }
        Ok(Some(CalibrationBlob { family: self.family, reports: vec![(0xFE, data)] }))
    }

    fn begin_calibration(&mut self, kind: CalibKind) -> Result<CalibSession> {
        Ok(CalibSession { kind, samples: 0 })
    }

    fn sample(&mut self, s: &mut CalibSession) -> Result<()> {
        s.samples += 1;
        if s.kind == CalibKind::Center {
            let c = self.control.lock();
            self.pending = Some([c.drift_left, c.drift_right]);
        }
        Ok(())
    }

    fn apply_temporary(&mut self, s: CalibSession) -> Result<()> {
        if s.kind == CalibKind::Center {
            if let Some(p) = self.pending.take() {
                self.hw_center = p;
            }
        }
        Ok(())
    }

    fn commit_permanent(&mut self) -> Result<()> {
        if self.fail_commit {
            return Err(DriverError::Protocol("simulated flash failure".into()));
        }
        self.saved_center = self.hw_center;
        Ok(())
    }

    fn abort(&mut self) -> Result<()> {
        self.pending = None;
        self.hw_center = self.saved_center;
        Ok(())
    }

    fn restore(&mut self, blob: &CalibrationBlob) -> Result<()> {
        let data = &blob.reports.first().ok_or_else(|| DriverError::Parse("empty backup".into()))?.1;
        if data.len() != 16 {
            return Err(DriverError::Parse("bad mock backup".into()));
        }
        let f = |i: usize| f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        self.hw_center = [(f(0), f(4)), (f(8), f(12))];
        self.saved_center = self.hw_center;
        Ok(())
    }

    fn set_output(&mut self, _o: OutputRequest) -> Result<()> {
        Ok(())
    }
}
