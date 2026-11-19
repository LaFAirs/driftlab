//! Thin HID I/O abstraction so protocol code can be tested with scripted fakes.

use crate::model::{DriverError, Result};

pub trait HidIo: Send {
    /// Read an input report; returns bytes read (0 on timeout).
    fn read_timeout(&mut self, buf: &mut [u8], ms: i32) -> Result<usize>;
    fn send_feature(&mut self, data: &[u8]) -> Result<()>;
    /// `buf[0]` must hold the report id; returns bytes written to `buf`.
    fn get_feature(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn write(&mut self, data: &[u8]) -> Result<usize>;
}

impl HidIo for hidapi::HidDevice {
    fn read_timeout(&mut self, buf: &mut [u8], ms: i32) -> Result<usize> {
        hidapi::HidDevice::read_timeout(self, buf, ms).map_err(|e| DriverError::Hid(e.to_string()))
    }
    fn send_feature(&mut self, data: &[u8]) -> Result<()> {
        hidapi::HidDevice::send_feature_report(self, data).map_err(|e| DriverError::Hid(e.to_string()))
    }
    fn get_feature(&mut self, buf: &mut [u8]) -> Result<usize> {
        hidapi::HidDevice::get_feature_report(self, buf).map_err(|e| DriverError::Hid(e.to_string()))
    }
    fn write(&mut self, data: &[u8]) -> Result<usize> {
        hidapi::HidDevice::write(self, data).map_err(|e| DriverError::Hid(e.to_string()))
    }
}

/// Scripted fake used by unit tests.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct FakeHid {
        pub sent: Vec<Vec<u8>>,
        pub features: HashMap<u8, Vec<u8>>,
        pub inputs: Vec<Vec<u8>>,
        /// If set, `send_feature` fails for payloads starting with these bytes.
        pub fail_on: Option<Vec<u8>>,
    }

    impl HidIo for FakeHid {
        fn read_timeout(&mut self, buf: &mut [u8], _ms: i32) -> Result<usize> {
            if self.inputs.is_empty() {
                return Ok(0);
            }
            let r = if self.inputs.len() > 1 { self.inputs.remove(0) } else { self.inputs[0].clone() };
            let n = r.len().min(buf.len());
            buf[..n].copy_from_slice(&r[..n]);
            Ok(n)
        }
        fn send_feature(&mut self, data: &[u8]) -> Result<()> {
            if let Some(f) = &self.fail_on {
                if data.starts_with(f) {
                    return Err(DriverError::Hid("scripted failure".into()));
                }
            }
            self.sent.push(data.to_vec());
            Ok(())
        }
        fn get_feature(&mut self, buf: &mut [u8]) -> Result<usize> {
            let id = buf[0];
            let r = self.features.get(&id).cloned().unwrap_or_else(|| vec![id]);
            let n = r.len().min(buf.len());
            buf[..n].copy_from_slice(&r[..n]);
            Ok(n)
        }
        fn write(&mut self, data: &[u8]) -> Result<usize> {
            self.sent.push(data.to_vec());
            Ok(data.len())
        }
    }
}
