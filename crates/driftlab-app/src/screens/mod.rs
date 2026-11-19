pub mod backups;
pub mod diagnose;
pub mod overview;
pub mod settings;
pub mod start;
pub mod test;
pub mod tuning;
pub mod wizard;

use crate::app::{App, Screen};
use crate::theme::{c32, Tokens};
use driftlab_core::math::DriftVerdict;
use egui::{vec2, Ui};

pub fn parse_screen(s: &str) -> Screen {
    match s {
        "overview" => Screen::Overview,
        "diagnose" => Screen::Diagnose,
        "wizard" => Screen::Wizard,
        "tuning" => Screen::Tuning,
        "test" => Screen::Test,
        "backups" => Screen::Backups,
        "settings" => Screen::Settings,
        _ => Screen::Start,
    }
}

/// Traffic-light level (0 ok, 1 warn, 2 bad) and translation key for a verdict.
pub fn verdict_info(v: DriftVerdict) -> (u8, &'static str) {
    match v {
        DriftVerdict::Ok => (0, "verdict.ok"),
        DriftVerdict::Slight => (1, "verdict.slight"),
        DriftVerdict::Strong => (2, "verdict.strong"),
        DriftVerdict::HardwareWear => (2, "verdict.wear"),
    }
}

/// Lay out two columns with a given left share of the available width.
pub fn two_cols(ui: &mut Ui, ratio: f32, f: impl FnOnce(&mut Ui, &mut Ui)) {
    let gap = 24.0;
    let total = ui.available_width();
    let lw = ((total - gap) * ratio).max(200.0);
    let rw = (total - gap - lw).max(200.0);
    let top = ui.cursor().min;
    let lrect = egui::Rect::from_min_size(top, vec2(lw, 10_000.0));
    let rrect = egui::Rect::from_min_size(top + vec2(lw + gap, 0.0), vec2(rw, 10_000.0));
    let mut l = ui.new_child(egui::UiBuilder::new().id_salt("col_l").max_rect(lrect));
    let mut r = ui.new_child(egui::UiBuilder::new().id_salt("col_r").max_rect(rrect));
    f(&mut l, &mut r);
    let h = l.min_rect().height().max(r.min_rect().height());
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(top, vec2(total, h)));
}

/// Honest hardware-defect advice block.
pub fn hardware_options(app: &App, ui: &mut Ui, t: &Tokens) {
    ui.label(egui::RichText::new(app.t("wear.title")).color(c32(t.bad)).strong());
    ui.label(app.t("wear.body"));
    for k in ["wear.o1", "wear.o2", "wear.o3", "wear.o4"] {
        ui.label(format!("•  {}", app.t(k)));
    }
}

/// Drive the demo controller's simulated sticks according to what the UI is asking the "user" to do.
pub fn drive_demo(app: &mut App, now: f64) {
    let want_circle = diagnose::wants_circle(app) || wizard::wants_circle(app);
    let want_rest = diagnose::wants_rest(app) || wizard::wants_rest(app);
    let screen = app.screen;
    let tester_ramp = test::wants_trigger_ramp(app);
    let Some(d) = app.dev() else { return };
    let Some(ctl) = &d.demo else { return };
    let mut c = ctl.lock();
    let t = now as f32;
    if want_rest {
        c.left = (0.0, 0.0);
        c.right = (0.0, 0.0);
        c.pressed.clear();
        c.l2 = 0.0;
        c.r2 = 0.0;
    } else if want_circle {
        let a = t * 2.4;
        c.left = (a.cos(), a.sin());
        c.right = ((-a).cos(), (-a).sin());
    } else if tester_ramp {
        let k = ((t * 0.5).fract() * 2.0 - 1.0).abs();
        c.l2 = 1.0 - k;
        c.r2 = 1.0 - k;
    } else if matches!(screen, Screen::Overview | Screen::Test | Screen::Tuning | Screen::Diagnose) {
        let a = t * 0.9;
        c.left = (0.7 * a.cos(), 0.7 * a.sin());
        c.right = (0.5 * (a * 1.3).sin(), 0.5 * (a * 0.8).cos());
        let names = ["cross", "circle", "square", "triangle", "l1", "r1", "up", "right"];
        let i = ((t * 1.2) as usize) % names.len();
        c.pressed = vec![names[i].to_string()];
        c.l2 = ((t * 0.7).sin() * 0.5 + 0.5).max(0.0);
        c.r2 = ((t * 0.5).cos() * 0.5 + 0.5).max(0.0);
    } else {
        c.left = (0.0, 0.0);
        c.right = (0.0, 0.0);
        c.pressed.clear();
    }
}
