//! DriftLab logo: scene description + software rasteriser + SVG writer (one source for every size and format).
//!
//! Idea: a stick cap seen from above. A dot sits slightly off-centre (the drift) and a tapering path spirals
//! back to the centre (the correction). Three concepts are kept in `assets/branding/concepts/`.

use image::{Rgba, RgbaImage};

pub type Col = [f32; 4];

#[derive(Clone)]
pub enum Paint {
    Solid(Col),
    Lin { a: [f32; 2], b: [f32; 2], c0: Col, c1: Col },
    Rad { c: [f32; 2], r: f32, c0: Col, c1: Col },
}

#[derive(Clone)]
pub enum Geo {
    RRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    },
    Circle {
        c: [f32; 2],
        r: f32,
    },
    Ring {
        c: [f32; 2],
        r: f32,
        w: f32,
    },
    /// Tapered round stroke through points `(x, y, radius)`.
    Stroke(Vec<[f32; 3]>),
    /// Straight capsule.
    Bar {
        p0: [f32; 2],
        p1: [f32; 2],
        w: f32,
    },
    /// Ring segment, angles in degrees (screen space, clockwise), with round caps.
    Arc {
        c: [f32; 2],
        r: f32,
        w: f32,
        a0: f32,
        a1: f32,
    },
}

#[derive(Clone)]
pub struct Shape {
    pub geo: Geo,
    pub paint: Paint,
    /// Only visible inside the app-icon plate (used for the gloss).
    pub clip_plate: bool,
}

pub struct Logo {
    pub name: &'static str,
    pub shapes: Vec<Shape>,
    /// Plate shape (squircle) – absent in the flat monochrome variants.
    pub plate: Option<Shape>,
}

pub fn hex(h: &str, a: f32) -> Col {
    let v = |i: usize| f32::from(u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0)) / 255.0;
    [v(1), v(3), v(5), a]
}

fn spiral(c: [f32; 2], r0: f32, a0: f32, turn: f32, n: usize, w0: f32, w1: f32) -> Vec<[f32; 3]> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let r = r0 * (1.0 - t).powf(0.85);
            let a = (a0 + turn * t).to_radians();
            [c[0] + r * a.cos(), c[1] + r * a.sin(), (w0 + (w1 - w0) * t) / 2.0]
        })
        .collect()
}

fn solid(geo: Geo, col: Col) -> Shape {
    Shape { geo, paint: Paint::Solid(col), clip_plate: false }
}

fn plate(c0: Col, c1: Col) -> Shape {
    Shape {
        geo: Geo::RRect { x: 0.0, y: 0.0, w: 256.0, h: 256.0, r: 58.0 },
        paint: Paint::Lin { a: [30.0, 0.0], b: [226.0, 256.0], c0, c1 },
        clip_plate: false,
    }
}

fn gloss() -> Shape {
    Shape {
        geo: Geo::Circle { c: [128.0, -70.0], r: 215.0 },
        paint: Paint::Lin { a: [0.0, 0.0], b: [0.0, 150.0], c0: [1.0, 1.0, 1.0, 0.20], c1: [1.0, 1.0, 1.0, 0.0] },
        clip_plate: true,
    }
}

const C: [f32; 2] = [128.0, 128.0];

/// A: "Orbit" – blue→violet plate, concentric rings, drifting dot and the spiral back to the centre.
pub fn concept_a(flat: Option<Col>) -> Logo {
    let ink = |a: f32| flat.map_or([1.0, 1.0, 1.0, a], |c| [c[0], c[1], c[2], a.max(0.55)]);
    let sp = spiral(C, 54.0, -40.0, 300.0, 96, 10.5, 2.0);
    let d = [sp[0][0], sp[0][1]];
    let mut s = Vec::new();
    if flat.is_none() {
        s.push(gloss());
    }
    s.push(solid(Geo::Ring { c: C, r: 96.0, w: 9.0 }, ink(0.96)));
    s.push(solid(Geo::Ring { c: C, r: 70.0, w: 4.5 }, ink(0.5)));
    s.push(solid(Geo::Stroke(sp), ink(0.98)));
    s.push(solid(Geo::Circle { c: C, r: 5.5 }, ink(0.95)));
    if flat.is_none() {
        s.push(Shape {
            geo: Geo::Circle { c: d, r: 30.0 },
            paint: Paint::Rad { c: d, r: 30.0, c0: [1.0, 1.0, 1.0, 0.45], c1: [1.0, 1.0, 1.0, 0.0] },
            clip_plate: false,
        });
    }
    s.push(solid(Geo::Circle { c: d, r: 16.0 }, if flat.is_some() { ink(1.0) } else { hex("#FFFFFF", 1.0) }));
    if flat.is_none() {
        s.push(solid(Geo::Circle { c: d, r: 10.0 }, hex("#5B8CFF", 1.0)));
    }
    Logo { name: "a-orbit", shapes: s, plate: flat.is_none().then(|| plate(hex("#3B82F6", 1.0), hex("#7C3AED", 1.0))) }
}

/// B: "Cap" – dark plate, a lit stick cap with grooves, cyan drift dot and a light spiral.
pub fn concept_b(flat: Option<Col>) -> Logo {
    let mut s = Vec::new();
    let sp = spiral(C, 44.0, -40.0, 220.0, 64, 10.0, 2.5);
    let d = [sp[0][0], sp[0][1]];
    if flat.is_none() {
        s.push(gloss());
        s.push(Shape {
            geo: Geo::Circle { c: C, r: 98.0 },
            paint: Paint::Rad { c: [100.0, 92.0], r: 140.0, c0: hex("#5B6B86", 1.0), c1: hex("#1B2333", 1.0) },
            clip_plate: false,
        });
        for (r, a) in [(84.0, 0.30), (70.0, 0.24), (56.0, 0.18)] {
            s.push(solid(Geo::Ring { c: C, r, w: 3.0 }, [0.0, 0.0, 0.0, a]));
            s.push(solid(Geo::Ring { c: C, r: r + 3.5, w: 1.6 }, [1.0, 1.0, 1.0, a * 0.5]));
        }
        s.push(solid(Geo::Ring { c: C, r: 98.0, w: 4.0 }, [1.0, 1.0, 1.0, 0.55]));
        s.push(solid(Geo::Stroke(sp), hex("#7FD4FF", 1.0)));
        s.push(solid(Geo::Circle { c: C, r: 5.0 }, hex("#0A0F1A", 1.0)));
        s.push(Shape {
            geo: Geo::Circle { c: d, r: 30.0 },
            paint: Paint::Rad { c: d, r: 30.0, c0: hex("#38BDF8", 0.55), c1: hex("#38BDF8", 0.0) },
            clip_plate: false,
        });
        s.push(Shape {
            geo: Geo::Circle { c: d, r: 16.5 },
            paint: Paint::Lin {
                a: [d[0] - 14.0, d[1] - 14.0],
                b: [d[0] + 14.0, d[1] + 14.0],
                c0: hex("#7DD3FC", 1.0),
                c1: hex("#2563EB", 1.0),
            },
            clip_plate: false,
        });
    } else {
        let c = flat.unwrap_or([1.0, 1.0, 1.0, 1.0]);
        s.push(solid(Geo::Ring { c: C, r: 96.0, w: 10.0 }, c));
        s.push(solid(Geo::Ring { c: C, r: 66.0, w: 4.0 }, [c[0], c[1], c[2], 0.6]));
        s.push(solid(Geo::Stroke(sp), c));
        s.push(solid(Geo::Circle { c: d, r: 17.0 }, c));
    }
    Logo { name: "b-cap", shapes: s, plate: flat.is_none().then(|| plate(hex("#111827", 1.0), hex("#312E81", 1.0))) }
}

/// C: "D monogram" – a D built from an arc and a bar, with the drifting dot and its path inside the counter.
pub fn concept_c(flat: Option<Col>) -> Logo {
    let ink = |a: f32| flat.map_or([1.0, 1.0, 1.0, a], |c| [c[0], c[1], c[2], a]);
    let mut s = Vec::new();
    if flat.is_none() {
        s.push(gloss());
    }
    s.push(solid(Geo::Bar { p0: [84.0, 52.0], p1: [84.0, 204.0], w: 24.0 }, ink(1.0)));
    s.push(solid(Geo::Arc { c: [96.0, 128.0], r: 62.0, w: 24.0, a0: -90.0, a1: 90.0 }, ink(1.0)));
    let cc = [112.0, 128.0];
    let sp = spiral(cc, 24.0, -45.0, 200.0, 48, 8.0, 2.0);
    let d = [sp[0][0], sp[0][1]];
    let accent = flat.unwrap_or(hex("#FBBF24", 1.0));
    s.push(solid(Geo::Stroke(sp), if flat.is_some() { ink(1.0) } else { hex("#FDE68A", 1.0) }));
    s.push(solid(Geo::Circle { c: d, r: 12.0 }, accent));
    Logo { name: "c-monogram", shapes: s, plate: flat.is_none().then(|| plate(hex("#06B6D4", 1.0), hex("#2563EB", 1.0))) }
}

pub fn all(flat: Option<Col>) -> [Logo; 3] {
    [concept_a(flat), concept_b(flat), concept_c(flat)]
}

// ----------------------------------------------------------------------------------------------------------------
// rasteriser

fn inside(g: &Geo, x: f32, y: f32) -> bool {
    match g {
        Geo::RRect { x: rx, y: ry, w, h, r } => {
            let (cx, cy) = (x - (rx + w / 2.0), y - (ry + h / 2.0));
            let (qx, qy) = (cx.abs() - (w / 2.0 - r), cy.abs() - (h / 2.0 - r));
            (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r <= 0.0
        }
        Geo::Circle { c, r } => (x - c[0]).powi(2) + (y - c[1]).powi(2) <= r * r,
        Geo::Ring { c, r, w } => (((x - c[0]).powi(2) + (y - c[1]).powi(2)).sqrt() - r).abs() <= w / 2.0,
        Geo::Bar { p0, p1, w } => seg_dist(x, y, *p0, *p1) <= w / 2.0,
        Geo::Arc { c, r, w, a0, a1 } => {
            let (dx, dy) = (x - c[0], y - c[1]);
            let dist = (dx * dx + dy * dy).sqrt();
            let ang = dy.atan2(dx).to_degrees();
            let in_ang = ang >= *a0 && ang <= *a1;
            let cap = |a: f32| {
                let ar = a.to_radians();
                ((x - (c[0] + r * ar.cos())).powi(2) + (y - (c[1] + r * ar.sin())).powi(2)).sqrt() <= w / 2.0
            };
            (in_ang && (dist - r).abs() <= w / 2.0) || cap(*a0) || cap(*a1)
        }
        Geo::Stroke(p) => {
            for pair in p.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
                let l2 = (abx * abx + aby * aby).max(1e-9);
                let u = (((x - a[0]) * abx + (y - a[1]) * aby) / l2).clamp(0.0, 1.0);
                let (px, py) = (a[0] + abx * u, a[1] + aby * u);
                if ((x - px).powi(2) + (y - py).powi(2)).sqrt() <= a[2] + (b[2] - a[2]) * u {
                    return true;
                }
            }
            false
        }
    }
}

fn seg_dist(x: f32, y: f32, a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let l2 = (abx * abx + aby * aby).max(1e-9);
    let u = (((x - a[0]) * abx + (y - a[1]) * aby) / l2).clamp(0.0, 1.0);
    ((x - (a[0] + abx * u)).powi(2) + (y - (a[1] + aby * u)).powi(2)).sqrt()
}

fn paint_at(p: &Paint, x: f32, y: f32) -> Col {
    let mix = |a: Col, b: Col, t: f32| {
        let t = t.clamp(0.0, 1.0);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
    };
    match p {
        Paint::Solid(c) => *c,
        Paint::Lin { a, b, c0, c1 } => {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            mix(*c0, *c1, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9))
        }
        Paint::Rad { c, r, c0, c1 } => mix(*c0, *c1, ((x - c[0]).powi(2) + (y - c[1]).powi(2)).sqrt() / r),
    }
}

fn sample(logo: &Logo, x: f32, y: f32) -> Col {
    // premultiplied accumulation, back to front
    let mut out = [0.0f32; 4];
    let mut put = |c: Col| {
        let a = c[3];
        out = [c[0] * a + out[0] * (1.0 - a), c[1] * a + out[1] * (1.0 - a), c[2] * a + out[2] * (1.0 - a), a + out[3] * (1.0 - a)];
    };
    let in_plate = logo.plate.as_ref().is_none_or(|p| inside(&p.geo, x, y));
    if let Some(p) = &logo.plate {
        if in_plate {
            put(paint_at(&p.paint, x, y));
        }
    }
    for s in &logo.shapes {
        if (!s.clip_plate || in_plate) && inside(&s.geo, x, y) {
            put(paint_at(&s.paint, x, y));
        }
    }
    out
}

pub fn render(logo: &Logo, size: u32) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);
    let ss = if size <= 64 {
        6
    } else if size <= 256 {
        4
    } else {
        3
    };
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32 * 256.0;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32 * 256.0;
                    let c = sample(logo, x, y);
                    for k in 0..4 {
                        acc[k] += c[k];
                    }
                }
            }
            let n = (ss * ss) as f32;
            let a = acc[3] / n;
            let px_val = if a <= 1e-4 {
                [0, 0, 0, 0]
            } else {
                [
                    (acc[0] / n / a * 255.0).round() as u8,
                    (acc[1] / n / a * 255.0).round() as u8,
                    (acc[2] / n / a * 255.0).round() as u8,
                    (a * 255.0).round() as u8,
                ]
            };
            img.put_pixel(px, py, Rgba(px_val));
        }
    }
    img
}

// ----------------------------------------------------------------------------------------------------------------
// SVG

fn css(c: Col) -> String {
    format!("#{:02X}{:02X}{:02X}", (c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8)
}

fn stroke_polygon(p: &[[f32; 3]]) -> String {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for i in 0..p.len() {
        let (a, b) = (p[i.saturating_sub(1)], p[(i + 1).min(p.len() - 1)]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / l, dx / l);
        left.push(format!("{:.2},{:.2}", p[i][0] + nx * p[i][2], p[i][1] + ny * p[i][2]));
        right.push(format!("{:.2},{:.2}", p[i][0] - nx * p[i][2], p[i][1] - ny * p[i][2]));
    }
    right.reverse();
    left.extend(right);
    left.join(" ")
}

pub fn svg(logo: &Logo) -> String {
    let mut defs = String::new();
    let mut body = String::new();
    let mut id = 0;
    let mut fill = |p: &Paint, defs: &mut String| -> String {
        match p {
            Paint::Solid(c) => format!("fill=\"{}\" fill-opacity=\"{:.2}\"", css(*c), c[3]),
            Paint::Lin { a, b, c0, c1 } => {
                id += 1;
                defs.push_str(&format!(
                    "<linearGradient id=\"g{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"><stop offset=\"0\" stop-color=\"{}\" stop-opacity=\"{:.2}\"/><stop offset=\"1\" stop-color=\"{}\" stop-opacity=\"{:.2}\"/></linearGradient>",
                    a[0], a[1], b[0], b[1], css(*c0), c0[3], css(*c1), c1[3]
                ));
                format!("fill=\"url(#g{id})\"")
            }
            Paint::Rad { c, r, c0, c1 } => {
                id += 1;
                defs.push_str(&format!(
                    "<radialGradient id=\"g{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\"><stop offset=\"0\" stop-color=\"{}\" stop-opacity=\"{:.2}\"/><stop offset=\"1\" stop-color=\"{}\" stop-opacity=\"{:.2}\"/></radialGradient>",
                    c[0], c[1], r, css(*c0), c0[3], css(*c1), c1[3]
                ));
                format!("fill=\"url(#g{id})\"")
            }
        }
    };
    if let Some(p) = &logo.plate {
        let f = fill(&p.paint, &mut defs);
        body.push_str(&format!("<rect width=\"256\" height=\"256\" rx=\"58\" {f}/>\n"));
        body.push_str("<clipPath id=\"plate\"><rect width=\"256\" height=\"256\" rx=\"58\"/></clipPath>\n");
    }
    for s in &logo.shapes {
        let f = fill(&s.paint, &mut defs);
        let clip = if s.clip_plate { " clip-path=\"url(#plate)\"" } else { "" };
        let stroke_of = |col: &Paint| match col {
            Paint::Solid(c) => format!("stroke=\"{}\" stroke-opacity=\"{:.2}\"", css(*c), c[3]),
            _ => String::new(),
        };
        match &s.geo {
            Geo::RRect { x, y, w, h, r } => {
                body.push_str(&format!("<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{r}\" {f}{clip}/>\n"))
            }
            Geo::Circle { c, r } => body.push_str(&format!("<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{r}\" {f}{clip}/>\n", c[0], c[1])),
            Geo::Ring { c, r, w } => body.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{r}\" fill=\"none\" {} stroke-width=\"{w}\"{clip}/>\n",
                c[0],
                c[1],
                stroke_of(&s.paint)
            )),
            Geo::Bar { p0, p1, w } => body.push_str(&format!(
                "<path d=\"M{} {}L{} {}\" fill=\"none\" {} stroke-width=\"{w}\" stroke-linecap=\"round\"{clip}/>\n",
                p0[0],
                p0[1],
                p1[0],
                p1[1],
                stroke_of(&s.paint)
            )),
            Geo::Arc { c, r, w, a0, a1 } => {
                let pt = |a: f32| (c[0] + r * a.to_radians().cos(), c[1] + r * a.to_radians().sin());
                let ((x0, y0), (x1, y1)) = (pt(*a0), pt(*a1));
                body.push_str(&format!("<path d=\"M{x0:.2} {y0:.2}A{r} {r} 0 0 1 {x1:.2} {y1:.2}\" fill=\"none\" {} stroke-width=\"{w}\" stroke-linecap=\"round\"{clip}/>\n", stroke_of(&s.paint)));
            }
            Geo::Stroke(p) => {
                body.push_str(&format!("<polygon points=\"{}\" {f}/>\n", stroke_polygon(p)));
                for q in [p[0], p[p.len() - 1]] {
                    body.push_str(&format!("<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" {f}/>\n", q[0], q[1], q[2]));
                }
            }
        }
    }
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 256 256\" width=\"256\" height=\"256\" role=\"img\" aria-label=\"DriftLab logo\">\n<defs>{defs}</defs>\n{body}</svg>\n")
}
