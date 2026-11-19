use crate::app::App;
use crate::theme::{c32, c32a};
use crate::widgets::{self, card, ghost, h1, primary};
use driftlab_core::Family;
use egui::{pos2, vec2, Rect, Sense, Stroke, Ui};

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    h1(ui, app.t("start.title"));
    widgets::dim(ui, &t, app.t("start.sub"));
    ui.add_space(24.0);
    card(ui, &t, |ui| {
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(360.0, 160.0), Sense::hover());
            paint_cable(ui, rect, now, app.reduced());
            ui.add_space(8.0);
            ui.label(egui::RichText::new(app.t("start.hint")).size(17.0).strong());
            ui.add_space(4.0);
            widgets::dim(ui, &t, app.t("start.usb"));
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() / 2.0 - 190.0).max(0.0));
                if primary(ui, &t, app.t("start.demo"), true).clicked() {
                    app.start_demo(Family::DualSense, now);
                }
                egui::ComboBox::from_id_salt("demo_family").selected_text(family_label(app.demo_family)).show_ui(ui, |ui| {
                    for f in
                        [Family::DualSense, Family::DualSenseEdge, Family::DualShock4, Family::Xbox, Family::XboxElite, Family::Generic]
                    {
                        ui.selectable_value(&mut app.demo_family, f, family_label(f));
                    }
                });
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for f in ["DualSense", "DualSense Edge", "DualShock 4", "Xbox One", "Xbox Series", "Xbox Elite 2"] {
                    widgets::chip(ui, &t, f, false);
                }
            });
        });
    });
    if app.devices.is_empty() && ghost(ui, &t, app.t("start.rescan"), true).clicked() {
        app.toast(now, app.t("start.scanning"), false);
    }
}

pub fn family_label(f: Family) -> &'static str {
    match f {
        Family::DualSense => "DualSense",
        Family::DualSenseEdge => "DualSense Edge",
        Family::DualShock4 => "DualShock 4",
        Family::Xbox => "Xbox Series",
        Family::XboxElite => "Xbox Elite",
        Family::Generic => "Generic",
    }
}

/// Animated cable approaching a controller silhouette.
fn paint_cable(ui: &Ui, rect: Rect, now: f64, reduced: bool) {
    let t = ui.style().visuals.hyperlink_color;
    let text = ui.style().visuals.text_color();
    let p = ui.painter();
    let c = rect.center();
    p.rect_stroke(
        Rect::from_center_size(c + vec2(60.0, 0.0), vec2(150.0, 70.0)),
        egui::CornerRadius::same(30),
        Stroke::new(3.0, text),
        egui::StrokeKind::Inside,
    );
    p.circle_stroke(c + vec2(30.0, 6.0), 12.0, Stroke::new(2.5, text));
    p.circle_stroke(c + vec2(90.0, 6.0), 12.0, Stroke::new(2.5, text));
    let k = if reduced { 1.0 } else { ((now * 0.6).sin() * 0.5 + 0.5) as f32 };
    let plug = c + vec2(-70.0 + k * 40.0, 0.0);
    p.line_segment([rect.left_center(), plug - vec2(20.0, 0.0)], Stroke::new(4.0, text));
    p.rect_filled(Rect::from_center_size(plug, vec2(36.0, 22.0)), egui::CornerRadius::same(5), t);
    p.rect_stroke(
        Rect::from_center_size(c + vec2(-14.0, 0.0), vec2(12.0, 16.0)),
        egui::CornerRadius::same(3),
        Stroke::new(2.0, c32a([128, 128, 128], 200)),
        egui::StrokeKind::Inside,
    );
    let _ = (pos2(0.0, 0.0), c32([0, 0, 0]));
    if !reduced {
        ui.ctx().request_repaint();
    }
}
