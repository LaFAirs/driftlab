//! CSV / JSON / report exports into `Documents/DriftLab`.

use driftlab_core::calibration::DriftReport;
use driftlab_core::storage::now_iso;
use driftlab_core::{ControllerInfo, DriverError};
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    let base = directories::UserDirs::new().and_then(|d| d.document_dir().map(|p| p.to_path_buf())).unwrap_or_else(std::env::temp_dir);
    base.join("DriftLab")
}

fn stamp() -> String {
    now_iso().replace([':', '-'], "")
}

pub fn write_csv(rep: &DriftReport, series: &[[f32; 4]]) -> Result<PathBuf, DriverError> {
    std::fs::create_dir_all(dir())?;
    let p = dir().join(format!("driftlab-{}.csv", stamp()));
    let mut s = String::from("index,left_x,left_y,right_x,right_y\n");
    for (i, v) in series.iter().enumerate() {
        s.push_str(&format!("{i},{:.5},{:.5},{:.5},{:.5}\n", v[0], v[1], v[2], v[3]));
    }
    s.push_str(&format!("# left score {:.1}, right score {:.1}\n", rep.left.score, rep.right.score));
    std::fs::write(&p, s)?;
    Ok(p)
}

pub fn write_json(rep: &DriftReport, info: Option<&ControllerInfo>) -> Result<PathBuf, DriverError> {
    std::fs::create_dir_all(dir())?;
    let p = dir().join(format!("driftlab-{}.json", stamp()));
    let v = serde_json::json!({
        "generated": now_iso(),
        "controller": info.map(|i| serde_json::json!({"name": i.name, "vid": i.vid, "pid": i.pid, "firmware": i.firmware, "serial": i.masked_serial()})),
        "report": rep,
    });
    std::fs::write(&p, serde_json::to_vec_pretty(&v)?)?;
    Ok(p)
}

pub fn report_png_path() -> PathBuf {
    dir().join(format!("driftlab-report-{}.png", stamp()))
}

/// Save an RGBA8 image as PNG.
pub fn save_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    image::save_buffer(path, rgba, w, h, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())
}

/// Open a folder/file with the platform's default handler.
pub fn open_path(p: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

pub fn open_url(u: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", u]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(u).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(u).spawn();
}
