//! Software correction: centre offset, deadzones, curves, circularity, profiles.

use crate::app::{active_profile_mut, App};
use crate::export;
use crate::theme::{c32, c32a};
use crate::widgets::{self, card, ghost, h1, h2, primary, StickPanelData};
use driftlab_core::math::{auto_deadzone, Curve, DeadzoneKind, StickCorrection};
use driftlab_core::storage::{self, Profile};
use egui::{pos2, vec2, Rect, Sense, Stroke, Ui};

#[derive(Default)]
pub struct TuningState {
    pub stick: usize,
}

/// Software corrections of the active profile of the selected device.
pub fn corrections(app: &App) -> (StickCorrection, StickCorrection) {
    let serial = app.dev().and_then(|d| d.info.serial.clone()).unwrap_or_else(|| "unknown".into());
    app.profiles
        .by_serial
        .get(&serial)
        .and_then(|l| l.get(app.active_profile.min(l.len().saturating_sub(1))))
        .map(|p| (p.left, p.right))
        .unwrap_or_default()
}

fn curve_kind(c: &Curve) -> usize {
    match c {
        Curve::Linear => 0,
        Curve::Exponential(_) => 1,
        Curve::SCurve(_) => 2,
        Curve::Bezier { .. } => 3,
    }
}

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    let Some(d) = app.dev() else { return };
    let (left, right) = (d.input.left, d.input.right);
    let serial = d.info.serial.clone().unwrap_or_else(|| "unknown".into());
    if app.profiles.by_serial.get(&serial).is_none_or(|l| l.is_empty()) {
        app.profiles.by_serial.insert(serial.clone(), vec![Profile::default()]);
    }
    let names: Vec<String> = app.profiles.by_serial[&serial].iter().map(|p| p.name.clone()).collect();
    app.active_profile = app.active_profile.min(names.len() - 1);
    let mut prof = app.profiles.by_serial[&serial][app.active_profile].clone();
    let before = prof.clone();

    h1(ui, app.t("tune.title"));
    widgets::dim(ui, &t, app.t("tune.sub"));
    ui.add_space(8.0);

    // Profile bar.
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("profile").selected_text(&names[app.active_profile]).show_ui(ui, |ui| {
            for (i, n) in names.iter().enumerate() {
                ui.selectable_value(&mut app.active_profile, i, n);
            }
        });
        if ghost(ui, &t, app.t("tune.new"), true).clicked() {
            let l = app.profiles.by_serial.get_mut(&serial).expect("exists");
            l.push(Profile { name: format!("Profile {}", l.len() + 1), ..Default::default() });
            app.active_profile = l.len() - 1;
        }
        if names.len() > 1 && ghost(ui, &t, app.t("tune.delete"), true).clicked() {
            let l = app.profiles.by_serial.get_mut(&serial).expect("exists");
            l.remove(app.active_profile);
            app.active_profile = 0;
        }
        if ghost(ui, &t, app.t("tune.export"), true).clicked() {
            if let Some(p) = rfd::FileDialog::new()
                .set_file_name(format!("{}.driftlab.json", prof.name))
                .add_filter("DriftLab profile", &["json"])
                .save_file()
            {
                match storage::export_profile(&prof, &p) {
                    Ok(()) => app.toast(now, format!("{} {}", app.t("toast.saved"), p.display()), false),
                    Err(e) => app.toast(now, e.to_string(), true),
                }
            }
        }
        if ghost(ui, &t, app.t("tune.import"), true).clicked() {
            if let Some(p) = rfd::FileDialog::new().add_filter("DriftLab profile", &["json"]).pick_file() {
                match storage::import_profile(&p) {
                    Ok(imp) => {
                        let l = app.profiles.by_serial.get_mut(&serial).expect("exists");
                        l.push(imp);
                        app.active_profile = l.len() - 1;
                    }
                    Err(e) => app.toast(now, e.to_string(), true),
                }
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(app.t("tune.name"));
        ui.text_edit_singleline(&mut prof.name);
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (ll, lr) = (app.t("diag.left"), app.t("diag.right"));
        ui.selectable_value(&mut app.tuning.stick, 0, ll);
        ui.selectable_value(&mut app.tuning.stick, 1, lr);
    });
    let which = app.tuning.stick;
    let mut c = if which == 0 { prof.left } else { prof.right };
    let live = if which == 0 { left } else { right };
    let last_drift = app.last_drift.clone();
    let last_circ = app.last_circ.clone();

    super::two_cols(ui, 0.5, |lui, rui| {
        {
            let ui = &mut *lui;
            card(ui, &t, |ui| {
                h2(ui, app.t("tune.settings"));
                ui.add(egui::Slider::new(&mut c.center_x, -0.25..=0.25).text(app.t("tune.cx")).fixed_decimals(3));
                ui.add(egui::Slider::new(&mut c.center_y, -0.25..=0.25).text(app.t("tune.cy")).fixed_decimals(3));
                egui::ComboBox::from_label(app.t("tune.kind")).selected_text(kind_name(c.kind)).show_ui(ui, |ui| {
                    for k in [DeadzoneKind::Radial, DeadzoneKind::Axial, DeadzoneKind::ScaledRadial, DeadzoneKind::Bowtie] {
                        ui.selectable_value(&mut c.kind, k, kind_name(k));
                    }
                });
                ui.add(egui::Slider::new(&mut c.inner_deadzone, 0.0..=0.5).text(app.t("tune.inner")).fixed_decimals(3));
                ui.add(egui::Slider::new(&mut c.outer_deadzone, 0.0..=0.3).text(app.t("tune.outer")).fixed_decimals(3));
                ui.add(egui::Slider::new(&mut c.anti_deadzone, 0.0..=0.5).text(app.t("tune.anti")).fixed_decimals(3));
                ui.add(egui::Slider::new(&mut c.circularity_gain, 0.8..=1.4).text(app.t("tune.circ")).fixed_decimals(3));
                // Curve.
                let mut k = curve_kind(&c.curve);
                egui::ComboBox::from_label(app.t("tune.curve"))
                    .selected_text(app.t(["curve.linear", "curve.exp", "curve.s", "curve.bezier"][k]))
                    .show_ui(ui, |ui| {
                        for (i, key) in ["curve.linear", "curve.exp", "curve.s", "curve.bezier"].iter().enumerate() {
                            ui.selectable_value(&mut k, i, app.t(key));
                        }
                    });
                if k != curve_kind(&c.curve) {
                    c.curve = match k {
                        0 => Curve::Linear,
                        1 => Curve::Exponential(1.5),
                        2 => Curve::SCurve(0.6),
                        _ => Curve::Bezier { x1: 0.3, y1: 0.1, x2: 0.7, y2: 0.9 },
                    };
                }
                match &mut c.curve {
                    Curve::Exponential(g) => {
                        ui.add(egui::Slider::new(g, 0.4..=3.0).text("γ"));
                    }
                    Curve::SCurve(s) => {
                        ui.add(egui::Slider::new(s, 0.0..=1.0).text(app.t("tune.strength")));
                    }
                    Curve::Bezier { x1, y1, x2, y2 } => bezier_editor(ui, &t, x1, y1, x2, y2),
                    Curve::Linear => {}
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ghost(ui, &t, app.t("tune.take_offset"), last_drift.is_some()).clicked() {
                        if let Some(r) = &last_drift {
                            let s = if which == 0 { r.left } else { r.right };
                            c.center_x = s.offset_x;
                            c.center_y = s.offset_y;
                        }
                    }
                    if ghost(ui, &t, app.t("tune.take_circ"), last_circ.is_some()).clicked() {
                        if let Some((_, _, ml, mr)) = &last_circ {
                            let sm: Vec<f32> = (if which == 0 { ml } else { mr }).smoothed().into_iter().filter(|r| *r > 0.0).collect();
                            if !sm.is_empty() {
                                c.circularity_gain = (sm.len() as f32 / sm.iter().sum::<f32>()).clamp(0.9, 1.3);
                            }
                        }
                    }
                    if primary(ui, &t, app.t("tune.auto"), last_drift.is_some()).clicked() {
                        if let Some(r) = &last_drift {
                            let mut s = if which == 0 { r.left } else { r.right };
                            s.offset_radius = 0.0;
                            c.center_x = (if which == 0 { r.left } else { r.right }).offset_x;
                            c.center_y = (if which == 0 { r.left } else { r.right }).offset_y;
                            c.inner_deadzone = auto_deadzone(&s).max(0.01);
                        }
                    }
                });
                if last_drift.is_none() {
                    widgets::dim(ui, &t, app.t("tune.auto_hint"));
                }
            });
        }
        {
            let ui = &mut *rui;
            card(ui, &t, |ui| {
                h2(ui, app.t("tune.preview"));
                let out = c.apply(live.0, live.1);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(app.t("tune.input"));
                        widgets::stick_panel(
                            ui,
                            &t,
                            190.0,
                            &StickPanelData { pos: live, trail: &[], heat: None, deadzone: c.inner_deadzone, circle_samples: &[] },
                        );
                    });
                    ui.vertical(|ui| {
                        ui.label(app.t("tune.output"));
                        widgets::stick_panel(
                            ui,
                            &t,
                            190.0,
                            &StickPanelData { pos: out, trail: &[], heat: None, deadzone: 0.0, circle_samples: &[] },
                        );
                    });
                });
                ui.add_space(8.0);
                ui.label(app.t("tune.response"));
                response_plot(ui, &t, &c);
            });
            ui.add_space(16.0);
            card(ui, &t, |ui| {
                h2(ui, app.t("tune.system"));
                let mut off = false;
                ui.add_enabled(false, egui::Checkbox::new(&mut off, app.t("tune.system_toggle")));
                ui.label(app.t(system_note_key()));
                ui.add_space(6.0);
                if ghost(ui, &t, app.t("tune.steam"), true).clicked() {
                    let txt = storage::steam_export_text(&prof);
                    let p = export::dir().join(format!("{}-steam.txt", prof.name.replace(' ', "_")));
                    let _ = std::fs::create_dir_all(export::dir());
                    match std::fs::write(&p, txt) {
                        Ok(()) => {
                            app.toast(now, format!("{} {}", app.t("toast.saved"), p.display()), false);
                            export::open_path(&export::dir());
                        }
                        Err(e) => app.toast(now, e.to_string(), true),
                    }
                }
            });
        }
    });

    if which == 0 {
        prof.left = c;
    } else {
        prof.right = c;
    }
    if prof != before {
        if let Some(p) = active_profile_mut(app) {
            *p = prof;
        }
    }
}

fn system_note_key() -> &'static str {
    if cfg!(target_os = "windows") {
        "tune.system_note_win"
    } else if cfg!(target_os = "linux") {
        "tune.system_note_linux"
    } else {
        "tune.system_note_mac"
    }
}

fn kind_name(k: DeadzoneKind) -> &'static str {
    match k {
        DeadzoneKind::Radial => "Radial",
        DeadzoneKind::Axial => "Axial",
        DeadzoneKind::ScaledRadial => "Scaled radial",
        DeadzoneKind::Bowtie => "Bowtie",
    }
}

fn response_plot(ui: &mut Ui, t: &crate::theme::Tokens, c: &StickCorrection) {
    let size = vec2(ui.available_width().min(380.0), 150.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, egui::CornerRadius::same(8), c32(crate::theme::lerp_rgb(t.card, t.bg_top, 0.4)));
    p.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(1.0, c32a(t.border, 200)));
    let pts: Vec<egui::Pos2> = (0..=60)
        .map(|i| {
            let x = i as f32 / 60.0;
            let (ox, _) = (StickCorrection { center_x: 0.0, center_y: 0.0, ..*c }).apply(x, 0.0);
            pos2(rect.left() + x * rect.width(), rect.bottom() - ox.abs() * rect.height())
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(2.5, c32(t.accent))));
}

fn bezier_editor(ui: &mut Ui, t: &crate::theme::Tokens, x1: &mut f32, y1: &mut f32, x2: &mut f32, y2: &mut f32) {
    let s = 200.0;
    let (rect, _) = ui.allocate_exact_size(vec2(s, s), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, egui::CornerRadius::same(8), c32(crate::theme::lerp_rgb(t.card, t.bg_top, 0.4)));
    let to = |x: f32, y: f32| pos2(rect.left() + x * s, rect.bottom() - y * s);
    p.line_segment([to(0.0, 0.0), to(*x1, *y1)], Stroke::new(1.0, c32a(t.text_dim, 140)));
    p.line_segment([to(1.0, 1.0), to(*x2, *y2)], Stroke::new(1.0, c32a(t.text_dim, 140)));
    let curve = Curve::Bezier { x1: *x1, y1: *y1, x2: *x2, y2: *y2 };
    let pts: Vec<egui::Pos2> = (0..=50).map(|i| to(i as f32 / 50.0, curve.eval(i as f32 / 50.0))).collect();
    p.add(egui::Shape::line(pts, Stroke::new(2.5, c32(t.accent))));
    for (i, (x, y)) in [(&mut *x1, &mut *y1), (&mut *x2, &mut *y2)].into_iter().enumerate() {
        let id = ui.id().with(("bz", i));
        let hr = Rect::from_center_size(to(*x, *y), vec2(22.0, 22.0));
        let resp = ui.interact(hr, id, Sense::drag());
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                *x = ((pos.x - rect.left()) / s).clamp(0.0, 1.0);
                *y = ((rect.bottom() - pos.y) / s).clamp(0.0, 1.0);
            }
        }
        p.circle_filled(to(*x, *y), 7.0, c32(t.accent));
        p.circle_stroke(to(*x, *y), 7.0, Stroke::new(2.0, c32(t.card)));
    }
}
