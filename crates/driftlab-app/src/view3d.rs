//! 3D controller view (wgpu offscreen → egui image) with a vector 2D fallback.

use crate::app::App;
use crate::theme::{self, c32, c32a, Tokens};
use driftlab_core::{Family, InputState};
use driftlab_render::{model::ModelKind, Palette, Pose, Renderer};
use egui::{pos2, vec2, Color32, CornerRadius, Painter, Rect, Sense, Stroke, TextureId, Ui, Vec2};

pub struct ViewState {
    rs: Option<eframe::egui_wgpu::RenderState>,
    renderer: Option<Renderer>,
    tex: Option<TextureId>,
    gen: u64,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub force_2d: bool,
    pub gyro_tilt: bool,
    last_interact: f64,
    spawn: f64,
    pub tilt: (f32, f32),
}

fn kind_of(f: Family) -> ModelKind {
    match f {
        Family::DualSense => ModelKind::DualSense,
        Family::DualSenseEdge => ModelKind::DualSenseEdge,
        Family::DualShock4 => ModelKind::DualShock4,
        Family::Xbox => ModelKind::Xbox,
        Family::XboxElite => ModelKind::XboxElite,
        Family::Generic => ModelKind::Generic,
    }
}

impl ViewState {
    pub fn new(rs: Option<eframe::egui_wgpu::RenderState>, force_2d: bool) -> Self {
        let renderer = rs.as_ref().map(|r| Renderer::new(r.device.clone(), r.queue.clone()));
        Self {
            rs,
            renderer,
            tex: None,
            gen: u64::MAX,
            yaw: 0.0,
            pitch: 0.55,
            zoom: 1.0,
            force_2d,
            gyro_tilt: true,
            last_interact: -100.0,
            spawn: -100.0,
            tilt: (0.0, 0.0),
        }
    }

    pub fn reset_for_new_device(&mut self, now: f64) {
        self.spawn = now;
        self.yaw = 0.0;
        self.pitch = 0.55;
        self.zoom = 1.0;
    }

    pub fn uses_3d(&self) -> bool {
        !self.force_2d && self.renderer.is_some()
    }
}

/// Which stage of the welcome animation we are in (0..1), if any.
pub fn welcome_progress(app: &App, now: f64) -> Option<f32> {
    app.welcome.as_ref().map(|w| ((now - w.start) / 1.9) as f32).filter(|p| *p < 1.0)
}

/// Draw the live controller of the selected device.
pub fn show(app: &mut App, ui: &mut Ui, size: Vec2, now: f64, interactive: bool) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let Some(dev) = app.dev() else { return };
    let family = dev.info.family;
    let input = dev.input.clone();
    let t = app.tokens;
    let reduced = app.settings.reduced_motion;
    let wp = welcome_progress(app, now);

    if resp.clicked() && wp.is_some() {
        app.welcome = None; // skip
    }
    if interactive {
        let v = &mut app.view;
        if resp.dragged() {
            let d = resp.drag_delta();
            v.yaw += d.x * 0.01;
            v.pitch = (v.pitch + d.y * 0.01).clamp(0.05, 1.45);
            v.last_interact = now;
        }
        if resp.hovered() {
            let s = ui.input(|i| i.smooth_scroll_delta.y);
            if s != 0.0 {
                v.zoom = (v.zoom * (1.0 + s * 0.002)).clamp(0.5, 2.5);
                v.last_interact = now;
            }
        }
        if resp.double_clicked() {
            v.yaw = 0.0;
            v.pitch = 0.55;
            v.zoom = 1.0;
            v.last_interact = now;
        }
    }
    let v = &mut app.view;
    // Idle spin / float.
    let idle = !reduced && now - v.last_interact > 3.0 && wp.is_none();
    if idle {
        v.yaw = ease_yaw(v.yaw, (now as f32 * 0.35).sin() * 0.45, 0.02);
    }
    // Gyro tilt (smoothed).
    let target_tilt = match (&input.gyro, v.gyro_tilt) {
        (Some(_), true) => match &input.accel {
            Some(a) => ((a[2].clamp(-1.0, 1.0) * 0.3), (-a[0].clamp(-1.0, 1.0) * 0.3)),
            None => (0.0, 0.0),
        },
        _ => (0.0, 0.0),
    };
    v.tilt = (v.tilt.0 + (target_tilt.0 - v.tilt.0) * 0.15, v.tilt.1 + (target_tilt.1 - v.tilt.1) * 0.15);

    if v.uses_3d() {
        let key = family.theme_key();
        let mut pal = Palette::for_theme(key);
        // Follow the animated accent so the model blends with the UI during theme transitions.
        pal.accent = [f32::from(t.accent[0]) / 255.0, f32::from(t.accent[1]) / 255.0, f32::from(t.accent[2]) / 255.0];
        let (offset_y, scale, sweep) = match wp {
            Some(p) if !reduced => {
                let k = theme::spring((p / 0.55).min(1.0));
                ((1.0 - k) * 2.2, 0.55 + 0.45 * k.min(1.1), ((p - 0.25) / 0.6).clamp(0.0, 1.0))
            }
            _ => (if idle { (now as f32 * 1.1).sin() * 0.05 } else { 0.0 }, 1.0, -1.0),
        };
        let pose = Pose {
            left: input.left,
            right: input.right,
            l2: input.l2,
            r2: input.r2,
            pressed: input.buttons.iter().filter(|(_, p)| *p).map(|(n, _)| n.clone()).collect(),
            tilt: v.tilt,
            touch: input.touch.clone(),
            yaw: v.yaw,
            pitch: v.pitch,
            zoom: v.zoom,
            offset_y,
            scale,
            sweep,
            palette: pal,
            lightbar: lightbar_color(&t, family),
        };
        let ppp = ui.ctx().pixels_per_point();
        let px = ((rect.width() * ppp) as u32, (rect.height() * ppp) as u32);
        if let (Some(r), Some(rs)) = (v.renderer.as_mut(), v.rs.as_ref()) {
            let recreated = r.render(kind_of(family), &pose, px);
            if let Some(view) = r.view() {
                let mut rr = rs.renderer.write();
                match v.tex {
                    None => {
                        v.tex = Some(rr.register_native_texture(&rs.device, view, eframe::wgpu::FilterMode::Linear));
                        v.gen = r.generation;
                    }
                    Some(id) if recreated || v.gen != r.generation => {
                        rr.update_egui_texture_from_wgpu_texture(&rs.device, view, eframe::wgpu::FilterMode::Linear, id);
                        v.gen = r.generation;
                    }
                    _ => {}
                }
            }
        }
        // Soft accent glow behind the model.
        let c = rect.center() + vec2(0.0, rect.height() * 0.12);
        for k in 0..10 {
            let f = 1.0 - k as f32 / 10.0;
            ui.painter().circle_filled(
                c,
                rect.width().min(rect.height()) * 0.5 * f,
                c32a(t.accent, (3.0 + 5.0 * t.glow * (1.0 - f)) as u8),
            );
        }
        if let Some(id) = v.tex {
            ui.painter().image(id, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    } else {
        draw_2d(ui.painter(), rect, family, &input, &t);
    }

    if let (Some(p), Some(w)) = (wp, &app.welcome) {
        let a = ((1.0 - ((p - 0.75) / 0.25).clamp(0.0, 1.0)) * ((p / 0.2).min(1.0)) * 255.0) as u8;
        let bat = w.battery.map(|b| format!(" · {} %", b.percent)).unwrap_or_default();
        let conn = match w.info.connection {
            driftlab_core::Connection::Usb => "USB",
            driftlab_core::Connection::Bluetooth => "Bluetooth",
            _ => "Demo",
        };
        let txt = format!(
            "{} {} · {conn}{bat}{}",
            w.info.name,
            crate::i18n::tr(app.lang(), "welcome.detected"),
            w.info.firmware.as_ref().map(|f| format!(" · FW {f}")).unwrap_or_default()
        );
        let pill = Rect::from_center_size(
            pos2(rect.center().x, rect.bottom() - 28.0),
            vec2((txt.chars().count() as f32 * 7.8 + 36.0).min(rect.width() - 16.0), 38.0),
        );
        ui.painter().rect_filled(pill, CornerRadius::same(19), Color32::from_black_alpha((a as f32 * 0.55) as u8));
        ui.painter().text(pill.center(), egui::Align2::CENTER_CENTER, txt, egui::FontId::proportional(14.0), Color32::from_white_alpha(a));
        ui.ctx().request_repaint();
    }
}

fn ease_yaw(cur: f32, target: f32, k: f32) -> f32 {
    cur + (target - cur) * k
}

fn lightbar_color(t: &Tokens, f: Family) -> [f32; 3] {
    match f {
        Family::DualShock4 | Family::DualSense | Family::DualSenseEdge => {
            [f32::from(t.accent[0]) / 255.0, f32::from(t.accent[1]) / 255.0, f32::from(t.accent[2]) / 255.0]
        }
        _ => [0.1, 0.8, 0.2],
    }
}

/// Small controller silhouette for sidebar rows.
pub fn mini_icon(p: &Painter, r: Rect, f: Family, t: &Tokens, a: f32) {
    let body = match f {
        Family::DualSense | Family::DualSenseEdge => Color32::from_rgb(235, 238, 244),
        _ => Color32::from_rgb(40, 44, 52),
    };
    let alpha = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (255.0 * a) as u8);
    p.rect_filled(r, CornerRadius::same(10), alpha(body));
    p.rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, c32a(t.accent, (160.0 * a) as u8)), egui::StrokeKind::Inside);
    let c = r.center();
    for s in [-1.0f32, 1.0] {
        p.circle_filled(pos2(c.x + s * 8.0, c.y + 2.0), 3.2, c32a(t.accent, (230.0 * a) as u8));
    }
}

/// 2D vector rendering of a controller with the same live animation as the 3D model.
pub fn draw_2d(p: &Painter, rect: Rect, f: Family, st: &InputState, t: &Tokens) {
    let c = rect.center();
    let s = (rect.width() / 2.6).min(rect.height() / 1.6);
    let ps_white = matches!(f, Family::DualSense | Family::DualSenseEdge);
    let body = if ps_white {
        Color32::from_rgb(236, 239, 244)
    } else if f == Family::DualShock4 {
        Color32::from_rgb(32, 36, 44)
    } else {
        Color32::from_rgb(28, 30, 28)
    };
    let dark = Color32::from_rgb(24, 26, 32);
    let on = |n: &str| st.button(n);
    // shadow
    p.circle_filled(c + vec2(0.0, s * 0.62), s * 0.9, Color32::from_black_alpha(26));
    // grips & body
    for sx in [-1.0f32, 1.0] {
        p.circle_filled(c + vec2(sx * s * 0.78, s * 0.28), s * 0.36, body);
    }
    p.rect_filled(Rect::from_center_size(c, vec2(s * 1.9, s * 0.78)), CornerRadius::same((s * 0.34) as u8), body);
    // triggers / bumpers
    for (sx, v, b) in [(-1.0f32, st.l2, "l1"), (1.0, st.r2, "r1")] {
        let pos = c + vec2(sx * s * 0.62, -s * 0.44);
        p.rect_filled(
            Rect::from_center_size(pos + vec2(0.0, -s * 0.06 + v * s * 0.08), vec2(s * 0.5, s * 0.14)),
            CornerRadius::same(6),
            if v > 0.05 { c32(t.accent) } else { dark },
        );
        p.rect_filled(
            Rect::from_center_size(pos + vec2(0.0, s * 0.08), vec2(s * 0.5, s * 0.07)),
            CornerRadius::same(4),
            if on(b) { c32(t.accent) } else { dark },
        );
    }
    // sticks
    let (lc, rc) = if matches!(f, Family::Xbox | Family::XboxElite) {
        (c + vec2(-s * 0.55, -s * 0.12), c + vec2(s * 0.2, s * 0.2))
    } else {
        (c + vec2(-s * 0.3, s * 0.12), c + vec2(s * 0.3, s * 0.12))
    };
    for (sc, v) in [(lc, st.left), (rc, st.right)] {
        p.circle_filled(sc, s * 0.2, dark);
        p.circle_filled(
            sc + vec2(v.0, -v.1) * s * 0.1,
            s * 0.15,
            if v.0.abs() + v.1.abs() > 0.05 { c32(t.accent) } else { Color32::from_rgb(70, 74, 84) },
        );
    }
    // d-pad and face buttons
    let (dp, fb) = if matches!(f, Family::Xbox | Family::XboxElite) {
        (c + vec2(-s * 0.28, s * 0.2), c + vec2(s * 0.62, -s * 0.08))
    } else {
        (c + vec2(-s * 0.62, -s * 0.08), c + vec2(s * 0.62, -s * 0.08))
    };
    for (n, d) in [("up", vec2(0.0, -1.0)), ("down", vec2(0.0, 1.0)), ("left", vec2(-1.0, 0.0)), ("right", vec2(1.0, 0.0))] {
        p.circle_filled(dp + d * s * 0.1, s * 0.05, if on(n) { c32(t.accent) } else { dark });
    }
    let faces: [(&[&str], Vec2); 4] = [
        (&["triangle", "y"], vec2(0.0, -1.0)),
        (&["cross", "a"], vec2(0.0, 1.0)),
        (&["square", "x"], vec2(-1.0, 0.0)),
        (&["circle", "b"], vec2(1.0, 0.0)),
    ];
    for (names, d) in faces {
        p.circle_filled(fb + d * s * 0.1, s * 0.05, if names.iter().any(|n| on(n)) { c32(t.accent) } else { dark });
    }
    if !matches!(f, Family::Xbox | Family::XboxElite | Family::Generic) {
        p.rect_filled(
            Rect::from_center_size(c + vec2(0.0, -s * 0.18), vec2(s * 0.5, s * 0.22)),
            CornerRadius::same(6),
            if on("touchpad") { c32(t.accent) } else { Color32::from_rgb(60, 64, 74) },
        );
        p.rect_filled(Rect::from_center_size(c + vec2(0.0, -s * 0.34), vec2(s * 0.4, 3.0)), CornerRadius::same(2), c32(t.accent));
    }
}
