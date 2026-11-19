//! Button / sensor / vibration / lightbar test.

use crate::app::{App, Screen};
use crate::theme::{c32, c32a};
use crate::widgets::{self, card, chip, ghost, h1, h2, StickPanelData};
use driftlab_core::OutputRequest;
use egui::{pos2, vec2, CornerRadius, Rect, Sense, Ui};

pub struct TestState {
    pub intensity: f32,
    pub lightbar: [f32; 3],
}

impl Default for TestState {
    fn default() -> Self {
        Self { intensity: 0.6, lightbar: [0.1, 0.4, 1.0] }
    }
}

pub fn wants_trigger_ramp(app: &App) -> bool {
    app.screen == Screen::Test
}

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    let Some(d) = app.dev() else { return };
    let input = d.input.clone();
    let caps = d.caps;
    let motion: Vec<[f32; 6]> = d.motion.iter().copied().collect();
    let handle = d.handle.clone();
    h1(ui, app.t("test.title"));
    ui.add_space(8.0);
    super::two_cols(ui, 0.5, |lui, rui| {
        {
            let ui = &mut *lui;
            card(ui, &t, |ui| {
                h2(ui, app.t("test.buttons"));
                ui.horizontal_wrapped(|ui| {
                    for (n, p) in &input.buttons {
                        chip(ui, &t, &n.to_uppercase(), *p);
                    }
                });
                ui.add_space(8.0);
                for (n, v) in [("L2", input.l2), ("R2", input.r2)] {
                    ui.horizontal(|ui| {
                        ui.label(widgets::num(n, 14.0));
                        widgets::progress_bar(ui, &t, 220.0, v);
                        ui.label(widgets::num(format!("{:.0} %", v * 100.0), 14.0));
                    });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    widgets::stick_panel(
                        ui,
                        &t,
                        150.0,
                        &StickPanelData { pos: input.left, trail: &[], heat: None, deadzone: 0.0, circle_samples: &[] },
                    );
                    widgets::stick_panel(
                        ui,
                        &t,
                        150.0,
                        &StickPanelData { pos: input.right, trail: &[], heat: None, deadzone: 0.0, circle_samples: &[] },
                    );
                });
                if caps.touchpad {
                    ui.add_space(8.0);
                    ui.label(app.t("test.touchpad"));
                    let (rect, _) = ui.allocate_exact_size(vec2(300.0, 120.0), Sense::hover());
                    ui.painter().rect_filled(rect, CornerRadius::same(12), c32(crate::theme::lerp_rgb(t.card, t.bg_top, 0.4)));
                    for (x, y) in &input.touch {
                        ui.painter().circle_filled(
                            pos2(rect.left() + x * rect.width(), rect.top() + y * rect.height()),
                            10.0,
                            c32(t.accent),
                        );
                    }
                }
            });
        }
        {
            let ui = &mut *rui;
            if caps.gyro {
                card(ui, &t, |ui| {
                    h2(ui, app.t("test.motion"));
                    let col = |i: usize| motion.iter().map(|m| m[i]).collect::<Vec<f32>>();
                    let (gx, gy, gz, ax, ay, az) = (col(0), col(1), col(2), col(3), col(4), col(5));
                    ui.label(app.t("test.gyro"));
                    widgets::line_plot(
                        ui,
                        &t,
                        vec2(ui.available_width(), 90.0),
                        &[(&gx, c32(t.accent)), (&gy, c32(t.warn)), (&gz, c32(t.ok))],
                        (-1.0, 1.0),
                    );
                    ui.label(app.t("test.accel"));
                    widgets::line_plot(
                        ui,
                        &t,
                        vec2(ui.available_width(), 90.0),
                        &[(&ax, c32(t.accent)), (&ay, c32(t.warn)), (&az, c32(t.ok))],
                        (-1.0, 1.0),
                    );
                });
                ui.add_space(16.0);
            }
            card(ui, &t, |ui| {
                h2(ui, app.t("test.rumble"));
                ui.add_enabled_ui(caps.rumble, |ui| {
                    let l = app.t("test.intensity");
                    ui.add(egui::Slider::new(&mut app.tester.intensity, 0.0..=1.0).text(l));
                    ui.horizontal(|ui| {
                        for (key, l, r) in [("test.left", true, false), ("test.right", false, true), ("test.both", true, true)] {
                            if ghost(ui, &t, app.t(key), true).clicked() {
                                let v = (app.tester.intensity * 255.0) as u8;
                                let req = OutputRequest {
                                    rumble_left: if l { v } else { 0 },
                                    rumble_right: if r { v } else { 0 },
                                    lightbar: None,
                                };
                                let _ = handle.exec(move |d| d.set_output(req));
                                let h2c = handle.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(std::time::Duration::from_millis(500));
                                    let _ = h2c.exec(|d| d.set_output(OutputRequest::default()));
                                });
                            }
                        }
                    });
                });
                if !caps.rumble {
                    widgets::dim(ui, &t, app.t("test.unsupported"));
                }
                if caps.lightbar {
                    ui.add_space(8.0);
                    h2(ui, app.t("test.lightbar"));
                    ui.horizontal(|ui| {
                        ui.color_edit_button_rgb(&mut app.tester.lightbar);
                        if ghost(ui, &t, app.t("btn.apply"), true).clicked() {
                            let c = app.tester.lightbar;
                            let req = OutputRequest {
                                lightbar: Some(((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)),
                                ..Default::default()
                            };
                            let r = handle.exec(move |d| d.set_output(req));
                            let _ = r;
                        }
                    });
                }
                let _ = (now, Rect::NOTHING, c32a([0, 0, 0], 0));
            });
        }
    });
}
