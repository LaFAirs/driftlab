//! The DriftLab logo drawn with vector primitives (SVG source: `assets/logo.svg`).

use crate::theme::Rgb;
use egui::{pos2, Painter, Rect, Stroke};

pub fn paint(p: &Painter, r: Rect, accent: Rgb, ink: Rgb) {
    let c = r.center();
    let s = r.width().min(r.height()) / 2.0;
    let ink = egui::Color32::from_rgb(ink[0], ink[1], ink[2]);
    let acc = egui::Color32::from_rgb(accent[0], accent[1], accent[2]);
    p.circle_stroke(c, s * 0.92, Stroke::new(s * 0.16, ink));
    // Crosshair ticks.
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        p.line_segment(
            [pos2(c.x + dx * s * 0.45, c.y + dy * s * 0.45), pos2(c.x + dx * s * 1.05, c.y + dy * s * 1.05)],
            Stroke::new(s * 0.10, ink),
        );
    }
    // Drifting stick dot, slightly off centre.
    p.circle_filled(pos2(c.x + s * 0.18, c.y - s * 0.12), s * 0.30, acc);
}
