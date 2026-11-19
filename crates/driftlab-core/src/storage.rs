//! Persistent data: backups, profiles, settings.

use crate::math::StickCorrection;
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Default per-user data directory.
pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("dev", "driftlab", "DriftLab")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("driftlab-data"))
}

pub fn config_dir() -> PathBuf {
    directories::ProjectDirs::from("dev", "driftlab", "DriftLab")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("driftlab-config"))
}

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backup {
    pub timestamp: String,
    pub serial: String,
    pub name: String,
    pub firmware: Option<String>,
    pub blob: CalibrationBlob,
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

/// ISO-8601 UTC timestamp from the system clock (no external time crate needed).
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    iso_from_unix(secs)
}

pub fn iso_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

pub fn create_backup(dir: &Path, info: &ControllerInfo, blob: CalibrationBlob) -> Result<PathBuf> {
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups)?;
    let ts = now_iso();
    let b = Backup {
        timestamp: ts.clone(),
        serial: info.serial.clone().unwrap_or_else(|| "unknown".into()),
        name: info.name.clone(),
        firmware: info.firmware.clone(),
        blob,
    };
    let mut path = backups.join(format!("{}_{}.json", sanitize(&ts), sanitize(&b.serial)));
    let mut n = 1;
    while path.exists() {
        path = backups.join(format!("{}_{}_{n}.json", sanitize(&ts), sanitize(&b.serial)));
        n += 1;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&b)?)?;
    Ok(path)
}

pub fn list_backups(dir: &Path) -> Vec<(PathBuf, Backup)> {
    let mut v: Vec<(PathBuf, Backup)> = std::fs::read_dir(dir.join("backups"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let b: Backup = serde_json::from_slice(&std::fs::read(&p).ok()?).ok()?;
            Some((p, b))
        })
        .collect();
    v.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
    v
}

pub fn delete_backup(path: &Path) -> Result<()> {
    Ok(std::fs::remove_file(path)?)
}

/// Write through a temporary file and rename, so a crash or full disk never leaves a half-written file behind.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Move an unreadable file aside (`<name>.corrupt`) so it is neither lost nor overwritten by the defaults.
fn quarantine(path: &Path) {
    let mut target = path.as_os_str().to_owned();
    target.push(".corrupt");
    let _ = std::fs::rename(path, std::path::PathBuf::from(target));
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub left: StickCorrection,
    pub right: StickCorrection,
    pub system_wide: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self { name: "Default".into(), left: StickCorrection::default(), right: StickCorrection::default(), system_wide: false }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProfileStore {
    /// serial → profiles
    pub by_serial: std::collections::BTreeMap<String, Vec<Profile>>,
}

impl ProfileStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("profiles.json");
        match std::fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| {
                quarantine(&path);
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        atomic_write(&dir.join("profiles.json"), &serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

pub fn export_profile(p: &Profile, path: &Path) -> Result<()> {
    Ok(std::fs::write(path, serde_json::to_vec_pretty(p)?)?)
}

pub fn import_profile(path: &Path) -> Result<Profile> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// Steam-Input-friendly summary of a profile (honest fallback when no virtual driver is available).
pub fn steam_export_text(p: &Profile) -> String {
    let f = |n: &str, c: &StickCorrection| {
        format!(
            "{n}: deadzone type={:?}, inner={:.1}%, outer={:.1}%, anti-deadzone={:.1}%, centre offset=({:+.3}, {:+.3})\n",
            c.kind,
            c.inner_deadzone * 100.0,
            c.outer_deadzone * 100.0,
            c.anti_deadzone * 100.0,
            c.center_x,
            c.center_y
        )
    };
    format!(
        "DriftLab profile \"{}\"\n{}{}\nSteam Input: Controller Settings → Edit Layout → Joystick → set 'Inner Deadzone' / 'Outer Deadzone' (Deadzone) to the values above.\n",
        p.name,
        f("Left stick", &p.left),
        f("Right stick", &p.right)
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: String,
    pub theme: String,
    pub reduced_motion: bool,
    pub polling_hz: u32,
    pub expert_mode: bool,
    pub ui_scale: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { language: "auto".into(), theme: "auto".into(), reduced_motion: false, polling_hz: 250, expert_mode: false, ui_scale: 1.0 }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.toml");
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|_| {
                quarantine(&path);
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let s = toml::to_string_pretty(self).map_err(|e| DriverError::Protocol(e.to_string()))?;
        atomic_write(&dir.join("settings.toml"), s.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("driftlab-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn info() -> ControllerInfo {
        ControllerInfo {
            id: "a".into(),
            name: "Pad".into(),
            family: Family::DualSense,
            vid: 1,
            pid: 2,
            connection: Connection::Usb,
            firmware: Some("1.0".into()),
            serial: Some("A4:11:22:33:3F:1C".into()),
        }
    }

    #[test]
    fn iso_known_dates() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn backup_roundtrip_and_delete() {
        let d = tmp("backup");
        let blob = CalibrationBlob { family: Family::DualSense, reports: vec![(12, vec![1, 2, 3])] };
        let p = create_backup(&d, &info(), blob.clone()).unwrap();
        let p2 = create_backup(&d, &info(), blob.clone()).unwrap();
        assert_ne!(p, p2);
        let l = list_backups(&d);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].1.blob, blob);
        assert_eq!(l[0].1.serial, "A4:11:22:33:3F:1C");
        delete_backup(&p).unwrap();
        assert_eq!(list_backups(&d).len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn profile_import_export_and_store() {
        let d = tmp("profile");
        std::fs::create_dir_all(&d).unwrap();
        let mut p = Profile::default();
        p.left.inner_deadzone = 0.12;
        let f = d.join("x.driftlab.json");
        export_profile(&p, &f).unwrap();
        assert_eq!(import_profile(&f).unwrap(), p);
        let mut s = ProfileStore::default();
        s.by_serial.insert("S".into(), vec![p.clone()]);
        s.save(&d).unwrap();
        assert_eq!(ProfileStore::load(&d), s);
        assert!(steam_export_text(&p).contains("12.0%"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn settings_default_on_missing_and_roundtrip() {
        let d = tmp("settings");
        assert_eq!(Settings::load(&d), Settings::default());
        let s = Settings { language: "de".into(), expert_mode: true, ..Default::default() };
        s.save(&d).unwrap();
        assert_eq!(Settings::load(&d), s);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn corrupt_profiles_and_settings_are_quarantined_not_overwritten() {
        let d = tmp("corrupt");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("profiles.json"), b"{ not json").unwrap();
        std::fs::write(d.join("settings.toml"), b"= = =").unwrap();
        assert_eq!(ProfileStore::load(&d), ProfileStore::default());
        assert_eq!(Settings::load(&d), Settings::default());
        assert!(d.join("profiles.json.corrupt").exists());
        assert!(d.join("settings.toml.corrupt").exists());
        ProfileStore::default().save(&d).unwrap();
        assert!(d.join("profiles.json.corrupt").exists(), "quarantined copy must survive the next save");
    }

    #[test]
    fn save_into_unwritable_location_reports_error() {
        let d = tmp("unwritable");
        std::fs::create_dir_all(&d).unwrap();
        let blocker = d.join("file");
        std::fs::write(&blocker, b"x").unwrap();
        // a *file* where the data directory should be: create_dir_all must fail, no panic
        assert!(ProfileStore::default().save(&blocker).is_err());
        assert!(Settings::default().save(&blocker).is_err());
    }
}
