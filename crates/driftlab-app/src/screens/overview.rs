use super::{two_cols, verdict_info};
use crate::app::{App, Screen};
use crate::view3d;
use crate::widgets::{self, card, chip, dot, ghost, h1, h2, primary, status_color};
use driftlab_core::ChargeState;
use egui::{vec2, Ui};

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    let Some(d) = app.dev() else { return };
    let info = d.info.clone();
    let caps = d.caps;
    let battery = d.input.battery;
    let rate = d.rate_hz;
    h1(ui, app.t("overview.title"));
    ui.add_space(8.0);
    let reduced = app.reduced();
    let mut go: Option<Screen> = None;
    two_cols(ui, 0.6, |lui, rui| {
        {
            let ui = &mut *lui;
            card(ui, &t, |ui| {
                let h = (ui.available_width() * 0.78).clamp(320.0, 520.0);
                view3d::show(app, ui, vec2(ui.available_width(), h), now, true);
                ui.horizontal_wrapped(|ui| {
                    widgets::dim(ui, &t, app.t("overview.viewhint"));
                    {
                        let l = app.t("overview.gyro");
                        ui.checkbox(&mut app.view.gyro_tilt, l);
                    }
                    if !app.view.uses_3d() {
                        widgets::chip(ui, &t, app.t("overview.fallback2d"), false);
                    }
                });
            });
        }
        {
            let ui = &mut *rui;
            card(ui, &t, |ui| {
                h2(ui, &info.name);
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    chip(
                        ui,
                        &t,
                        match info.connection {
                            driftlab_core::Connection::Usb => "USB",
                            driftlab_core::Connection::Bluetooth => "Bluetooth",
                            driftlab_core::Connection::Virtual => "Demo",
                            driftlab_core::Connection::Unknown => "—",
                        },
                        true,
                    );
                    if let Some(b) = battery {
                        let ch = match b.state {
                            ChargeState::Charging => " ⚡",
                            _ => "",
                        };
                        chip(ui, &t, &format!("{} %{ch}", b.percent), b.percent >= 30);
                    }
                    if let Some(f) = &info.firmware {
                        chip(ui, &t, &format!("FW {f}"), false);
                    }
                    chip(ui, &t, &format!("{:.0} Hz", rate), false);
                });
                ui.add_space(8.0);
                egui::Grid::new("devgrid").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                    ui.label(app.t("overview.serial"));
                    ui.label(widgets::num(info.masked_serial(), 14.0));
                    ui.end_row();
                    ui.label("VID:PID");
                    ui.label(widgets::num(format!("{:04X}:{:04X}", info.vid, info.pid), 14.0));
                    ui.end_row();
                });
                ui.add_space(8.0);
                widgets::dim(ui, &t, app.t("overview.caps"));
                ui.horizontal_wrapped(|ui| {
                    for (on, k) in [
                        (caps.hw_calibration, "cap.hw"),
                        (true, "cap.sw"),
                        (caps.triggers, "cap.triggers"),
                        (caps.gyro, "cap.gyro"),
                        (caps.touchpad, "cap.touchpad"),
                        (caps.rumble, "cap.rumble"),
                        (caps.lightbar, "cap.lightbar"),
                    ] {
                        if on {
                            chip(ui, &t, app.t(k), true);
                        }
                    }
                });
            });
            ui.add_space(16.0);
            card(ui, &t, |ui| {
                h2(ui, app.t("overview.quick"));
                ui.add_space(6.0);
                match &app.last_drift {
                    Some(r) => {
                        let worst = if r.left.score >= r.right.score { r.left } else { r.right };
                        let (lvl, key) = verdict_info(worst.verdict);
                        ui.horizontal(|ui| {
                            dot(ui, status_color(&t, lvl), 9.0);
                            ui.label(egui::RichText::new(app.t(key)).strong());
                            ui.label(widgets::num(format!("  {:.0}/100", worst.score), 16.0));
                        });
                    }
                    None => {
                        ui.horizontal(|ui| {
                            widgets::skeleton(ui, &t, vec2(160.0, 18.0), reduced);
                        });
                        widgets::dim(ui, &t, app.t("overview.noresult"));
                    }
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if primary(ui, &t, app.t("btn.drift_test"), true).clicked() {
                        go = Some(Screen::Diagnose);
                        app.diag.request_start = true;
                    }
                    if ghost(ui, &t, app.t("btn.calibrate"), true).clicked() {
                        go = Some(Screen::Wizard);
                    }
                });
            });
        }
    });
    if let Some(s) = go {
        app.go(s, now);
    }
}
