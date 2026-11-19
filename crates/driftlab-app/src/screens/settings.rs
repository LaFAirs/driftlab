//! Settings, expert-mode HID inspector, update check, about / credits / disclaimer.

use crate::app::App;
use crate::export;
use crate::widgets::{self, card, ghost, h1, h2, primary};
use driftlab_core::{storage, Family};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Ui};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Default)]
pub struct SettingsUi {
    pub update: Arc<Mutex<Option<Result<String, String>>>>,
    pub checking: bool,
}

const REPO: &str = env!("CARGO_PKG_REPOSITORY");

fn repo_slug() -> String {
    REPO.trim_end_matches('/').rsplit('/').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/")
}

/// Compare dotted versions numerically (`v` prefix tolerated).
pub fn is_newer(latest: &str, current: &str) -> bool {
    let p = |s: &str| {
        s.trim_start_matches('v')
            .split('.')
            .map(|x| x.split(['-', '+']).next().unwrap_or("0").parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    p(latest) > p(current)
}

fn check_updates(slot: Arc<Mutex<Option<Result<String, String>>>>) {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo_slug());
    std::thread::spawn(move || {
        let res = (|| -> Result<String, String> {
            let mut resp = ureq::get(&url)
                .header("User-Agent", "driftlab-update-check")
                .header("Accept", "application/vnd.github+json")
                .call()
                .map_err(|e| e.to_string())?;
            let v: serde_json::Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
            v["tag_name"].as_str().map(str::to_string).ok_or_else(|| "no release found".to_string())
        })();
        *slot.lock() = Some(res);
    });
}

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    h1(ui, app.t("set.title"));
    ui.add_space(8.0);
    let mut changed = false;
    card(ui, &t, |ui| {
        h2(ui, app.t("set.general"));
        egui::Grid::new("setgrid").num_columns(2).spacing([24.0, 10.0]).show(ui, |ui| {
            ui.label(app.t("set.language"));
            egui::ComboBox::from_id_salt("lang")
                .selected_text(match app.settings.language.as_str() {
                    "de" => "Deutsch",
                    "en" => "English",
                    _ => app.t("set.auto"),
                })
                .show_ui(ui, |ui| {
                    for (v, l) in [("auto", app.t("set.auto")), ("en", "English"), ("de", "Deutsch")] {
                        changed |= ui.selectable_value(&mut app.settings.language, v.to_string(), l).changed();
                    }
                });
            ui.end_row();
            ui.label(app.t("set.theme"));
            egui::ComboBox::from_id_salt("theme")
                .selected_text(if app.settings.theme == "auto" { app.t("set.auto").to_string() } else { app.settings.theme.clone() })
                .show_ui(ui, |ui| {
                    for v in ["auto", "ps5", "ps5_edge", "ps4", "xbox", "neutral"] {
                        let label = if v == "auto" { app.t("set.auto") } else { v };
                        changed |= ui.selectable_value(&mut app.settings.theme, v.to_string(), label).changed();
                    }
                });
            ui.end_row();
            ui.label(app.t("set.reduced"));
            changed |= ui.checkbox(&mut app.settings.reduced_motion, "").changed();
            ui.end_row();
            ui.label(app.t("set.scale"));
            changed |= ui
                .add(
                    egui::Slider::new(&mut app.settings.ui_scale, 0.8..=2.0)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
                        .custom_parser(|s| s.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)),
                )
                .drag_stopped();
            ui.end_row();
            ui.label(app.t("set.polling"));
            let mut hz = app.settings.polling_hz;
            if ui.add(egui::Slider::new(&mut hz, 60..=1000).suffix(" Hz")).changed() {
                app.settings.polling_hz = hz;
                driftlab_core::manager::POLL_HZ.store(hz, std::sync::atomic::Ordering::Relaxed);
                changed = true;
            }
            ui.end_row();
            ui.label(app.t("set.expert"));
            let l = app.t("set.expert_hint");
            changed |= ui.checkbox(&mut app.settings.expert_mode, l).changed();
            ui.end_row();
        });
    });
    ui.add_space(16.0);
    card(ui, &t, |ui| {
        h2(ui, app.t("set.maintenance"));
        ui.horizontal_wrapped(|ui| {
            if ghost(ui, &t, app.t("set.open_logs"), true).clicked() {
                export::open_path(&storage::log_dir());
            }
            if ghost(ui, &t, app.t("set.export_logs"), true).clicked() {
                export_logs(app, now);
            }
            if primary(ui, &t, app.t("set.check_updates"), !app.settings_ui.checking).clicked() {
                app.settings_ui.checking = true;
                *app.settings_ui.update.lock() = None;
                check_updates(app.settings_ui.update.clone());
            }
        });
        if let Some(r) = app.settings_ui.update.lock().clone() {
            app.settings_ui.checking = false;
            match r {
                Ok(tag) if is_newer(&tag, env!("CARGO_PKG_VERSION")) => {
                    ui.label(format!("{} {tag}", app.t("set.update_available")));
                    if ghost(ui, &t, app.t("set.open_download"), true).clicked() {
                        export::open_url(&format!("{REPO}/releases/latest"));
                    }
                }
                Ok(_) => {
                    ui.label(app.t("set.up_to_date"));
                }
                Err(e) => {
                    ui.label(egui::RichText::new(format!("{} {e}", app.t("set.update_failed"))).color(widgets::status_color(&t, 2)));
                }
            }
        }
        widgets::dim(ui, &t, app.t("set.update_note"));
    });
    if app.settings.expert_mode {
        ui.add_space(16.0);
        card(ui, &t, |ui| {
            h2(ui, app.t("set.inspector"));
            widgets::dim(ui, &t, app.t("set.inspector_hint"));
            if let Some(d) = app.dev() {
                let job = hex_job(&d.input.raw_report, d.info.family, &t);
                ui.label(job);
            } else {
                widgets::dim(ui, &t, app.t("set.no_device"));
            }
        });
    }
    ui.add_space(16.0);
    card(ui, &t, |ui| {
        h2(ui, app.t("set.about"));
        ui.label(format!("DriftLab v{} — MIT License", env!("CARGO_PKG_VERSION")));
        ui.add_space(6.0);
        ui.label(egui::RichText::new(app.t("about.disclaimer")).strong());
        ui.label(app.t("about.trademark"));
        ui.add_space(6.0);
        h2(ui, app.t("about.credits"));
        for l in [
            "dualshock-tools (MIT) — DS4/DualSense calibration protocol reference",
            "Linux kernel hid-playstation.c / hid-sony.c (GPL-2.0, documentation only — no code copied)",
            "SDL SDL_hidapi_ps4.c / SDL_hidapi_ps5.c (zlib) — report layouts",
            "egui / eframe / wgpu (MIT/Apache-2.0) · hidapi · gilrs",
            "Inter typeface (SIL OFL 1.1) · Phosphor Icons (MIT)",
            "3D models (CC BY 4.0, modified): DualSense – be the knight · DualSense Edge – miwaVV · DualShock 4 – shaielwolf · Xbox One/Series – TdoubleU8 · Xbox Elite – Vrah (Wee.Patt), via Sketchfab",
            "Studio HDRI studio_small_03 by Poly Haven (CC0) · full details in CREDITS.md",
        ] {
            ui.label(format!("•  {l}"));
        }
    });
    if changed {
        let _ = app.settings.save(&app.config_dir);
    }
}

fn export_logs(app: &mut App, now: f64) {
    let dir = storage::log_dir();
    let newest = std::fs::read_dir(&dir)
        .ok()
        .and_then(|rd| rd.flatten().filter(|e| e.path().is_file()).max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok()));
    match newest {
        Some(f) => {
            let dst = export::dir().join(f.file_name());
            let _ = std::fs::create_dir_all(export::dir());
            match std::fs::copy(f.path(), &dst) {
                Ok(_) => {
                    app.toast(now, format!("{} {}", app.t("toast.saved"), dst.display()), false);
                    export::open_path(&export::dir());
                }
                Err(e) => app.toast(now, e.to_string(), true),
            }
        }
        None => app.toast(now, app.t("set.no_logs"), true),
    }
}

/// Field map for report 0x01 (USB layout) of PlayStation controllers: (name, start, len).
fn fields(f: Family) -> &'static [(&'static str, usize, usize)] {
    match f {
        Family::DualSense | Family::DualSenseEdge => &[
            ("id", 0, 1),
            ("sticks", 1, 4),
            ("triggers", 5, 2),
            ("buttons", 8, 3),
            ("gyro", 16, 6),
            ("accel", 22, 6),
            ("touch", 33, 8),
            ("battery", 53, 1),
        ],
        Family::DualShock4 => {
            &[("id", 0, 1), ("sticks", 1, 4), ("buttons", 5, 3), ("triggers", 8, 2), ("gyro", 13, 6), ("accel", 19, 6), ("battery", 30, 1)]
        }
        _ => &[],
    }
}

fn hex_job(data: &[u8], f: Family, t: &crate::theme::Tokens) -> LayoutJob {
    let palette = [t.accent, t.warn, t.ok, t.bad, [160, 120, 255], [255, 140, 200], [90, 200, 220], [240, 200, 80]];
    let fmap = fields(f);
    let mut job = LayoutJob::default();
    let font = FontId::monospace(13.0);
    if data.is_empty() {
        job.append("—", 0.0, TextFormat { font_id: font, color: Color32::GRAY, ..Default::default() });
        return job;
    }
    for (i, b) in data.iter().enumerate() {
        if i % 16 == 0 {
            job.append(
                &format!("{i:04X}  "),
                0.0,
                TextFormat { font_id: font.clone(), color: crate::theme::c32(t.text_dim), ..Default::default() },
            );
        }
        let col = fmap
            .iter()
            .position(|(_, s, l)| i >= *s && i < s + l)
            .map(|k| crate::theme::c32(palette[k % palette.len()]))
            .unwrap_or(crate::theme::c32(t.text_dim));
        job.append(&format!("{b:02X} "), 0.0, TextFormat { font_id: font.clone(), color: col, ..Default::default() });
        if i % 16 == 15 {
            job.append("\n", 0.0, TextFormat { font_id: font.clone(), ..Default::default() });
        }
    }
    job.append("\n", 0.0, TextFormat { font_id: font.clone(), ..Default::default() });
    for (n, s, l) in fmap {
        job.append(
            &format!("{n}[{s}..{}] ", s + l),
            0.0,
            TextFormat { font_id: font.clone(), color: crate::theme::c32(t.text_dim), ..Default::default() },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v0.1.10", "0.1.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
    }
}
