//! DriftLab core: device detection, HID protocols, calibration mathematics, profiles. No UI dependencies.

pub mod calibration;
pub mod db;
pub mod gamepad;
pub mod hidio;
pub mod manager;
pub mod math;
pub mod mock;
pub mod model;
pub mod ps4;
pub mod ps5;
pub mod safety;
pub mod storage;

pub use model::*;
