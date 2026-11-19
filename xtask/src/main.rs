//! Build helpers: `cargo xtask icons|banners|screenshots|all`.
//!
//! * `icons`       – renders the logo (same geometry as `assets/logo.svg`) to PNGs, `.ico`, `.icns`
//! * `banners`     – README banner and GitHub social preview (1280×640)
//! * `screenshots` – runs the app in demo mode and captures the README screenshots

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use image::{codecs::ico::IcoEncoder, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").to_path_buf()
}

/// Signed distance helpers (logo is defined on a 256×256 canvas).
fn logo_pixel(x: f32, y: f32, with_bg: bool) -> Rgba<u8> {
    let (cx, cy) = (x - 128.0, y - 128.0);
    let r = (cx * cx + cy * cy).sqrt();
    let dot = ((x - 146.0).powi(2) + (y - 116.0).powi(2)).sqrt();
    let in_round = {
        let (qx, qy) = (cx.abs() - (128.0 - 56.0), cy.abs() - (128.0 - 56.0));
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - 56.0;
        outside <= 0.0
    };
    let tick = |a: f32, b: f32, c: f32, d: f32| x >= a && x <= b && y >= c && y <= d;
    let ticks = tick(123.0, 133.0, 18.0, 52.0)
        || tick(123.0, 133.0, 204.0, 238.0)
        || tick(18.0, 52.0, 123.0, 133.0)
        || tick(204.0, 238.0, 123.0, 133.0);
    if dot <= 26.0 {
        Rgba([45, 107, 255, 255])
    } else if (77.0..=91.0).contains(&(r - 0.0)) && (r - 84.0).abs() <= 7.0 || ticks {
        Rgba([236, 240, 247, 255])
    } else if with_bg && in_round {
        let t = (x + y) / 512.0;
        let l = |a: f32, b: f32| (a + (b - a) * t) as u8;
        Rgba([l(27.0, 11.0), l(35.0, 15.0), l(53.0, 23.0), 255])
    } else {
        Rgba([0, 0, 0, 0])
    }
}

fn render_logo(size: u32) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);
    let ss = 4;
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0u32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32 * 256.0;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32 * 256.0;
                    let p = logo_pixel(x, y, true);
                    // premultiply for correct edge blending
                    let a = u32::from(p[3]);
                    acc[0] += u32::from(p[0]) * a;
                    acc[1] += u32::from(p[1]) * a;
                    acc[2] += u32::from(p[2]) * a;
                    acc[3] += a;
                }
            }
            let n = (ss * ss) as u32;
            let a = acc[3] / n;
            let px_val = match (acc[0].checked_div(acc[3]), acc[1].checked_div(acc[3]), acc[2].checked_div(acc[3])) {
                (Some(r), Some(g), Some(b)) => [r as u8, g as u8, b as u8, a as u8],
                _ => [0, 0, 0, 0],
            };
            img.put_pixel(px, py, Rgba(px_val));
        }
    }
    img
}

fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut v = Vec::new();
    image::codecs::png::PngEncoder::new(&mut v)
        .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgba8)
        .expect("png encode");
    v
}

fn icons() {
    let dir = root().join("assets/icons");
    std::fs::create_dir_all(&dir).expect("mkdir");
    for s in [16, 24, 32, 48, 64, 128, 256, 512, 1024] {
        std::fs::write(dir.join(format!("icon-{s}.png")), png_bytes(&render_logo(s))).expect("write png");
    }
    // .ico (16..256)
    let frames: Vec<image::codecs::ico::IcoFrame> = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&s| image::codecs::ico::IcoFrame::as_png(render_logo(s).as_raw(), s, s, ExtendedColorType::Rgba8).expect("ico frame"))
        .collect();
    let f = std::fs::File::create(dir.join("driftlab.ico")).expect("create ico");
    IcoEncoder::new(f).encode_images(&frames).expect("ico");
    // .icns (PNG-based chunks)
    let mut body = Vec::new();
    for (tag, s) in [(b"ic07", 128u32), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024)] {
        let data = png_bytes(&render_logo(s));
        body.extend_from_slice(tag);
        body.extend_from_slice(&((data.len() + 8) as u32).to_be_bytes());
        body.extend_from_slice(&data);
    }
    let mut icns = b"icns".to_vec();
    icns.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    icns.extend(body);
    std::fs::write(dir.join("driftlab.icns"), icns).expect("write icns");
    println!("icons written to {}", dir.display());
}

fn draw_text(img: &mut RgbaImage, font: &FontRef, text: &str, x: f32, y: f32, px: f32, color: [u8; 3]) {
    let scale = PxScale::from(px);
    let sf = font.as_scaled(scale);
    let mut cx = x;
    for ch in text.chars() {
        let g = font.glyph_id(ch).with_scale_and_position(scale, point(cx, y + sf.ascent()));
        cx += sf.h_advance(g.id);
        if let Some(o) = font.outline_glyph(g) {
            let b = o.px_bounds();
            o.draw(|gx, gy, c| {
                let (ix, iy) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
                if ix >= 0 && iy >= 0 && (ix as u32) < img.width() && (iy as u32) < img.height() {
                    let p = img.get_pixel_mut(ix as u32, iy as u32);
                    for k in 0..3 {
                        p[k] = (f32::from(p[k]) * (1.0 - c) + f32::from(color[k]) * c) as u8;
                    }
                }
            });
        }
    }
}

fn banner(w: u32, h: u32, out: &Path) {
    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let t = (x as f32 / w as f32 * 0.6 + y as f32 / h as f32 * 0.4).clamp(0.0, 1.0);
            let l = |a: f32, b: f32| (a + (b - a) * t) as u8;
            // soft blue glow top-right
            let g = (1.0 - (((x as f32 - w as f32 * 0.85).powi(2) + (y as f32 - h as f32 * 0.1).powi(2)).sqrt() / (w as f32 * 0.6)))
                .clamp(0.0, 1.0);
            let add = (g * g * 60.0) as u8;
            img.put_pixel(
                x,
                y,
                Rgba([l(14.0, 8.0).saturating_add(0), l(20.0, 12.0).saturating_add(add / 3), l(36.0, 22.0).saturating_add(add), 255]),
            );
        }
    }
    let logo = render_logo(h * 2 / 5);
    image::imageops::overlay(&mut img, &logo, (w as i64 / 2) - (logo.width() as i64 / 2), (h as f32 * 0.07) as i64);
    let inter_semibold = std::fs::read(root().join("assets/fonts/Inter-SemiBold.ttf")).expect("font");
    let inter = std::fs::read(root().join("assets/fonts/Inter-Regular.ttf")).expect("font");
    let fb = FontRef::try_from_slice(&inter_semibold).expect("font");
    let fr = FontRef::try_from_slice(&inter).expect("font");
    let title_px = h as f32 * 0.16;
    let width_of = |f: &FontRef, s: &str, px: f32| {
        let sf = f.as_scaled(PxScale::from(px));
        s.chars().map(|c| sf.h_advance(f.glyph_id(c))).sum::<f32>()
    };
    let tw = width_of(&fb, "DriftLab", title_px);
    draw_text(&mut img, &fb, "DriftLab", (w as f32 - tw) / 2.0, h as f32 * 0.55, title_px, [236, 240, 247]);
    let sub_px = h as f32 * 0.065;
    let sw = width_of(&fr, "Fix stick drift. For real.", sub_px);
    draw_text(
        &mut img,
        &fr,
        "Fix stick drift. For real.",
        (w as f32 - sw) / 2.0,
        h as f32 * 0.55 + title_px * 1.25,
        sub_px,
        [150, 170, 205],
    );
    std::fs::create_dir_all(out.parent().expect("parent")).expect("mkdir");
    img.save(out).expect("save banner");
    println!("wrote {}", out.display());
}

fn banners() {
    let d = root().join(".github/assets");
    banner(1280, 640, &d.join("social-preview.png"));
    banner(1600, 480, &d.join("banner.png"));
    std::fs::copy(root().join("assets/icons/icon-256.png"), d.join("logo.png")).ok();
    std::fs::copy(root().join("assets/logo.svg"), d.join("logo.svg")).ok();
}

fn screenshots() {
    let dir = root().join(".github/assets/screenshots");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let status = std::process::Command::new("cargo")
        .args(["run", "--release", "-p", "driftlab-app", "--", "--screenshots"])
        .arg(&dir)
        .current_dir(root())
        .status()
        .expect("run app");
    assert!(status.success(), "screenshot run failed");
    println!("screenshots in {}", dir.display());
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        Some("banners") => banners(),
        Some("screenshots") => screenshots(),
        Some("all") => {
            icons();
            banners();
            screenshots();
        }
        _ => eprintln!("usage: cargo xtask <icons|banners|screenshots|all>"),
    }
}
