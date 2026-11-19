//! The DriftLab logo drawn with vector primitives (SVG source: `assets/logo.svg`, geometry shared with `xtask/src/logo.rs`).
//!
//! A stick cap seen from above: rings, a dot that drifted off-centre and the tapering path that spirals back.

use crate::theme::Rgb;
use egui::{pos2, Painter, Rect, Stroke};

pub fn paint(p: &Painter, r: Rect, accent: Rgb, ink: Rgb) {
    let c = r.center();
    let k = r.width().min(r.height()) / 256.0;
    let ink_c = |a: f32| egui::Color32::from_rgba_unmultiplied(ink[0], ink[1], ink[2], (a * 255.0) as u8);
    let acc = egui::Color32::from_rgb(accent[0], accent[1], accent[2]);
    let at = |x: f32, y: f32| pos2(c.x + (x - 128.0) * k, c.y + (y - 128.0) * k);
    p.circle_stroke(c, 96.0 * k, Stroke::new(9.0 * k, ink_c(0.96)));
    p.circle_stroke(c, 70.0 * k, Stroke::new(4.5 * k, ink_c(0.5)));
    // spiral from the dot back to the centre (same parameters as the app icon)
    let n = 48;
    let pts: Vec<(egui::Pos2, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let rad = 54.0 * (1.0 - t).powf(0.85);
            let a = (-40.0 + 300.0 * t).to_radians();
            (at(128.0 + rad * a.cos(), 128.0 + rad * a.sin()), (10.5 + (2.0 - 10.5) * t) * k)
        })
        .collect();
    for w in pts.windows(2) {
        p.line_segment([w[0].0, w[1].0], Stroke::new((w[0].1 + w[1].1) / 2.0, ink_c(0.98)));
        p.circle_filled(w[1].0, w[1].1 / 2.0, ink_c(0.98));
    }
    p.circle_filled(c, 5.5 * k, ink_c(0.95));
    p.circle_filled(pts[0].0, 16.0 * k, ink_c(1.0));
    p.circle_filled(pts[0].0, 10.0 * k, acc);
}
