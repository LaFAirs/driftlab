//! Controller models: named, animatable parts built procedurally. Original stylised designs – no
//! brand logos, no official assets. Tool-button labels are neutral (shapes/letters).

use crate::mesh::Mesh;
use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    DualSense,
    DualSenseEdge,
    DualShock4,
    Xbox,
    XboxElite,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    /// Main housing colour.
    Body,
    /// Dark inserts / sticks.
    Dark,
    /// Theme accent (buttons light up with it).
    Accent,
    /// Emissive light (lightbar).
    Glow,
    Metal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Anim {
    None,
    /// Stick tilts by axis values; `right` selects stick.
    Stick {
        right: bool,
    },
    Trigger {
        right: bool,
    },
    /// Button name matches `InputState.buttons` names.
    Button(&'static str),
    Touchpad,
}

pub struct Part {
    pub name: &'static str,
    pub mesh: Mesh,
    pub transform: Mat4,
    /// Pivot (in model space) around which stick tilt happens.
    pub pivot: Vec3,
    pub paint: Paint,
    pub anim: Anim,
}

fn part(name: &'static str, mesh: Mesh, t: Mat4, paint: Paint, anim: Anim) -> Part {
    Part { name, mesh, transform: t, pivot: t.transform_point3(Vec3::ZERO), paint, anim }
}

fn at(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}

/// Build all parts for a model. Units are roughly "controller widths / 3".
pub fn build(kind: ModelKind) -> Vec<Part> {
    let ps = matches!(kind, ModelKind::DualSense | ModelKind::DualSenseEdge | ModelKind::DualShock4);
    let mut p = Vec::new();

    // Housing.
    match kind {
        ModelKind::DualSense | ModelKind::DualSenseEdge => {
            p.push(part("body", Mesh::superellipsoid([1.25, 0.34, 0.62], 0.55, 0.8, 24, 48), at(0.0, 0.0, 0.0), Paint::Body, Anim::None));
            p.push(part(
                "midplate",
                Mesh::superellipsoid([0.5, 0.36, 0.45], 0.4, 0.7, 14, 28),
                at(0.0, 0.0, -0.02),
                Paint::Dark,
                Anim::None,
            ));
        }
        ModelKind::DualShock4 | ModelKind::Generic => {
            p.push(part("body", Mesh::superellipsoid([1.2, 0.33, 0.6], 0.6, 0.85, 24, 48), at(0.0, 0.0, 0.0), Paint::Dark, Anim::None));
        }
        ModelKind::Xbox | ModelKind::XboxElite => {
            p.push(part("body", Mesh::superellipsoid([1.15, 0.34, 0.58], 0.6, 0.85, 24, 48), at(0.0, 0.0, 0.0), Paint::Dark, Anim::None));
        }
    }
    // Grips.
    for (name, s) in [("grip_l", -1.0f32), ("grip_r", 1.0)] {
        let m = at(s * 0.95, -0.05, 0.62) * Mat4::from_rotation_y(-s * 0.30) * Mat4::from_rotation_x(-0.15);
        p.push(part(
            name,
            Mesh::superellipsoid([0.24, 0.30, 0.62], 0.8, 0.9, 16, 32),
            m,
            if kind == ModelKind::DualSense || kind == ModelKind::DualSenseEdge { Paint::Body } else { Paint::Dark },
            Anim::None,
        ));
    }
    // Sticks.
    let (lpos, rpos) = if ps { ((-0.42, 0.36, 0.28), (0.42, 0.36, 0.28)) } else { ((-0.78, 0.34, -0.12), (0.30, 0.34, 0.30)) };
    p.push(part("stick_base_l", Mesh::cylinder(0.26, 0.08, 24), at(lpos.0, lpos.1 - 0.04, lpos.2), Paint::Metal, Anim::None));
    p.push(part("stick_base_r", Mesh::cylinder(0.26, 0.08, 24), at(rpos.0, rpos.1 - 0.04, rpos.2), Paint::Metal, Anim::None));
    p.push(part("stick_l", Mesh::stick_cap(), at(lpos.0, lpos.1 + 0.02, lpos.2), Paint::Dark, Anim::Stick { right: false }));
    p.push(part("stick_r", Mesh::stick_cap(), at(rpos.0, rpos.1 + 0.02, rpos.2), Paint::Dark, Anim::Stick { right: true }));

    // D-pad (left on PS, below-left on Xbox) and face buttons.
    let (dp, fb) = if ps { ((-0.82, 0.33, -0.20), (0.82, 0.33, -0.20)) } else { ((-0.40, 0.33, 0.30), (0.80, 0.33, -0.12)) };
    for (name, bname, dx, dz) in [
        ("dpad_up", "up", 0.0, -0.17),
        ("dpad_down", "down", 0.0, 0.17),
        ("dpad_left", "left", -0.17, 0.0),
        ("dpad_right", "right", 0.17, 0.0),
    ] {
        p.push(part(name, Mesh::rbox([0.09, 0.04, 0.09]), at(dp.0 + dx, dp.1, dp.2 + dz), Paint::Dark, Anim::Button(bname)));
    }
    let faces: [(&'static str, [&'static str; 2], f32, f32); 4] = if ps {
        [
            ("face_top", ["triangle", "y"], 0.0, -0.17),
            ("face_bottom", ["cross", "a"], 0.0, 0.17),
            ("face_left", ["square", "x"], -0.17, 0.0),
            ("face_right", ["circle", "b"], 0.17, 0.0),
        ]
    } else {
        [
            ("face_top", ["y", "triangle"], 0.0, -0.17),
            ("face_bottom", ["a", "cross"], 0.0, 0.17),
            ("face_left", ["x", "square"], -0.17, 0.0),
            ("face_right", ["b", "circle"], 0.17, 0.0),
        ]
    };
    for (name, bn, dx, dz) in faces {
        let btn = bn[0];
        p.push(part(
            name,
            Mesh::cylinder(0.075, 0.07, 20),
            at(fb.0 + dx, fb.1, fb.2 + dz),
            if ps { Paint::Dark } else { Paint::Metal },
            Anim::Button(btn),
        ));
    }

    // Bumpers and triggers.
    for (bn, tn, s, bname) in [("bumper_l", "trigger_l", -1.0f32, "l1"), ("bumper_r", "trigger_r", 1.0, "r1")] {
        p.push(part(
            bn,
            Mesh::rbox([0.34, 0.07, 0.10]),
            at(s * 0.80, 0.30, -0.56) * Mat4::from_rotation_z(-s * 0.08),
            if ps && kind != ModelKind::DualShock4 { Paint::Body } else { Paint::Dark },
            Anim::Button(bname),
        ));
        p.push(part(tn, Mesh::rbox([0.28, 0.16, 0.14]), at(s * 0.80, 0.16, -0.68), Paint::Dark, Anim::Trigger { right: s > 0.0 }));
    }

    // Brand-neutral centre details.
    if ps {
        p.push(part("lightbar", Mesh::rbox([0.55, 0.012, 0.025]), at(0.0, 0.345, -0.54), Paint::Glow, Anim::None));
        if kind != ModelKind::Generic {
            p.push(part("touchpad", Mesh::rbox([0.36, 0.015, 0.20]), at(0.0, 0.345, -0.28), Paint::Metal, Anim::Touchpad));
        }
        p.push(part("btn_center", Mesh::cylinder(0.05, 0.05, 16), at(0.0, 0.33, 0.20), Paint::Metal, Anim::Button("home")));
    } else {
        p.push(part("btn_center", Mesh::cylinder(0.075, 0.05, 20), at(0.0, 0.34, -0.22), Paint::Accent, Anim::Button("guide")));
        p.push(part("btn_view", Mesh::cylinder(0.04, 0.04, 16), at(-0.2, 0.33, -0.12), Paint::Metal, Anim::Button("view")));
        p.push(part("btn_menu", Mesh::cylinder(0.04, 0.04, 16), at(0.2, 0.33, -0.12), Paint::Metal, Anim::Button("menu")));
    }
    if matches!(kind, ModelKind::DualSenseEdge | ModelKind::XboxElite) {
        for (n, s) in [("paddle_l", -1.0f32), ("paddle_r", 1.0)] {
            p.push(part(n, Mesh::rbox([0.05, 0.03, 0.30]), at(s * 0.85, -0.18, 0.95), Paint::Metal, Anim::None));
        }
    }
    p
}

/// Number of parts with a given animation kind (used by tests).
pub fn count_anim(parts: &[Part], f: impl Fn(&Anim) -> bool) -> usize {
    parts.iter().filter(|p| f(&p.anim)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ModelKind; 6] =
        [ModelKind::DualSense, ModelKind::DualSenseEdge, ModelKind::DualShock4, ModelKind::Xbox, ModelKind::XboxElite, ModelKind::Generic];

    #[test]
    fn every_model_has_required_named_parts() {
        for k in ALL {
            let parts = build(k);
            for n in ["stick_l", "stick_r", "trigger_l", "trigger_r", "bumper_l", "bumper_r", "dpad_up", "face_bottom", "body"] {
                assert!(parts.iter().any(|p| p.name == n), "{k:?} missing {n}");
            }
            assert_eq!(count_anim(&parts, |a| matches!(a, Anim::Stick { .. })), 2);
        }
    }

    #[test]
    fn ps_models_have_lightbar_and_touchpad() {
        for k in [ModelKind::DualSense, ModelKind::DualSenseEdge, ModelKind::DualShock4] {
            let parts = build(k);
            assert!(parts.iter().any(|p| p.name == "lightbar") && parts.iter().any(|p| p.name == "touchpad"));
        }
        assert!(!build(ModelKind::Xbox).iter().any(|p| p.name == "touchpad"));
    }
}
