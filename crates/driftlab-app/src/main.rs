#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
//! DriftLab – stick drift diagnosis and calibration.

#![allow(deprecated)] // egui 0.37 deprecates Id::new in favour of salts; ids here are local and stable.

mod app;
mod export;
mod fonts;
mod i18n;
mod logo;
mod screens;
mod shots;
mod theme;
mod view3d;
mod widgets;

use std::path::PathBuf;

/// Command-line options (kept deliberately tiny).
#[derive(Default, Clone)]
pub struct Args {
    pub demo: bool,
    pub force_2d: bool,
    pub screen: Option<String>,
    pub screenshots: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--demo" => a.demo = true,
            "--2d" => a.force_2d = true,
            "--screen" => a.screen = it.next(),
            "--screenshots" => a.screenshots = it.next().map(PathBuf::from),
            "--version" | "-V" => {
                println!("driftlab {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => {}
        }
    }
    a
}

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let dir = driftlab_core::storage::log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("driftlab")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir)
        .ok()?;
    let (nb, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,wgpu=warn,naga=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(nb).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();
    Some(guard)
}

fn main() -> eframe::Result {
    let _guard = init_logging();
    let args = parse_args();
    tracing::info!("DriftLab {} starting", env!("CARGO_PKG_VERSION"));
    let viewport = egui::ViewportBuilder::default()
        .with_title("DriftLab")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([1100.0, 720.0])
        .with_icon(std::sync::Arc::new(load_icon()));
    let options = eframe::NativeOptions {
        viewport,
        persist_window: true,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration::default(),
        ..Default::default()
    };
    eframe::run_native("DriftLab", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, &args)))))
}

fn load_icon() -> egui::IconData {
    // 64×64 icon rendered from the same geometry as the logo.
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
            let r = (fx * fx + fy * fy).sqrt();
            let dot = ((fx - 0.18).powi(2) + (fy + 0.12).powi(2)).sqrt();
            let o = (y * n + x) * 4;
            let (c, a): ([u8; 3], u8) = if r > 0.98 {
                ([0, 0, 0], 0)
            } else if dot < 0.30 {
                ([45, 107, 255], 255)
            } else if (0.78..0.94).contains(&r) || ((fx.abs() < 0.07 || fy.abs() < 0.07) && r > 0.45 && r < 0.98) {
                ([236, 240, 247], 255)
            } else {
                ([20, 24, 34], 255)
            };
            rgba[o..o + 3].copy_from_slice(&c);
            rgba[o + 3] = a;
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}
