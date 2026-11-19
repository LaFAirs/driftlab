//! Design tokens, theme transitions (OKLab interpolation) and WCAG contrast helpers.

use egui::{Color32, CornerRadius, Margin, Stroke};

/// Spacing scale (px) on the 4/8 grid.
#[allow(dead_code)]
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
    pub const HUGE: f32 = 48.0;
}

/// Corner radii.
#[allow(dead_code)]
pub mod radius {
    pub const S: u8 = 8;
    pub const M: u8 = 12;
    pub const L: u8 = 20;
    pub const XL: u8 = 28;
}

/// Animation durations in seconds.
#[allow(dead_code)]
pub mod dur {
    pub const FAST: f32 = 0.12;
    pub const BASE: f32 = 0.24;
    pub const THEME: f32 = 0.5;
    pub const SCREEN: f32 = 0.28;
}

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    pub key: &'static str,
    pub dark: bool,
    pub bg_top: Rgb,
    pub bg_bottom: Rgb,
    pub sidebar: Rgb,
    pub card: Rgb,
    pub card_hover: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub text_dim: Rgb,
    pub accent: Rgb,
    pub on_accent: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub bad: Rgb,
    /// 0..1 strength of accent glows.
    pub glow: f32,
    /// Text colour used on top of `sidebar`.
    pub sidebar_text: Rgb,
}

const fn h(v: u32) -> Rgb {
    [((v >> 16) & 255) as u8, ((v >> 8) & 255) as u8, (v & 255) as u8]
}

fn best_on(bg: Rgb) -> Rgb {
    let (w, b) = (h(0xFFFFFF), h(0x0B0D12));
    if contrast(w, bg) >= contrast(b, bg) {
        w
    } else {
        b
    }
}

impl Tokens {
    pub fn for_key(key: &str) -> Self {
        match key {
            "ps5" => Self::light(
                "ps5",
                h(0xF5F7FA),
                h(0xE9EEF7),
                h(0xFFFFFF),
                h(0x1B2230),
                h(0x566073),
                h(0x2A66F2),
                h(0xFFFFFF),
                h(0x1B2230),
                0.45,
            ),
            "ps5_edge" => {
                let mut t = Self::light(
                    "ps5_edge",
                    h(0xF1F3F7),
                    h(0xE3E8F1),
                    h(0xFFFFFF),
                    h(0x151A24),
                    h(0x566073),
                    h(0x2A66F2),
                    h(0x151A24),
                    h(0xF2F5FA),
                    0.5,
                );
                t.sidebar_text = h(0xF2F5FA);
                t
            }
            "ps4" => Self::dark("ps4", h(0x0B0F17), h(0x0F1624), h(0x141A26), h(0xEEF2F8), h(0x9AA6BA), h(0x1E90FF), 0.9),
            "xbox" | "xbox_elite" => Self::dark("xbox", h(0x0E0F0E), h(0x121612), h(0x181A18), h(0xF0F3F0), h(0x9BA79B), h(0x16C60C), 0.85),
            _ => Self::dark("neutral", h(0x16181B), h(0x1B1E22), h(0x1F2226), h(0xECEEF1), h(0xA0A6AF), h(0xC9CED6), 0.35),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn light(
        key: &'static str,
        bg_top: Rgb,
        bg_bottom: Rgb,
        card: Rgb,
        text: Rgb,
        dim: Rgb,
        accent: Rgb,
        sidebar: Rgb,
        sidebar_text: Rgb,
        glow: f32,
    ) -> Self {
        Self {
            key,
            dark: false,
            bg_top,
            bg_bottom,
            sidebar,
            card,
            card_hover: h(0xF3F6FC),
            border: h(0xDDE3EE),
            text,
            text_dim: dim,
            accent,
            on_accent: best_on(accent),
            ok: h(0x0E7A3E),
            warn: h(0x9A5B00),
            bad: h(0xC62828),
            glow,
            sidebar_text,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dark(key: &'static str, bg_top: Rgb, bg_bottom: Rgb, card: Rgb, text: Rgb, dim: Rgb, accent: Rgb, glow: f32) -> Self {
        Self {
            key,
            dark: true,
            bg_top,
            bg_bottom,
            sidebar: bg_top,
            card,
            card_hover: lerp_rgb(card, accent, 0.08),
            border: lerp_rgb(card, h(0xFFFFFF), 0.10),
            text,
            text_dim: dim,
            accent,
            on_accent: best_on(accent),
            ok: h(0x4CD98A),
            warn: h(0xFFB454),
            bad: h(0xFF6B6B),
            glow,
            sidebar_text: text,
        }
    }

    /// Interpolate every token in OKLab.
    pub fn lerp(&self, o: &Tokens, t: f32) -> Tokens {
        let l = |a: Rgb, b: Rgb| lerp_rgb(a, b, t);
        Tokens {
            key: if t < 0.5 { self.key } else { o.key },
            dark: if t < 0.5 { self.dark } else { o.dark },
            bg_top: l(self.bg_top, o.bg_top),
            bg_bottom: l(self.bg_bottom, o.bg_bottom),
            sidebar: l(self.sidebar, o.sidebar),
            card: l(self.card, o.card),
            card_hover: l(self.card_hover, o.card_hover),
            border: l(self.border, o.border),
            text: l(self.text, o.text),
            text_dim: l(self.text_dim, o.text_dim),
            accent: l(self.accent, o.accent),
            on_accent: l(self.on_accent, o.on_accent),
            ok: l(self.ok, o.ok),
            warn: l(self.warn, o.warn),
            bad: l(self.bad, o.bad),
            glow: self.glow + (o.glow - self.glow) * t,
            sidebar_text: l(self.sidebar_text, o.sidebar_text),
        }
    }
}

pub fn c32(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn c32a(c: Rgb, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a)
}

// ---------- colour science ----------

fn to_linear(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// sRGB → OKLab.
pub fn to_oklab(c: Rgb) -> [f32; 3] {
    let (r, g, b) = (to_linear(c[0]), to_linear(c[1]), to_linear(c[2]));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

pub fn from_oklab(c: [f32; 3]) -> Rgb {
    let l = (c[0] + 0.396_337_78 * c[1] + 0.215_803_76 * c[2]).powi(3);
    let m = (c[0] - 0.105_561_346 * c[1] - 0.063_854_17 * c[2]).powi(3);
    let s = (c[0] - 0.089_484_18 * c[1] - 1.291_485_5 * c[2]).powi(3);
    [
        from_linear(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        from_linear(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s),
        from_linear(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    ]
}

pub fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let (la, lb) = (to_oklab(a), to_oklab(b));
    let t = t.clamp(0.0, 1.0);
    from_oklab([la[0] + (lb[0] - la[0]) * t, la[1] + (lb[1] - la[1]) * t, la[2] + (lb[2] - la[2]) * t])
}

fn luminance(c: Rgb) -> f32 {
    0.2126 * to_linear(c[0]) + 0.7152 * to_linear(c[1]) + 0.0722 * to_linear(c[2])
}

/// WCAG contrast ratio.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

// ---------- easing ----------

pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Damped spring overshoot, 0 → 1.
pub fn spring(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (-6.0 * t).exp() * (12.0 * t).cos()
}

/// Animated theme state.
pub struct ThemeState {
    from: Tokens,
    to: Tokens,
    start: f64,
    pub duration: f32,
}

impl ThemeState {
    pub fn new(t: Tokens) -> Self {
        Self { from: t, to: t, start: 0.0, duration: dur::THEME }
    }

    pub fn set_target(&mut self, now: f64, t: Tokens) {
        if t.key == self.to.key {
            return;
        }
        self.from = self.current(now);
        self.to = t;
        self.start = now;
    }

    pub fn animating(&self, now: f64) -> bool {
        ((now - self.start) as f32) < self.duration && self.from != self.to
    }

    pub fn current(&self, now: f64) -> Tokens {
        if self.duration <= 0.0 {
            return self.to;
        }
        let t = ease_in_out(((now - self.start) as f32) / self.duration);
        if t >= 1.0 {
            self.to
        } else {
            self.from.lerp(&self.to, t)
        }
    }
}

/// Push tokens into egui's style.
pub fn apply(ctx: &egui::Context, t: &Tokens, scale: f32) {
    let mut v = if t.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = c32(t.bg_top);
    v.window_fill = c32(t.card);
    v.extreme_bg_color = c32(t.card);
    v.faint_bg_color = c32(t.card_hover);
    v.override_text_color = Some(c32(t.text));
    v.hyperlink_color = c32(t.accent);
    v.selection.bg_fill = c32a(t.accent, 90);
    v.selection.stroke = Stroke::new(1.0, c32(t.accent));
    let r = CornerRadius::same(radius::S);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c32(t.border));
    v.widgets.noninteractive.bg_fill = c32(t.card);
    // slider rails / checkbox frames use bg_fill: keep them clearly visible on every theme
    v.widgets.inactive.bg_fill = c32(lerp_rgb(t.card, t.text, 0.22));
    v.slider_trailing_fill = true;
    v.widgets.inactive.weak_bg_fill = c32(lerp_rgb(t.card, t.border, 0.5));
    v.widgets.hovered.bg_fill = c32(t.card_hover);
    v.widgets.hovered.weak_bg_fill = c32(t.card_hover);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, c32(t.accent));
    v.widgets.active.bg_fill = c32(t.accent);
    v.widgets.active.weak_bg_fill = c32(t.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.5, c32(t.text));
    v.window_corner_radius = CornerRadius::same(radius::L);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(space::S, space::S);
        s.spacing.button_padding = egui::vec2(space::L, space::S);
        s.spacing.window_margin = Margin::same(space::L as i8);
        s.interaction.tooltip_delay = 0.2;
    });
    ctx.set_zoom_factor(scale.clamp(0.8, 2.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [&str; 5] = ["ps5", "ps5_edge", "ps4", "xbox", "neutral"];

    #[test]
    fn wcag_aa_in_all_themes() {
        for k in KEYS {
            let t = Tokens::for_key(k);
            for (n, fg, bg) in [
                ("text/bg", t.text, t.bg_top),
                ("text/bg_bottom", t.text, t.bg_bottom),
                ("text/card", t.text, t.card),
                ("dim/card", t.text_dim, t.card),
                ("dim/bg", t.text_dim, t.bg_top),
                ("on_accent/accent", t.on_accent, t.accent),
                ("sidebar", t.sidebar_text, t.sidebar),
                ("accent/card (UI)", t.accent, t.card),
                ("ok/card", t.ok, t.card),
                ("warn/card", t.warn, t.card),
                ("bad/card", t.bad, t.card),
            ] {
                let min = if n.contains("(UI)") { 3.0 } else { 4.5 };
                assert!(contrast(fg, bg) >= min, "{k} {n}: {:.2}", contrast(fg, bg));
            }
        }
    }

    /// Snapshot of the key design tokens – update deliberately when restyling.
    #[test]
    fn token_snapshot() {
        let hex = |c: Rgb| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
        let snap: Vec<String> = KEYS
            .iter()
            .map(|k| {
                let t = Tokens::for_key(k);
                format!("{k}: bg={} card={} accent={} text={} dark={}", hex(t.bg_top), hex(t.card), hex(t.accent), hex(t.text), t.dark)
            })
            .collect();
        assert_eq!(
            snap,
            vec![
                "ps5: bg=#F5F7FA card=#FFFFFF accent=#2A66F2 text=#1B2230 dark=false",
                "ps5_edge: bg=#F1F3F7 card=#FFFFFF accent=#2A66F2 text=#151A24 dark=false",
                "ps4: bg=#0B0F17 card=#141A26 accent=#1E90FF text=#EEF2F8 dark=true",
                "xbox: bg=#0E0F0E card=#181A18 accent=#16C60C text=#F0F3F0 dark=true",
                "neutral: bg=#16181B card=#1F2226 accent=#C9CED6 text=#ECEEF1 dark=true",
            ]
        );
    }

    #[test]
    fn oklab_roundtrip_and_interpolation() {
        for c in [[0u8, 0, 0], [255, 255, 255], [42, 102, 242], [22, 198, 12]] {
            let r = from_oklab(to_oklab(c));
            for i in 0..3 {
                assert!((i32::from(r[i]) - i32::from(c[i])).abs() <= 1, "{c:?} → {r:?}");
            }
        }
        assert_eq!(lerp_rgb([10, 20, 30], [200, 100, 50], 0.0), [10, 20, 30]);
        let (a, b) = (Tokens::for_key("ps5"), Tokens::for_key("xbox"));
        assert_eq!(a.lerp(&b, 1.0).accent, b.accent);
        assert_eq!(a.lerp(&b, 0.0).bg_top, a.bg_top);
    }

    #[test]
    fn theme_transition_is_500ms() {
        let mut s = ThemeState::new(Tokens::for_key("neutral"));
        s.set_target(10.0, Tokens::for_key("ps4"));
        assert!(s.animating(10.2));
        assert!(!s.animating(10.6));
        assert_eq!(s.current(10.6).accent, Tokens::for_key("ps4").accent);
        assert_ne!(s.current(10.25).accent, Tokens::for_key("ps4").accent);
    }
}
