//! Full wizard flow against the MockDriver (no hardware).

use driftlab_core::calibration::*;
use driftlab_core::mock::MockDriver;
use driftlab_core::safety::preflight;
use driftlab_core::storage::{create_backup, list_backups};
use driftlab_core::*;

#[test]
fn full_wizard_flow_improves_drift_and_commits() {
    let mut d = MockDriver::new(Family::DualSense);
    let info = d.info();
    let pf = preflight(&info, &d.capabilities(), Some(Battery { percent: 82, state: ChargeState::Charging }), false);
    assert!(!pf.blocked());

    // Backup
    let dir = std::env::temp_dir().join(format!("driftlab-wiz-{}", std::process::id()));
    let blob = d.read_calibration().unwrap().unwrap();
    create_backup(&dir, &info, blob.clone()).unwrap();
    assert_eq!(list_backups(&dir).len(), 1);

    let before = drift_test(&mut d, 200).unwrap();
    assert!(before.worst_score() > 12.0);

    hw_center_calibration(&mut d, 3, 100).unwrap();
    let after = drift_test(&mut d, 200).unwrap();
    let cmp = compare(&before, &after);
    assert!(cmp.improved, "{cmp:?}");

    d.commit_permanent().unwrap();
    // Restore backup brings drift back.
    d.restore(&blob).unwrap();
    let restored = drift_test(&mut d, 200).unwrap();
    assert!(restored.worst_score() > after.worst_score());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn moving_sticks_abort_center_calibration() {
    let mut d = MockDriver::new(Family::DualSense);
    // Sweep the stick while sampling by making noise huge.
    d.control.lock().noise = 0.3;
    assert!(matches!(hw_center_calibration(&mut d, 3, 100), Err(DriverError::Safety(_))));
}

#[test]
fn failed_commit_leaves_safe_state() {
    let mut d = MockDriver::new(Family::DualShock4);
    hw_center_calibration(&mut d, 2, 50).unwrap();
    d.fail_commit = true;
    assert!(d.commit_permanent().is_err());
    d.abort().unwrap();
    let r = drift_test(&mut d, 100).unwrap();
    assert!(r.worst_score() > 12.0, "abort reverts the temporary calibration");
}

#[test]
fn comparison_never_claims_false_improvement() {
    let mut d = MockDriver::new(Family::Xbox);
    let a = drift_test(&mut d, 100).unwrap();
    let b = drift_test(&mut d, 100).unwrap();
    assert!(!compare(&a, &b).improved);
}
