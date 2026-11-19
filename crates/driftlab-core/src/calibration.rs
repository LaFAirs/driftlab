//! High-level calibration workflow helpers (driver-agnostic, UI-free).

use crate::math::{analyze_drift, estimate_center, DriftResult};
use crate::model::*;
use serde::{Deserialize, Serialize};

/// Collect `n` resting samples of both sticks.
pub type StickSamples = Vec<(f32, f32)>;

pub fn collect_rest(driver: &mut dyn ControllerDriver, n: usize) -> Result<(StickSamples, StickSamples)> {
    let (mut l, mut r) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let mut guard = 0;
    while l.len() < n {
        match driver.poll() {
            Ok(s) => {
                l.push(s.left);
                r.push(s.right);
            }
            Err(DriverError::Parse(_)) if guard < n * 4 => guard += 1, // timeouts / junk reports
            Err(e) => return Err(e),
        }
    }
    Ok((l, r))
}

/// Result of a two-stick drift test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftReport {
    pub left: DriftResult,
    pub right: DriftResult,
    pub samples: usize,
}

impl DriftReport {
    /// Worst score of both sticks.
    pub fn worst_score(&self) -> f32 {
        self.left.score.max(self.right.score)
    }
}

pub fn drift_test(driver: &mut dyn ControllerDriver, n: usize) -> Result<DriftReport> {
    let (l, r) = collect_rest(driver, n)?;
    Ok(DriftReport { left: analyze_drift(&l), right: analyze_drift(&r), samples: n })
}

/// Before/after comparison. Improvement is only claimed when it is measurable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub before: f32,
    pub after: f32,
    pub delta: f32,
    pub improved: bool,
}

/// Minimum score decrease that counts as a real improvement.
pub const MIN_IMPROVEMENT: f32 = 5.0;

pub fn compare(before: &DriftReport, after: &DriftReport) -> Comparison {
    let (b, a) = (before.worst_score(), after.worst_score());
    Comparison { before: b, after: a, delta: b - a, improved: b - a >= MIN_IMPROVEMENT }
}

/// Hardware centre calibration with stability check. Result is applied *temporarily*.
/// On any failure the driver is aborted so the device returns to a safe state.
pub fn hw_center_calibration(driver: &mut dyn ControllerDriver, rounds: u32, samples_per_round: usize) -> Result<()> {
    let (l, r) = collect_rest(driver, samples_per_round.max(10))?;
    let (cl, cr) = (estimate_center(&l), estimate_center(&r));
    if !cl.stable || !cr.stable {
        return Err(DriverError::Safety("sticks were moving – release both sticks and try again".into()));
    }
    let mut session = driver.begin_calibration(CalibKind::Center)?;
    for _ in 0..rounds.max(1) {
        if let Err(e) = driver.sample(&mut session) {
            let _ = driver.abort();
            return Err(e);
        }
    }
    driver.apply_temporary(session)
}

/// Hardware range calibration: begin → user sweeps (caller drives `sweep`) → end.
pub fn hw_range_calibration(
    driver: &mut dyn ControllerDriver,
    mut sweep: impl FnMut(&mut dyn ControllerDriver) -> Result<bool>,
) -> Result<()> {
    let session = driver.begin_calibration(CalibKind::Range)?;
    match sweep(driver) {
        Ok(true) => driver.apply_temporary(session),
        Ok(false) => {
            let _ = driver.abort();
            Err(DriverError::Safety("range sweep incomplete – aborted".into()))
        }
        Err(e) => {
            let _ = driver.abort();
            Err(e)
        }
    }
}
