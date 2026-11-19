//! Calibration wizard: precheck → backup → centre → range → triggers → compare → decide → done.
//! Hardware calibration is always applied temporarily first and only written permanently on request.

use crate::app::{App, Collector, OpResult, Screen};
use crate::theme::{c32, c32a};
use crate::view3d;
use crate::widgets::{self, card, chip, dot, ghost, h1, h2, primary, status_color};
use driftlab_core::calibration::{compare, hw_center_calibration, DriftReport};
use driftlab_core::math::{
    analyze_drift, auto_deadzone, circularity, estimate_center, median, CenterEstimate, Circularity, DriftResult, EdgeMap, StickCorrection,
};
use driftlab_core::safety::{preflight, CheckStatus, Preflight};
use driftlab_core::storage::{self, Profile};
use driftlab_core::*;
use egui::{vec2, Ui};

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    #[default]
    Precheck,
    Backup,
    Center,
    Range,
    Trigger,
    Compare,
    Decide,
    Done,
}

const STEPS: [(Step, &str); 8] = [
    (Step::Precheck, "wiz.s.precheck"),
    (Step::Backup, "wiz.s.backup"),
    (Step::Center, "wiz.s.center"),
    (Step::Range, "wiz.s.range"),
    (Step::Trigger, "wiz.s.trigger"),
    (Step::Compare, "wiz.s.compare"),
    (Step::Decide, "wiz.s.decide"),
    (Step::Done, "wiz.s.done"),
];

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sub {
    #[default]
    Intro,
    Before,
    Round(u8),
    Working,
    Ready,
}

#[derive(Default)]
pub struct Wizard {
    pub step: Step,
    pub sub: Sub,
    pub hw: bool,
    pub force_sw: bool,
    pub ack: bool,
    pub backup_path: Option<std::path::PathBuf>,
    pub backup_blob: Option<CalibrationBlob>,
    pub backup_requested: bool,
    pub before: Option<DriftReport>,
    pub after: Option<DriftReport>,
    pub rounds: Vec<(CenterEstimate, CenterEstimate)>,
    pub centers: Option<(CenterEstimate, CenterEstimate)>,
    pub maps: [EdgeMap; 2],
    pub session: Option<CalibSession>,
    pub range_started: bool,
    pub range_start_t: f64,
    pub trig: Option<[(f32, f32); 2]>,
    pub trig_start: f64,
    pub trig_run: bool,
    pub sw_corr: Option<(StickCorrection, StickCorrection)>,
    pub hw_applied: bool,
    pub committed: bool,
    pub saved_profile: bool,
    pub err: Option<String>,
    pub done_t: f64,
    pub unstable: bool,
}

pub fn wants_rest(app: &App) -> bool {
    app.screen == Screen::Wizard
        && matches!((app.wizard.step, app.wizard.sub), (Step::Center, Sub::Before | Sub::Round(_)) | (Step::Compare, Sub::Before))
}

pub fn wants_circle(app: &App) -> bool {
    app.screen == Screen::Wizard && app.wizard.step == Step::Range && (!app.wizard.hw || app.wizard.range_started)
}

/// Screenshot helper: jump straight to a realistic "before/after" comparison.
pub fn script_to_compare(app: &mut App, _now: f64) {
    let synth = |ox: f32, oy: f32, noise: f32| -> Vec<(f32, f32)> {
        (0..200).map(|i| (ox + noise * (i as f32 * 1.7).sin(), oy + noise * (i as f32 * 2.3).cos())).collect()
    };
    let before =
        DriftReport { left: analyze_drift(&synth(0.06, -0.04, 0.004)), right: analyze_drift(&synth(0.02, 0.03, 0.004)), samples: 200 };
    let after =
        DriftReport { left: analyze_drift(&synth(0.002, -0.001, 0.002)), right: analyze_drift(&synth(0.001, 0.001, 0.002)), samples: 200 };
    app.wizard = Wizard { step: Step::Compare, sub: Sub::Ready, hw: true, before: Some(before), after: Some(after), ..Default::default() };
}

fn reset(app: &mut App) {
    app.wizard = Wizard::default();
    app.collector = None;
}

pub fn on_disconnect(app: &mut App) {
    if app.wizard.step != Step::Precheck {
        reset(app);
        app.wizard.err = Some(app.t("wiz.err.disconnected").to_string());
    }
}

/// Called when leaving the wizard screen: abort any open hardware session.
pub fn abort_if_running(app: &mut App) {
    let w = &app.wizard;
    if w.hw && matches!(w.step, Step::Center | Step::Range | Step::Trigger | Step::Compare | Step::Decide) && !w.committed {
        app.run_job("abort", OpResult::Unit, |d| d.abort());
    }
    if !matches!(app.wizard.step, Step::Done) {
        reset(app);
    }
}

pub fn on_input(app: &mut App, st: &InputState) {
    let w = &mut app.wizard;
    if w.step == Step::Range && (!w.hw || w.range_started) {
        w.maps[0].add(st.left.0, st.left.1);
        w.maps[1].add(st.right.0, st.right.1);
    }
    if w.step == Step::Trigger && w.trig_run {
        let t = w.trig.get_or_insert([(1.0, 0.0), (1.0, 0.0)]);
        for (i, v) in [st.l2, st.r2].into_iter().enumerate() {
            t[i].0 = t[i].0.min(v);
            t[i].1 = t[i].1.max(v);
        }
    }
}

pub fn on_op_result(app: &mut App, label: &'static str, r: OpResult, now: f64) {
    match (label, r) {
        ("backup", OpResult::Backup(Ok(Some(blob)))) => {
            let Some(info) = app.dev().map(|d| d.info.clone()) else { return };
            match storage::create_backup(&app.data_dir, &info, blob.clone()) {
                Ok(p) => {
                    app.wizard.backup_path = Some(p);
                    app.wizard.backup_blob = Some(blob);
                    app.wizard.sub = Sub::Ready;
                }
                Err(e) => backup_failed(app, e.to_string()),
            }
        }
        ("backup", OpResult::Backup(Ok(None))) => backup_failed(app, app.t("wiz.err.nobackup").to_string()),
        ("backup", OpResult::Backup(Err(e))) => backup_failed(app, e.to_string()),
        ("hw_center", OpResult::Unit(Ok(()))) => {
            app.wizard.hw_applied = true;
            app.wizard.step = Step::Range;
            app.wizard.sub = Sub::Intro;
        }
        ("range_begin", OpResult::Session(Ok(s))) => {
            app.wizard.session = Some(s);
            app.wizard.range_started = true;
            app.wizard.range_start_t = now;
        }
        ("range_apply", OpResult::Unit(Ok(()))) => {
            app.wizard.session = None;
            app.wizard.range_started = false;
            next_after_range(app);
        }
        ("commit", OpResult::Unit(Ok(()))) => {
            app.wizard.committed = true;
            app.wizard.step = Step::Done;
            app.wizard.done_t = now;
        }
        ("restore", OpResult::Unit(Ok(()))) => app.toast(now, app.t("toast.restored"), false),
        ("abort", OpResult::Unit(Ok(()))) => {}
        (_, OpResult::Unit(Err(e))) | (_, OpResult::Session(Err(e))) | (_, OpResult::Backup(Err(e))) => {
            let msg = format!("{}: {e}", app.t("wiz.err.hw"));
            app.wizard.err = Some(msg.clone());
            app.toast(now, msg, true);
            // Safe state: abort session, fall back to Precheck.
            app.run_job("abort", OpResult::Unit, |d| d.abort());
            let err = app.wizard.err.take();
            reset(app);
            app.wizard.err = err;
        }
        _ => {}
    }
}

fn backup_failed(app: &mut App, msg: String) {
    app.wizard.err = Some(format!("{} {msg}", app.t("wiz.err.backup")));
    app.wizard.hw = false;
    app.wizard.force_sw = true;
    app.wizard.step = Step::Precheck;
    app.wizard.sub = Sub::Intro;
}

fn next_after_range(app: &mut App) {
    let has_trig = app.dev().is_some_and(|d| d.caps.triggers);
    app.wizard.step = if has_trig { Step::Trigger } else { Step::Compare };
    app.wizard.sub = Sub::Intro;
    if !has_trig {
        prepare_compare(app);
    }
}

/// Build the software correction (used on the software path and as the profile to save).
fn build_sw_correction(w: &Wizard) -> (StickCorrection, StickCorrection) {
    let build = |c: Option<&CenterEstimate>, d: Option<&DriftResult>, m: &EdgeMap, hw: bool| {
        let mut s = StickCorrection::default();
        if let Some(c) = c {
            if !hw {
                s.center_x = c.x;
                s.center_y = c.y;
            }
        }
        if let Some(d) = d {
            let mut resid = *d;
            resid.offset_radius = 0.0; // centre removed separately
            s.inner_deadzone = auto_deadzone(&resid).max(0.02);
        }
        let sm: Vec<f32> = m.smoothed().into_iter().filter(|r| *r > 0.0).collect();
        if sm.len() > 36 {
            let avg = sm.iter().sum::<f32>() / sm.len() as f32;
            s.circularity_gain = (1.0 / avg.max(0.5)).clamp(0.9, 1.3);
        }
        s
    };
    let (cl, cr) = w.centers.map(|c| (Some(c.0), Some(c.1))).unwrap_or((None, None));
    let b = w.before.as_ref();
    (build(cl.as_ref(), b.map(|r| &r.left), &w.maps[0], w.hw), build(cr.as_ref(), b.map(|r| &r.right), &w.maps[1], w.hw))
}

fn prepare_compare(app: &mut App) {
    if !app.wizard.hw {
        app.wizard.sw_corr = Some(build_sw_correction(&app.wizard));
    }
}

pub fn tick(app: &mut App, now: f64) {
    if app.screen != Screen::Wizard {
        return;
    }
    // Auto-backup on entering the backup step.
    if app.wizard.step == Step::Backup && !app.wizard.backup_requested && app.pending.is_none() {
        app.wizard.backup_requested = true;
        app.wizard.sub = Sub::Working;
        app.run_job("backup", OpResult::Backup, |d| d.read_calibration());
    }
    let done = app.collector.as_ref().is_some_and(|c| (now - c.start) as f32 >= c.secs);
    if done {
        let c = app.collector.take();
        if let Some(c) = c {
            match (app.wizard.step, app.wizard.sub) {
                (Step::Center, Sub::Before) => {
                    app.wizard.before =
                        Some(DriftReport { left: analyze_drift(&c.left), right: analyze_drift(&c.right), samples: c.left.len() });
                    app.wizard.rounds.clear();
                    app.wizard.sub = Sub::Round(0);
                    app.collector = Some(Collector { start: now, secs: 1.2, left: vec![], right: vec![] });
                }
                (Step::Center, Sub::Round(n)) => {
                    let (l, r) = (estimate_center(&c.left), estimate_center(&c.right));
                    if l.stable && r.stable {
                        app.wizard.unstable = false;
                        app.wizard.rounds.push((l, r));
                        if n + 1 >= 3 {
                            finish_center(app);
                        } else {
                            app.wizard.sub = Sub::Round(n + 1);
                            app.collector = Some(Collector { start: now, secs: 1.2, left: vec![], right: vec![] });
                        }
                    } else {
                        // Movement detected: restart the rounds with a hint.
                        app.wizard.unstable = true;
                        app.wizard.rounds.clear();
                        app.wizard.sub = Sub::Round(0);
                        app.collector = Some(Collector { start: now, secs: 1.2, left: vec![], right: vec![] });
                    }
                }
                (Step::Compare, Sub::Before) => {
                    let (mut l, mut r) = (c.left, c.right);
                    if let Some((cl, cr)) = app.wizard.sw_corr.filter(|_| !app.wizard.hw) {
                        l = l.into_iter().map(|p| cl.apply(p.0, p.1)).collect();
                        r = r.into_iter().map(|p| cr.apply(p.0, p.1)).collect();
                    }
                    app.wizard.after = Some(DriftReport { left: analyze_drift(&l), right: analyze_drift(&r), samples: l.len() });
                    app.wizard.sub = Sub::Ready;
                }
                _ => {}
            }
        }
    }
    // Trigger window.
    if app.wizard.step == Step::Trigger && app.wizard.trig_run && now - app.wizard.trig_start > 6.0 {
        app.wizard.trig_run = false;
        app.wizard.sub = Sub::Ready;
    }
}

fn finish_center(app: &mut App) {
    let r = &app.wizard.rounds;
    let med = |f: &dyn Fn(&(CenterEstimate, CenterEstimate)) -> f32| median(&r.iter().map(f).collect::<Vec<_>>());
    let mk = |x: f32, y: f32| CenterEstimate { x, y, std_x: 0.0, std_y: 0.0, samples_used: 0, stable: true };
    app.wizard.centers = Some((mk(med(&|c| c.0.x), med(&|c| c.0.y)), mk(med(&|c| c.1.x), med(&|c| c.1.y))));
    if app.wizard.hw {
        app.wizard.sub = Sub::Working;
        app.run_job("hw_center", OpResult::Unit, |d| hw_center_calibration(d, 3, 100));
    } else {
        app.wizard.step = Step::Range;
        app.wizard.sub = Sub::Intro;
    }
}

fn pf(app: &App) -> Option<Preflight> {
    let d = app.dev()?;
    Some(preflight(&d.info, &d.caps, d.input.battery, app.settings.expert_mode))
}

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    h1(ui, app.t("wiz.title"));
    ui.add_space(8.0);
    // Progress.
    let idx = STEPS.iter().position(|(s, _)| *s == app.wizard.step).unwrap_or(0);
    let frac = widgets::anim_value(ui, egui::Id::new("wiz_prog"), (idx as f32 + 1.0) / STEPS.len() as f32, app.reduced());
    widgets::progress_bar(ui, &t, ui.available_width(), frac);
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for (i, (_, k)) in STEPS.iter().enumerate() {
            chip(ui, &t, app.t(k), i == idx);
        }
    });
    ui.add_space(16.0);
    if let Some(e) = app.wizard.err.clone() {
        egui::Frame::new()
            .fill(c32a(t.bad, 28))
            .stroke(egui::Stroke::new(1.0, c32(t.bad)))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&e).color(c32(t.bad)));
                    if ui.small_button(app.t("btn.copy_details")).clicked() {
                        ui.ctx().copy_text(e.clone());
                    }
                });
            });
        ui.add_space(12.0);
    }
    let busy = app.pending.is_some();
    super::two_cols(ui, 0.55, |lui, rui| {
        card(lui, &t, |ui| step_body(app, ui, now, busy));
        card(rui, &t, |ui| {
            let w = ui.available_width();
            view3d::show(app, ui, vec2(w, 300.0), now, true);
        });
    });
}

fn nav_row(app: &mut App, ui: &mut Ui, next_label: &'static str, can_next: bool) -> (bool, bool) {
    let t = app.tokens;
    let (mut next, mut cancel) = (false, false);
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        next = primary(ui, &t, app.t(next_label), can_next).clicked();
        cancel = ghost(ui, &t, app.t("btn.cancel"), true).clicked();
    });
    (next, cancel)
}

fn step_body(app: &mut App, ui: &mut Ui, now: f64, busy: bool) {
    let t = app.tokens;
    let Some(dev) = app.dev() else { return };
    let caps = dev.caps;
    match app.wizard.step {
        Step::Precheck => {
            h2(ui, app.t("wiz.pre.title"));
            ui.label(app.t("wiz.pre.body"));
            ui.add_space(8.0);
            let Some(p) = pf(app) else { return };
            for c in &p.checks {
                ui.horizontal(|ui| {
                    let lvl = match c.status {
                        CheckStatus::Pass => 0,
                        CheckStatus::Warn => 1,
                        CheckStatus::Block => 2,
                    };
                    dot(ui, status_color(&t, lvl), 6.0);
                    ui.label(&c.detail);
                });
            }
            let hw_possible = caps.hw_calibration && !p.blocked() && !app.wizard.force_sw;
            app.wizard.hw = hw_possible;
            ui.add_space(8.0);
            if hw_possible {
                ui.label(egui::RichText::new(app.t("wiz.pre.hw")).strong());
                egui::Frame::new()
                    .fill(c32a(t.warn, 28))
                    .stroke(egui::Stroke::new(1.0, c32(t.warn)))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(app.t("wiz.pre.warning"));
                        let l = app.t("wiz.pre.ack");
                        ui.checkbox(&mut app.wizard.ack, l);
                    });
            } else {
                ui.label(egui::RichText::new(app.t("wiz.pre.sw")).strong());
                ui.label(app.t(if caps.hw_calibration { "wiz.pre.sw_why_blocked" } else { "wiz.pre.sw_why_unsupported" }));
            }
            let can = !hw_possible || app.wizard.ack;
            let (next, cancel) = nav_row(app, ui, "btn.start", can);
            if next {
                app.wizard.step = if hw_possible { Step::Backup } else { Step::Center };
                app.wizard.sub = Sub::Intro;
                app.wizard.err = None;
            }
            if cancel {
                app.go(Screen::Overview, now);
            }
        }
        Step::Backup => {
            h2(ui, app.t("wiz.backup.title"));
            ui.label(app.t("wiz.backup.body"));
            ui.add_space(8.0);
            if busy || app.wizard.sub == Sub::Working {
                widgets::skeleton(ui, &t, vec2(ui.available_width(), 18.0), app.reduced());
            }
            if let Some(p) = &app.wizard.backup_path {
                ui.label(egui::RichText::new(app.t("wiz.backup.saved")).color(c32(t.ok)));
                ui.label(widgets::num(p.display().to_string(), 12.5));
            }
            let (next, cancel) = nav_row(app, ui, "btn.next", app.wizard.backup_path.is_some());
            if next {
                app.wizard.step = Step::Center;
                app.wizard.sub = Sub::Intro;
            }
            if cancel {
                reset(app);
            }
        }
        Step::Center => {
            h2(ui, app.t("wiz.center.title"));
            ui.label(app.t("wiz.center.body"));
            ui.add_space(8.0);
            match app.wizard.sub {
                Sub::Intro => {
                    let (next, cancel) = nav_row(app, ui, "btn.begin", true);
                    if next {
                        app.wizard.sub = Sub::Before;
                        app.collector = Some(Collector { start: now, secs: app.diag.test_secs, left: vec![], right: vec![] });
                    }
                    if cancel {
                        reset(app);
                    }
                }
                Sub::Before | Sub::Round(_) => {
                    let (el, secs) = app.collector.as_ref().map(|c| ((now - c.start) as f32, c.secs)).unwrap_or((0.0, 1.0));
                    let label = match app.wizard.sub {
                        Sub::Round(n) => format!("{}/3", n + 1),
                        _ => format!("{:.0}", (secs - el).ceil().max(0.0)),
                    };
                    widgets::ring(ui, &t, 120.0, el / secs, &label);
                    ui.label(app.t(if app.wizard.sub == Sub::Before { "wiz.center.before" } else { "wiz.center.rounds" }));
                    if app.wizard.unstable {
                        ui.label(egui::RichText::new(app.t("wiz.center.moved")).color(c32(t.warn)).strong());
                    }
                    if ghost(ui, &t, app.t("btn.cancel"), true).clicked() {
                        reset(app);
                    }
                }
                Sub::Working | Sub::Ready => {
                    widgets::skeleton(ui, &t, vec2(ui.available_width(), 18.0), app.reduced());
                    ui.label(app.t("wiz.working"));
                }
            }
        }
        Step::Range => {
            h2(ui, app.t("wiz.range.title"));
            ui.label(app.t("wiz.range.body"));
            ui.add_space(8.0);
            let hw = app.wizard.hw;
            if hw && !app.wizard.range_started && app.wizard.session.is_none() {
                if app.pending.is_none() && app.wizard.sub == Sub::Intro {
                    app.wizard.sub = Sub::Working;
                    app.run_job("range_begin", OpResult::Session, |d| d.begin_calibration(CalibKind::Range));
                }
                widgets::skeleton(ui, &t, vec2(ui.available_width(), 18.0), app.reduced());
                return;
            }
            let live = app.dev().map(|d| (d.input.left, d.input.right)).unwrap_or_default();
            let cov = app.wizard.maps[0].coverage().min(app.wizard.maps[1].coverage());
            ui.horizontal(|ui| {
                widgets::coverage_ring(ui, &t, 150.0, &app.wizard.maps[0], live.0);
                widgets::coverage_ring(ui, &t, 150.0, &app.wizard.maps[1], live.1);
            });
            widgets::progress_bar(ui, &t, ui.available_width(), cov / 0.95);
            ui.label(widgets::num(format!("{:.0} % / 95 %", cov * 100.0), 14.0));
            let (next, cancel) = nav_row(app, ui, "btn.next", cov >= 0.95 && !busy);
            if next {
                if hw {
                    app.run_job("range_apply", OpResult::Unit, |d| {
                        // Session is owned by the UI; the driver only needs its kind/state.
                        d.apply_temporary(CalibSession { kind: CalibKind::Range, samples: 0 })
                    });
                } else {
                    next_after_range(app);
                }
            }
            if cancel {
                abort_if_running(app);
                reset(app);
            }
        }
        Step::Trigger => {
            h2(ui, app.t("wiz.trig.title"));
            ui.label(app.t("wiz.trig.body"));
            ui.add_space(8.0);
            let (l2, r2) = app.dev().map(|d| (d.input.l2, d.input.r2)).unwrap_or_default();
            ui.horizontal(|ui| {
                widgets::progress_bar(ui, &t, 160.0, l2);
                widgets::progress_bar(ui, &t, 160.0, r2);
            });
            if let Some(tr) = app.wizard.trig {
                ui.label(widgets::num(
                    format!("L2 {:.0}–{:.0} %   R2 {:.0}–{:.0} %", tr[0].0 * 100.0, tr[0].1 * 100.0, tr[1].0 * 100.0, tr[1].1 * 100.0),
                    14.0,
                ));
            }
            if app.wizard.trig_run {
                let el = ((now - app.wizard.trig_start) / 6.0) as f32;
                widgets::progress_bar(ui, &t, ui.available_width(), el);
            }
            ui.horizontal(|ui| {
                if !app.wizard.trig_run && ghost(ui, &t, app.t("btn.begin"), true).clicked() {
                    app.wizard.trig = None;
                    app.wizard.trig_run = true;
                    app.wizard.trig_start = now;
                }
                if primary(ui, &t, app.t(if app.wizard.trig.is_some() { "btn.next" } else { "btn.skip" }), !app.wizard.trig_run).clicked() {
                    app.wizard.step = Step::Compare;
                    app.wizard.sub = Sub::Intro;
                    prepare_compare(app);
                }
            });
        }
        Step::Compare => {
            h2(ui, app.t("wiz.compare.title"));
            match app.wizard.sub {
                Sub::Intro => {
                    ui.label(app.t(if app.wizard.hw { "wiz.compare.body_hw" } else { "wiz.compare.body_sw" }));
                    let (next, cancel) = nav_row(app, ui, "btn.begin", true);
                    if next {
                        app.wizard.sub = Sub::Before;
                        app.collector = Some(Collector { start: now, secs: app.diag.test_secs, left: vec![], right: vec![] });
                    }
                    if cancel {
                        abort_if_running(app);
                        reset(app);
                    }
                }
                Sub::Before => {
                    let (el, secs) = app.collector.as_ref().map(|c| ((now - c.start) as f32, c.secs)).unwrap_or((0.0, 1.0));
                    widgets::ring(ui, &t, 120.0, el / secs, &format!("{:.0}", (secs - el).ceil().max(0.0)));
                    ui.label(app.t("diag.rest.body"));
                }
                _ => compare_ui(app, ui, now),
            }
        }
        Step::Decide => decide_ui(app, ui, now, busy),
        Step::Done => done_ui(app, ui, now),
    }
}

fn compare_ui(app: &mut App, ui: &mut Ui, _now: f64) {
    let t = app.tokens;
    let (Some(b), Some(a)) = (app.wizard.before.clone(), app.wizard.after.clone()) else { return };
    let cmp = compare(&b, &a);
    ui.horizontal_top(|ui| {
        for (title, r) in [("wiz.compare.before", &b), ("wiz.compare.after", &a)] {
            ui.vertical(|ui| {
                ui.set_width(150.0);
                ui.label(egui::RichText::new(app.t(title)).strong());
                ui.label(widgets::num(format!("{:.0}", r.worst_score()), 46.0));
                let w = if r.left.score >= r.right.score { r.left } else { r.right };
                let (lvl, key) = super::verdict_info(w.verdict);
                ui.horizontal(|ui| {
                    dot(ui, status_color(&t, lvl), 6.0);
                    ui.label(app.t(key));
                });
            });
        }
    });
    let delta = widgets::anim_value(ui, egui::Id::new("wiz_delta"), cmp.delta, app.reduced());
    ui.label(egui::RichText::new(format!("{:+.0}", -delta)).size(40.0).strong().color(if cmp.improved { c32(t.ok) } else { c32(t.warn) }));
    ui.label(app.t(if cmp.improved { "wiz.compare.improved" } else { "wiz.compare.notimproved" }));
    let (next, cancel) = nav_row(app, ui, "btn.next", true);
    if next {
        app.wizard.step = Step::Decide;
        app.wizard.sub = Sub::Intro;
    }
    if cancel {
        abort_if_running(app);
        reset(app);
    }
}

fn is_improved(app: &App) -> bool {
    match (&app.wizard.before, &app.wizard.after) {
        (Some(b), Some(a)) => compare(b, a).improved,
        _ => false,
    }
}

fn decide_ui(app: &mut App, ui: &mut Ui, now: f64, busy: bool) {
    let t = app.tokens;
    h2(ui, app.t("wiz.decide.title"));
    let improved = is_improved(app);
    ui.label(app.t(if app.wizard.hw { "wiz.decide.body_hw" } else { "wiz.decide.body_sw" }));
    if !improved {
        ui.label(egui::RichText::new(app.t("wiz.decide.warn_noimp")).color(c32(t.warn)).strong());
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        let save_label = if app.wizard.hw { "wiz.decide.save_hw" } else { "wiz.decide.save_sw" };
        if primary(ui, &t, app.t(save_label), !busy).clicked() {
            if app.wizard.hw {
                app.run_job("commit", OpResult::Unit, |d| d.commit_permanent());
            } else {
                save_profile(app, now);
                app.wizard.step = Step::Done;
                app.wizard.done_t = now;
            }
        }
        if ghost(ui, &t, app.t("wiz.decide.discard"), !busy).clicked() {
            if app.wizard.hw {
                let blob = app.wizard.backup_blob.clone();
                app.run_job("abort", OpResult::Unit, move |d| {
                    let r = d.abort();
                    if let Some(b) = blob {
                        let _ = d.restore(&b);
                    }
                    r
                });
                app.toast(now, app.t("wiz.decide.discard_hint"), false);
            }
            reset(app);
        }
        if ghost(ui, &t, app.t("wiz.decide.repeat"), !busy).clicked() {
            let (hw, ack, bp, bb) = (app.wizard.hw, app.wizard.ack, app.wizard.backup_path.clone(), app.wizard.backup_blob.clone());
            reset(app);
            app.wizard.hw = hw;
            app.wizard.ack = ack;
            app.wizard.backup_path = bp;
            app.wizard.backup_blob = bb;
            app.wizard.step = Step::Center;
        }
    });
    if busy {
        widgets::skeleton(ui, &t, vec2(ui.available_width(), 14.0), app.reduced());
    }
    if !app.wizard.hw {
        let _: Option<Circularity> = app.last_circ.as_ref().map(|c| c.0.clone());
        let _ = circularity;
    }
}

fn save_profile(app: &mut App, now: f64) {
    let Some((l, r)) = app.wizard.sw_corr else { return };
    if let Some(p) = crate::app::active_profile_mut(app) {
        *p = Profile { name: format!("Calibrated {}", &storage::now_iso()[..10]), left: l, right: r, system_wide: false };
    }
    let _ = app.profiles.save(&app.data_dir);
    app.wizard.saved_profile = true;
    app.toast(now, app.t("toast.profile_saved"), false);
}

fn done_ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    let improved = is_improved(app);
    h2(ui, app.t(if improved { "wiz.done.good" } else { "wiz.done.honest" }));
    ui.label(app.t(if improved { "wiz.done.good_body" } else { "wiz.done.honest_body" }));
    if app.wizard.hw {
        ui.label(app.t(if app.wizard.committed { "wiz.done.saved_hw" } else { "wiz.done.temp" }));
    }
    if !improved {
        ui.add_space(8.0);
        super::hardware_options(app, ui, &t);
    }
    if improved && !app.reduced() {
        confetti(ui, &t, now - app.wizard.done_t);
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if primary(ui, &t, app.t("btn.finish"), true).clicked() {
            reset(app);
            app.go(Screen::Overview, now);
        }
        if ghost(ui, &t, app.t("wiz.decide.repeat"), true).clicked() {
            reset(app);
        }
    });
}

/// Subtle confetti / glow, only shown for a measurable improvement.
fn confetti(ui: &mut Ui, t: &crate::theme::Tokens, age: f64) {
    if age > 4.0 {
        return;
    }
    let rect = ui.max_rect();
    let fade = (1.0 - (age / 4.0) as f32).clamp(0.0, 1.0);
    for i in 0..36 {
        let seed = i as f32 * 12.9898;
        let x = rect.left() + (seed.sin() * 43758.545).fract().abs() * rect.width();
        let y = rect.top()
            + ((age as f32 * (0.4 + (i % 5) as f32 * 0.1)) * 160.0 + (seed.cos() * 999.0).fract().abs() * 80.0) % rect.height().max(1.0);
        let col = if i % 2 == 0 { t.accent } else { t.ok };
        ui.painter().circle_filled(egui::pos2(x, y), 3.0, c32a(col, (200.0 * fade) as u8));
    }
    ui.ctx().request_repaint();
}
