//! Reusable premium-feeling widgets: cards, buttons, chips, stick panels, plots.

use crate::theme::{self, c32, c32a, radius, space, Tokens};
use egui::{
    epaint::Shadow, pos2, vec2, Align2, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

/// Card with soft multi-layer shadow. Hover lifts it slightly when `interactive`.
pub fn card<R>(ui: &mut Ui, t: &Tokens, add: impl FnOnce(&mut Ui) -> R) -> R {
    let inner_w = (ui.available_width() - 2.0 * space::XL - 2.0).max(100.0);
    Frame::new()
        .fill(c32(t.card))
        .stroke(Stroke::new(1.0, c32(t.border)))
        .corner_radius(CornerRadius::same(radius::L))
        .inner_margin(Margin::same(space::XL as i8))
        .shadow(Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(if t.dark { 90 } else { 22 }) })
        .show(ui, |ui| {
            ui.set_width(inner_w);
            add(ui)
        })
        .inner
}

/// Section title.
pub fn h1(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(28.0).strong());
}

pub fn h2(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(18.0).strong());
}

pub fn dim(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.label(egui::RichText::new(text).color(c32(t.text_dim)));
}

/// Primary (accent) button with hover lift and press feedback.
pub fn primary(ui: &mut Ui, t: &Tokens, text: &str, enabled: bool) -> Response {
    button_impl(ui, t, text, true, enabled)
}

pub fn ghost(ui: &mut Ui, t: &Tokens, text: &str, enabled: bool) -> Response {
    button_impl(ui, t, text, false, enabled)
}

fn button_impl(ui: &mut Ui, t: &Tokens, text: &str, primary: bool, enabled: bool) -> Response {
    let font = FontId::proportional(15.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let size = vec2(galley.size().x + space::XL * 2.0, 44.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() && enabled, theme::dur::FAST);
    let press = ui.ctx().animate_bool_with_time(resp.id.with("p"), resp.is_pointer_button_down_on() && enabled, 0.06);
    let rect = rect.translate(vec2(0.0, -2.0 * hover + 2.0 * press));
    let (bg, fg, stroke) = if primary {
        (theme::lerp_rgb(t.accent, if t.dark { [255, 255, 255] } else { [0, 0, 0] }, 0.08 * hover), t.on_accent, Stroke::NONE)
    } else {
        (theme::lerp_rgb(t.card, t.card_hover, hover), t.text, Stroke::new(1.0, c32(theme::lerp_rgb(t.border, t.accent, hover))))
    };
    let alpha = if enabled { 255 } else { 110 };
    if primary && enabled && t.glow > 0.0 {
        ui.painter().rect_filled(
            rect.expand(2.0 + 4.0 * hover),
            CornerRadius::same(radius::M + 4),
            c32a(t.accent, (28.0 * t.glow * (0.5 + hover)) as u8),
        );
    }
    ui.painter().rect_filled(rect, CornerRadius::same(radius::M), c32a(bg, alpha));
    if stroke.width > 0.0 {
        ui.painter().rect_stroke(rect, CornerRadius::same(radius::M), stroke, StrokeKind::Inside);
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(3.0), CornerRadius::same(radius::M + 3), Stroke::new(2.0, c32(t.accent)), StrokeKind::Outside);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(15.0), c32a(fg, alpha));
    resp
}

/// Small pill label.
pub fn chip(ui: &mut Ui, t: &Tokens, text: &str, on: bool) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.0), Color32::WHITE);
    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 20.0, 26.0), Sense::hover());
    let (bg, fg) = if on {
        (c32a(t.accent, 40), c32(if t.dark { t.accent } else { theme::lerp_rgb(t.accent, [0, 0, 0], 0.25) }))
    } else {
        (c32(theme::lerp_rgb(t.card, t.border, 0.5)), c32(t.text_dim))
    };
    ui.painter().rect_filled(rect, CornerRadius::same(13), bg);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(13.0), fg);
}

pub fn status_color(t: &Tokens, level: u8) -> Color32 {
    c32(match level {
        0 => t.ok,
        1 => t.warn,
        _ => t.bad,
    })
}

/// Traffic-light dot.
pub fn dot(ui: &mut Ui, color: Color32, r: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(r * 2.0 + 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), r, color);
}

/// Skeleton placeholder bar with shimmer.
pub fn skeleton(ui: &mut Ui, t: &Tokens, size: Vec2, reduced: bool) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(radius::S), c32(theme::lerp_rgb(t.card, t.border, 0.6)));
    if !reduced {
        let time = ui.input(|i| i.time) as f32;
        let x = rect.left() + ((time * 0.8).fract()) * (rect.width() + 80.0) - 40.0;
        let band = Rect::from_min_max(pos2(x.max(rect.left()), rect.top()), pos2((x + 40.0).min(rect.right()), rect.bottom()));
        if band.width() > 0.0 {
            ui.painter().rect_filled(band, CornerRadius::same(radius::S), c32a(t.accent, 14));
        }
        ui.ctx().request_repaint();
    }
}

/// Tabular numeric text (monospace digits).
pub fn num(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into()).size(size).family(egui::FontFamily::Name("num".into()))
}

/// Animated float display.
pub fn anim_value(ui: &Ui, id: egui::Id, target: f32, reduced: bool) -> f32 {
    if reduced {
        target
    } else {
        ui.ctx().animate_value_with_time(id, target, 0.6)
    }
}

// ---------- Stick panel ----------

pub struct StickPanelData<'a> {
    pub pos: (f32, f32),
    pub trail: &'a [(f32, f32)],
    pub heat: Option<&'a [u16]>,
    pub deadzone: f32,
    pub circle_samples: &'a [(f32, f32)],
}

pub const HEAT_N: usize = 32;

/// Draw a circular stick panel with crosshair, trail, heatmap and live position.
pub fn stick_panel(ui: &mut Ui, t: &Tokens, size: f32, d: &StickPanelData) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let r = size / 2.0 - 6.0;
    let p = ui.painter();
    p.circle_filled(c, r, c32(theme::lerp_rgb(t.card, t.bg_top, 0.4)));
    // heatmap
    if let Some(h) = d.heat {
        let max = h.iter().copied().max().unwrap_or(1).max(1) as f32;
        let cell = 2.0 * r / HEAT_N as f32;
        for gy in 0..HEAT_N {
            for gx in 0..HEAT_N {
                let v = h[gy * HEAT_N + gx];
                if v == 0 {
                    continue;
                }
                let a = ((v as f32 / max).sqrt() * 170.0) as u8;
                let cr = Rect::from_min_size(pos2(c.x - r + gx as f32 * cell, c.y - r + gy as f32 * cell), Vec2::splat(cell));
                p.rect_filled(cr, CornerRadius::ZERO, c32a(t.accent, a));
            }
        }
    }
    p.circle_stroke(c, r, Stroke::new(2.0, c32(t.border)));
    for f in [0.25, 0.5, 0.75] {
        p.circle_stroke(c, r * f, Stroke::new(1.0, c32a(t.border, 150)));
    }
    p.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], Stroke::new(1.0, c32a(t.text_dim, 90)));
    p.line_segment([pos2(c.x, c.y - r), pos2(c.x, c.y + r)], Stroke::new(1.0, c32a(t.text_dim, 90)));
    if d.deadzone > 0.0 {
        p.circle_stroke(c, r * d.deadzone, Stroke::new(1.5, c32a(t.warn, 200)));
    }
    let to = |(x, y): (f32, f32)| pos2(c.x + x.clamp(-1.0, 1.0) * r, c.y - y.clamp(-1.0, 1.0) * r);
    // circularity polygon
    if d.circle_samples.len() > 2 {
        let pts: Vec<Pos2> = d.circle_samples.iter().map(|&s| to(s)).collect();
        p.add(egui::Shape::closed_line(pts, Stroke::new(2.0, c32(t.accent))));
    }
    // trail
    let n = d.trail.len();
    for (i, w) in d.trail.windows(2).enumerate() {
        let a = (i as f32 / n.max(1) as f32 * 200.0) as u8;
        p.line_segment([to(w[0]), to(w[1])], Stroke::new(2.0, c32a(t.accent, a)));
    }
    let pt = to(d.pos);
    if t.glow > 0.0 {
        p.circle_filled(pt, 16.0, c32a(t.accent, (40.0 * t.glow) as u8));
    }
    p.circle_filled(pt, 7.0, c32(t.accent));
    p.circle_stroke(pt, 7.0, Stroke::new(2.0, c32(t.card)));
    rect
}

/// Simple line plot for time series; `series` = (values, colour).
pub fn line_plot(ui: &mut Ui, t: &Tokens, size: Vec2, series: &[(&[f32], Color32)], range: (f32, f32)) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(radius::S), c32(theme::lerp_rgb(t.card, t.bg_top, 0.4)));
    let zero_y = rect.bottom() - (0.0 - range.0) / (range.1 - range.0) * rect.height();
    p.line_segment([pos2(rect.left(), zero_y), pos2(rect.right(), zero_y)], Stroke::new(1.0, c32a(t.border, 200)));
    for (vals, col) in series {
        if vals.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = vals
            .iter()
            .enumerate()
            .map(|(i, v)| {
                pos2(
                    rect.left() + i as f32 / (vals.len() - 1) as f32 * rect.width(),
                    rect.bottom() - ((v - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) * rect.height(),
                )
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(1.6, *col)));
    }
}

/// Circular progress ring with centre text.
pub fn ring(ui: &mut Ui, t: &Tokens, size: f32, frac: f32, label: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let r = size / 2.0 - 8.0;
    let p = ui.painter();
    p.circle_stroke(c, r, Stroke::new(8.0, c32(t.border)));
    let n = 96;
    let steps = (frac.clamp(0.0, 1.0) * n as f32) as usize;
    if steps > 0 {
        let pts: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
                pos2(c.x + r * a.cos(), c.y + r * a.sin())
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(8.0, c32(t.accent))));
    }
    p.text(c, Align2::CENTER_CENTER, label, FontId::proportional(size * 0.28), c32(t.text));
}

/// Horizontal progress bar.
pub fn progress_bar(ui: &mut Ui, t: &Tokens, width: f32, frac: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), c32(t.border));
    let mut f = rect;
    f.set_width(rect.width() * frac.clamp(0.0, 1.0));
    ui.painter().rect_filled(f, CornerRadius::same(4), c32(t.accent));
}

/// Coverage segments ring (72 segments filled).
pub fn coverage_ring(ui: &mut Ui, t: &Tokens, size: f32, map: &driftlab_core::math::EdgeMap, live: (f32, f32)) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let r = size / 2.0 - 10.0;
    let p = ui.painter();
    p.circle_filled(c, r, c32(theme::lerp_rgb(t.card, t.bg_top, 0.4)));
    let n = driftlab_core::math::SEGMENTS;
    for (i, &rad) in map.radius.iter().enumerate().take(n) {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (i as f32 + 0.85) / n as f32 * std::f32::consts::TAU;
        let filled = rad >= 0.5;
        let col = if filled { c32(t.accent) } else { c32(t.border) };
        let pts = vec![pos2(c.x + r * a0.cos(), c.y - r * a0.sin()), pos2(c.x + r * a1.cos(), c.y - r * a1.sin())];
        p.add(egui::Shape::line(pts, Stroke::new(8.0, col)));
    }
    let pt = pos2(c.x + live.0.clamp(-1.0, 1.0) * r * 0.9, c.y - live.1.clamp(-1.0, 1.0) * r * 0.9);
    p.circle_filled(pt, 6.0, c32(t.text));
}

/// Circularity plot: measured polygon vs ideal circle; weakest segment marked.
pub fn circularity_plot(ui: &mut Ui, t: &Tokens, size: f32, radii: &[f32], weakest: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let r = size / 2.0 - 12.0;
    let p = ui.painter();
    p.circle_filled(c, r, c32(theme::lerp_rgb(t.card, t.bg_top, 0.4)));
    p.circle_stroke(c, r, Stroke::new(1.5, c32a(t.text_dim, 160)));
    let n = radii.len();
    if n > 2 {
        let pts: Vec<Pos2> = (0..n)
            .map(|i| {
                let a = (i as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
                pos2(c.x + r * radii[i] * a.cos(), c.y - r * radii[i] * a.sin())
            })
            .collect();
        p.add(egui::Shape::closed_line(pts, Stroke::new(2.5, c32(t.accent))));
        let a = (weakest as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
        let rr = r * radii[weakest];
        p.circle_filled(pos2(c.x + rr * a.cos(), c.y - rr * a.sin()), 6.0, c32(t.bad));
    }
}
