//! Headless preview renderer for model acceptance checks.
//!
//! cargo run --release -p driftlab-render --example preview -- <kind> <out.png> [--glb file.glb]
//!     [--yaw 0.0] [--pitch 0.72] [--l2 0..1] [--stick 0.5,0.5] [--press cross,up] [--exposure 1.0] [--size 1100x760]
//! kind: dualsense | edge | ds4 | xbox | elite

use driftlab_render::{Kind, Palette, Pose, Renderer};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let kind_s = a.first().map(String::as_str).unwrap_or("dualsense");
    let out = a.get(1).cloned().unwrap_or_else(|| "preview.png".into());
    let get = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let (kind, theme) = match kind_s {
        "edge" => (Kind::DualSenseEdge, "ps5_edge"),
        "ds4" => (Kind::DualShock4, "ps4"),
        "xbox" => (Kind::Xbox, "xbox"),
        "elite" => (Kind::XboxElite, "xbox"),
        _ => (Kind::DualSense, "ps5"),
    };
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("no GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
    let mut r = Renderer::new(device, queue);
    if let Some(e) = get("--exposure").and_then(|s| s.parse().ok()) {
        r.exposure = e;
    }
    if let Some(p) = get("--glb") {
        let bytes = std::fs::read(&p).expect("read glb");
        r.load_model_bytes(kind, &bytes).expect("load glb");
    }
    let mut pose = Pose { palette: Palette::for_theme(theme), ..Default::default() };
    if let Some(v) = get("--yaw").and_then(|s| s.parse().ok()) {
        pose.yaw = v;
    }
    if let Some(v) = get("--pitch").and_then(|s| s.parse().ok()) {
        pose.pitch = v;
    }
    if let Some(v) = get("--l2").and_then(|s| s.parse().ok()) {
        pose.l2 = v;
        pose.r2 = v;
    }
    if let Some(s) = get("--stick") {
        let v: Vec<f32> = s.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() == 2 {
            pose.left = (v[0], v[1]);
            pose.right = (-v[0], -v[1]);
        }
    }
    if let Some(s) = get("--press") {
        pose.pressed = s.split(',').map(String::from).collect();
    }
    let size = get("--size").and_then(|s| s.split_once('x').map(|(w, h)| (w.parse().unwrap_or(1100), h.parse().unwrap_or(760)))).unwrap_or((1100, 760));
    r.render(kind, &pose, size);
    let (w, h, rgba) = r.read_rgba().expect("readback");
    // Composite over a mid-grey studio backdrop so transparency is visible.
    let mut img = image::RgbaImage::new(w, h);
    for (i, p) in img.pixels_mut().enumerate() {
        let (x, y) = ((i as u32 % w) as f32 / w as f32, (i as u32 / w) as f32 / h as f32);
        let bg = 0.16 + 0.10 * (1.0 - y) + 0.02 * (x - 0.5).abs();
        let s = i * 4;
        let a = f32::from(rgba[s + 3]) / 255.0;
        // The renderer outputs straight colour for the model and premultiplied for the ground; treat as premultiplied.
        let c = |k: usize| ((f32::from(rgba[s + k]) / 255.0 + bg * 255.0 / 255.0 * (1.0 - a)).clamp(0.0, 1.0) * 255.0) as u8;
        *p = image::Rgba([c(0), c(1), c(2), 255]);
    }
    img.save(&out).expect("save png");
    println!("wrote {out}");
}
