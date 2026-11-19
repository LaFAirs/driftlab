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

mod logo;

/// Concept used for the app icon (0 = A orbit, 1 = B cap, 2 = C monogram).
const CHOSEN: usize = 0;

fn render_logo(size: u32) -> RgbaImage {
    logo::render(&logo::all(None)[CHOSEN], size)
}

fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut v = Vec::new();
    image::codecs::png::PngEncoder::new(&mut v)
        .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgba8)
        .expect("png encode");
    v
}

fn icons() {
    branding();
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

/// Everything derived from the chosen concept: SVG sources, mono + lockup variants, installer graphics.
fn branding() {
    let d = root().join("assets/branding");
    std::fs::create_dir_all(d.join("installer")).expect("mkdir");
    let chosen = &logo::all(None)[CHOSEN];
    let svg = logo::svg(chosen);
    std::fs::write(root().join("assets/logo.svg"), &svg).expect("logo.svg");
    std::fs::write(d.join("logo.svg"), &svg).expect("logo.svg");
    let mono_light = &logo::all(Some([0.93, 0.94, 0.97, 1.0]))[CHOSEN];
    let mono_dark = &logo::all(Some([0.07, 0.09, 0.13, 1.0]))[CHOSEN];
    std::fs::write(d.join("logo-mono-light.svg"), logo::svg(mono_light)).expect("mono");
    std::fs::write(d.join("logo-mono-dark.svg"), logo::svg(mono_dark)).expect("mono");
    // Lockup: icon + wordmark (SVG uses Inter / system sans; the PNG embeds the real Inter glyphs).
    let inner = svg.split_once('>').map(|x| x.1).unwrap_or("").trim_end().trim_end_matches("</svg>").to_string();
    for (name, ink, bg) in [("lockup-dark", "#ECF0F7", "#0E1117"), ("lockup-light", "#111827", "#FFFFFF")] {
        let s = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 800 256\" width=\"800\" height=\"256\" role=\"img\" aria-label=\"DriftLab\">\n<rect width=\"800\" height=\"256\" fill=\"{bg}\"/>\n<svg x=\"0\" y=\"0\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\">{inner}</svg>\n<text x=\"296\" y=\"170\" font-family=\"Inter, 'Segoe UI', Helvetica, Arial, sans-serif\" font-weight=\"600\" font-size=\"118\" letter-spacing=\"-2\" fill=\"{ink}\">DriftLab</text>\n</svg>\n"
        );
        std::fs::write(d.join(format!("{name}.svg")), s).expect("lockup");
    }
    let inter_semibold = std::fs::read(root().join("assets/fonts/Inter-SemiBold.ttf")).expect("font");
    let fb = FontRef::try_from_slice(&inter_semibold).expect("font");
    for (name, ink, bg) in [("lockup-dark", [236u8, 240, 247], [14u8, 17, 23]), ("lockup-light", [17, 24, 39], [255, 255, 255])] {
        let (w, h) = (1600u32, 512u32);
        let mut img = RgbaImage::from_pixel(w, h, Rgba([bg[0], bg[1], bg[2], 255]));
        image::imageops::overlay(&mut img, &render_logo(512), 0, 0);
        draw_text(&mut img, &fb, "DriftLab", 600.0, 150.0, 236.0, ink);
        img.save(d.join(format!("{name}.png"))).expect("lockup png");
    }
    // Installer graphics (Inno Setup wants 24-bit BMPs).
    let (w, h) = (164u32, 314u32);
    let mut wiz = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let t = y as f32 / h as f32;
            wiz.put_pixel(x, y, Rgba([(18.0 + 24.0 * t) as u8, (22.0 + 12.0 * t) as u8, (40.0 + 60.0 * t) as u8, 255]));
        }
    }
    image::imageops::overlay(&mut wiz, &render_logo(120), 22, 40);
    draw_text(&mut wiz, &fb, "DriftLab", 22.0, 175.0, 30.0, [236, 240, 247]);
    image::DynamicImage::ImageRgba8(wiz).to_rgb8().save(d.join("installer/wizard.bmp")).expect("bmp");
    let mut small = RgbaImage::from_pixel(55, 55, Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut small, &render_logo(55), 0, 0);
    image::DynamicImage::ImageRgba8(small).to_rgb8().save(d.join("installer/wizard-small.bmp")).expect("bmp");
    println!("branding written to {}", d.display());
}
fn concepts() {
    let dir = root().join("assets/branding/concepts");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let sizes = [16u32, 32, 48, 256, 512];
    let (w, rowh) = (2 * (sizes.iter().sum::<u32>() + 6 * 24), 580u32);
    let mut sheet = RgbaImage::from_pixel(w, rowh * 3, Rgba([243, 245, 249, 255]));
    let half = w / 2;
    for y in 0..rowh * 3 {
        for x in half..w {
            sheet.put_pixel(x, y, Rgba([14, 17, 23, 255]));
        }
    }
    let concepts = logo::all(None);
    for (row, c) in concepts.iter().enumerate() {
        std::fs::write(dir.join(format!("concept-{}.svg", c.name)), logo::svg(c)).expect("svg");
        std::fs::write(dir.join(format!("concept-{}.png", c.name)), png_bytes(&logo::render(c, 512))).expect("png");
        for panel in 0..2u32 {
            let mut x = panel * half + 24;
            for &s in &sizes {
                let img = logo::render(c, s);
                image::imageops::overlay(&mut sheet, &img, i64::from(x), i64::from(row as u32 * rowh + 24 + (512 - s)));
                x += s + 24;
            }
        }
    }
    for (i, c) in logo::all(Some([0.93, 0.94, 0.97, 1.0])).iter().enumerate() {
        std::fs::write(dir.join(format!("mono-{}.svg", ["a-orbit", "b-cap", "c-monogram"][i])), logo::svg(c)).expect("svg");
    }
    sheet.save(dir.join("comparison.png")).expect("sheet");
    println!("concepts written to {}", dir.display());
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
        Some("concepts") => concepts(),
        _ => eprintln!("usage: cargo xtask <icons|banners|concepts|screenshots|all>"),
    }
}
