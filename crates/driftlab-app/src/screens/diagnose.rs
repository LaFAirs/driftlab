use super::verdict_info;
use crate::app::{App, Collector};
use crate::export;
use crate::theme::{c32, c32a};
use crate::widgets::{self, card, chip, dot, ghost, h1, h2, primary, status_color, StickPanelData};
use driftlab_core::calibration::DriftReport;
use driftlab_core::math::{analyze_drift, circularity, trigger_stats, Circularity, EdgeMap, TriggerStats};
use driftlab_core::{InputState, Result as CoreResult};
use egui::{vec2, Ui};

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    #[default]
    Idle,
    Rest,
    Circle,
    Trigger,
}

pub struct DiagState {
    pub phase: Phase,
    pub request_start: bool,
    pub show_corrected: bool,
    pub maps: [EdgeMap; 2],
    pub circ_start: f64,
    pub circ: Option<[Circularity; 2]>,
    pub trig_samples: [Vec<f32>; 2],
    pub trig_start: f64,
    pub trig: Option<[TriggerStats; 2]>,
    pub test_secs: f32,
    pub report_rect: Option<egui::Rect>,
    pub want_report: bool,
}

impl Default for DiagState {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            request_start: false,
            show_corrected: false,
            maps: Default::default(),
            circ_start: 0.0,
            circ: None,
            trig_samples: Default::default(),
            trig_start: 0.0,
            trig: None,
            test_secs: 5.0,
            report_rect: None,
            want_report: false,
        }
    }
}

pub fn wants_rest(app: &App) -> bool {
    app.diag.phase == Phase::Rest
}

pub fn wants_circle(app: &App) -> bool {
    app.diag.phase == Phase::Circle
}

pub fn on_input(app: &mut App, st: &InputState) {
    match app.diag.phase {
        Phase::Circle => {
            app.diag.maps[0].add(st.left.0, st.left.1);
            app.diag.maps[1].add(st.right.0, st.right.1);
        }
        Phase::Trigger => {
            app.diag.trig_samples[0].push(st.l2);
            app.diag.trig_samples[1].push(st.r2);
        }
        _ => {}
    }
}

pub fn start_rest(app: &mut App, now: f64) {
    app.diag.phase = Phase::Rest;
    app.collector = Some(Collector { start: now, secs: app.diag.test_secs, left: vec![], right: vec![] });
}

pub fn tick(app: &mut App, now: f64) {
    if app.diag.request_start && app.screen == crate::app::Screen::Diagnose && app.selected.is_some() {
        app.diag.request_start = false;
        start_rest(app, now);
    }
    match app.diag.phase {
        Phase::Rest => {
            let done = app.collector.as_ref().is_some_and(|c| (now - c.start) as f32 >= c.secs);
            if done {
                if let Some(c) = app.collector.take() {
                    let rep = DriftReport { left: analyze_drift(&c.left), right: analyze_drift(&c.right), samples: c.left.len() };
                    app.last_drift = Some(rep);
                }
                app.diag.phase = Phase::Idle;
            }
        }
        Phase::Circle => {
            let cov = app.diag.maps[0].coverage().min(app.diag.maps[1].coverage());
            if cov >= 0.95 || now - app.diag.circ_start > 30.0 {
                app.diag.circ = Some([circularity(&app.diag.maps[0]), circularity(&app.diag.maps[1])]);
                app.last_circ = Some((
                    app.diag.circ.as_ref().map(|c| c[0].clone()).unwrap_or_else(|| circularity(&app.diag.maps[0])),
                    app.diag.circ.as_ref().map(|c| c[1].clone()).unwrap_or_else(|| circularity(&app.diag.maps[1])),
                    app.diag.maps[0].clone(),
                    app.diag.maps[1].clone(),
                ));
                app.diag.phase = Phase::Idle;
            }
        }
        Phase::Trigger => {
            if now - app.diag.trig_start > 6.0 {
                app.diag.trig = Some([trigger_stats(&app.diag.trig_samples[0]), trigger_stats(&app.diag.trig_samples[1])]);
                app.diag.phase = Phase::Idle;
            }
        }
        Phase::Idle => {}
    }
}

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    let Some(d) = app.dev() else { return };
    let caps = d.caps;
    let (left, right, raw_l, raw_r) = (d.input.left, d.input.right, d.input.raw_left, d.input.raw_right);
    let trails: [Vec<(f32, f32)>; 2] = [d.trail[0].iter().map(|x| x.1).collect(), d.trail[1].iter().map(|x| x.1).collect()];
    let series: Vec<[f32; 4]> = d.series.iter().copied().collect();
    let heats = d.heat.clone();
    let corr = super::tuning::corrections(app);
    let busy = app.diag.phase != Phase::Idle;

    h1(ui, app.t("diag.title"));
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        if primary(ui, &t, app.t("diag.btn.drift"), !busy).clicked() {
            start_rest(app, now);
        }
        if ghost(ui, &t, app.t("diag.btn.circ"), !busy).clicked() {
            app.diag.maps = Default::default();
            app.diag.circ_start = now;
            app.diag.circ = None;
            app.diag.phase = Phase::Circle;
        }
        if caps.triggers && ghost(ui, &t, app.t("diag.btn.trig"), !busy).clicked() {
            app.diag.trig_samples = Default::default();
            app.diag.trig_start = now;
            app.diag.phase = Phase::Trigger;
        }
        {
            let l = app.t("diag.corrected");
            ui.checkbox(&mut app.diag.show_corrected, l);
        }
    });
    ui.add_space(12.0);

    let show = |p: (f32, f32), c: &driftlab_core::math::StickCorrection, on: bool| if on { c.apply(p.0, p.1) } else { p };
    let (pl, pr) = (show(left, &corr.0, app.diag.show_corrected), show(right, &corr.1, app.diag.show_corrected));

    super::two_cols(ui, 0.5, |lui, rui| {
        stick_card(app, lui, &t, "diag.left", pl, &trails[0], &heats[0], raw_l, 0);
        stick_card(app, rui, &t, "diag.right", pr, &trails[1], &heats[1], raw_r, 1);
    });
    ui.add_space(16.0);

    // Active test.
    if busy {
        card(ui, &t, |ui| {
            ui.horizontal(|ui| match app.diag.phase {
                Phase::Rest => {
                    let (el, secs) = app.collector.as_ref().map(|c| ((now - c.start) as f32, c.secs)).unwrap_or((0.0, 5.0));
                    widgets::ring(ui, &t, 110.0, el / secs, &format!("{:.0}", (secs - el).ceil().max(0.0)));
                    ui.vertical(|ui| {
                        h2(ui, app.t("diag.rest.title"));
                        ui.label(app.t("diag.rest.body"));
                    });
                }
                Phase::Circle => {
                    let cov = app.diag.maps[0].coverage().min(app.diag.maps[1].coverage());
                    widgets::ring(ui, &t, 110.0, cov, &format!("{:.0}%", cov * 100.0));
                    ui.vertical(|ui| {
                        h2(ui, app.t("diag.circ.title"));
                        ui.label(app.t("diag.circ.body"));
                        if ghost(ui, &t, app.t("btn.cancel"), true).clicked() {
                            app.diag.phase = Phase::Idle;
                        }
                    });
                }
                Phase::Trigger => {
                    let el = ((now - app.diag.trig_start) as f32 / 6.0).min(1.0);
                    widgets::ring(ui, &t, 110.0, el, &format!("{:.0}", (6.0 * (1.0 - el)).ceil()));
                    ui.vertical(|ui| {
                        h2(ui, app.t("diag.trig.title"));
                        ui.label(app.t("diag.trig.body"));
                    });
                }
                Phase::Idle => {}
            });
        });
        ui.add_space(16.0);
    }

    // Results.
    if let Some(rep) = app.last_drift.clone() {
        let scoped = ui.scope(|ui| {
            card(ui, &t, |ui| {
                h2(ui, app.t("diag.results"));
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    for (name, r) in [("diag.left", rep.left), ("diag.right", rep.right)] {
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() / 2.0 - 12.0).max(260.0));
                            let (lvl, key) = verdict_info(r.verdict);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(app.t(name)).strong());
                                dot(ui, status_color(&t, lvl), 7.0);
                                ui.label(egui::RichText::new(app.t(key)).color(status_color(&t, lvl)).strong());
                            });
                            let sc = widgets::anim_value(ui, egui::Id::new(("score", name)), r.score, app.reduced());
                            ui.label(widgets::num(format!("{sc:.0}"), 44.0));
                            widgets::dim(ui, &t, app.t("diag.score"));
                            egui::Grid::new(("g", name)).num_columns(2).show(ui, |ui| {
                                for (k, v) in [
                                    ("diag.offset", format!("X {:+.1} %  Y {:+.1} %", r.offset_x * 100.0, r.offset_y * 100.0)),
                                    ("diag.radius", format!("{:.1} %", r.offset_radius * 100.0)),
                                    ("diag.noise", format!("{:.2} %", r.noise * 100.0)),
                                    ("diag.jitter", format!("{:.2} %", r.jitter * 100.0)),
                                ] {
                                    ui.label(app.t(k));
                                    ui.label(widgets::num(v, 14.0));
                                    ui.end_row();
                                }
                            });
                        });
                    }
                });
                let worst = if rep.left.score >= rep.right.score { rep.left } else { rep.right };
                if worst.verdict == driftlab_core::math::DriftVerdict::HardwareWear {
                    ui.add_space(8.0);
                    super::hardware_options(app, ui, &t);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ghost(ui, &t, app.t("btn.export_csv"), true).clicked() {
                        do_export(app, now, true);
                    }
                    if ghost(ui, &t, app.t("btn.export_json"), true).clicked() {
                        do_export(app, now, false);
                    }
                    if ghost(ui, &t, app.t("btn.report"), true).clicked() {
                        app.diag.want_report = true;
                    }
                });
            });
        });
        app.diag.report_rect = Some(scoped.response.rect);
        ui.add_space(16.0);
    }

    // Circularity.
    if let Some((cl, cr, ml, mr)) = app.last_circ.clone() {
        card(ui, &t, |ui| {
            h2(ui, app.t("diag.circ.result"));
            ui.horizontal(|ui| {
                for (name, c, m) in [("diag.left", &cl, &ml), ("diag.right", &cr, &mr)] {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(app.t(name)).strong());
                        widgets::circularity_plot(ui, &t, 200.0, &m.smoothed(), c.weakest_segment);
                        ui.label(widgets::num(format!("Ø {:.1} %   max {:.1} %", c.mean_error_pct, c.max_error_pct), 14.0));
                        widgets::dim(ui, &t, &format!("{} {}°", app.t("diag.weakest"), c.weakest_segment * 5));
                    });
                    ui.add_space(24.0);
                }
            });
        });
        ui.add_space(16.0);
    }
    if app.diag.phase == Phase::Circle {
        card(ui, &t, |ui| {
            ui.horizontal(|ui| {
                widgets::coverage_ring(ui, &t, 200.0, &app.diag.maps[0], pl);
                widgets::coverage_ring(ui, &t, 200.0, &app.diag.maps[1], pr);
            });
        });
        ui.add_space(16.0);
    }

    // Triggers.
    if let Some(ts) = app.diag.trig {
        card(ui, &t, |ui| {
            h2(ui, app.t("diag.trig.result"));
            ui.horizontal(|ui| {
                for (n, s) in [("L2", ts[0]), ("R2", ts[1])] {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(n).strong());
                        ui.label(widgets::num(
                            format!(
                                "{} {:.0} %   {} {:.0} %   {} {:.0} %",
                                app.t("diag.trig.rest"),
                                s.rest * 100.0,
                                app.t("diag.trig.max"),
                                s.max * 100.0,
                                app.t("diag.trig.lin"),
                                s.linearity * 100.0
                            ),
                            14.0,
                        ));
                    });
                    ui.add_space(24.0);
                }
            });
        });
        ui.add_space(16.0);
    }

    // Time series.
    card(ui, &t, |ui| {
        h2(ui, app.t("diag.timeline"));
        let col = |i: usize| series.iter().map(|s| s[i]).collect::<Vec<f32>>();
        let (lx, ly, rx, ry) = (col(0), col(1), col(2), col(3));
        widgets::line_plot(
            ui,
            &t,
            vec2(ui.available_width(), 160.0),
            &[(&lx, c32(t.accent)), (&ly, c32a(t.accent, 130)), (&rx, c32(t.warn)), (&ry, c32a(t.warn, 130))],
            (-1.0, 1.0),
        );
        ui.horizontal(|ui| {
            chip(ui, &t, "L X/Y", true);
            chip(ui, &t, "R X/Y", false);
        });
    });
    let _ = (raw_l, raw_r);
    let _: Option<CoreResult<()>> = None;
}

#[allow(clippy::too_many_arguments)]
fn stick_card(
    app: &mut App,
    ui: &mut Ui,
    t: &crate::theme::Tokens,
    title: &'static str,
    pos: (f32, f32),
    trail: &[(f32, f32)],
    heat: &[u16],
    raw: (f32, f32),
    _i: usize,
) {
    card(ui, t, |ui| {
        h2(ui, app.t(title));
        let size = (ui.available_width()).clamp(200.0, 300.0);
        ui.horizontal(|ui| {
            widgets::stick_panel(ui, t, size, &StickPanelData { pos, trail, heat: Some(heat), deadzone: 0.0, circle_samples: &[] });
            ui.vertical(|ui| {
                let r = (pos.0 * pos.0 + pos.1 * pos.1).sqrt();
                let a = pos.1.atan2(pos.0).to_degrees().rem_euclid(360.0);
                for (k, v) in [
                    ("X", format!("{:+.3}", pos.0)),
                    ("Y", format!("{:+.3}", pos.1)),
                    ("r", format!("{r:.3}")),
                    ("θ", format!("{a:.0}°")),
                    ("raw", format!("{:.0} / {:.0}", raw.0, raw.1)),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(k).color(c32(t.text_dim)));
                        ui.label(widgets::num(v, 15.0));
                    });
                }
            });
        });
    });
}

fn do_export(app: &mut App, now: f64, csv: bool) {
    let Some(rep) = app.last_drift.clone() else { return };
    let series: Vec<[f32; 4]> = app.dev().map(|d| d.series.iter().copied().collect()).unwrap_or_default();
    let res = if csv { export::write_csv(&rep, &series) } else { export::write_json(&rep, app.dev().map(|d| &d.info)) };
    match res {
        Ok(p) => app.toast(now, format!("{} {}", app.t("toast.saved"), p.display()), false),
        Err(e) => app.toast(now, format!("{e}"), true),
    }
}
