//! glTF/GLB loader for the controller models (`assets/models/*.glb`, prepared by `tools/models/*.py`).
//!
//! Node names drive the animation (`stick_l`, `trigger_r`, `face_cross`, `btn_home`, …). Child nodes
//! (button symbols, letters) follow the animation of their nearest animated ancestor, using that
//! ancestor's origin as pivot – so the Blender object origin *is* the real hinge / ball-joint point.
//! Materials follow the glTF metallic-roughness spec (base colour, metal/rough, normal, occlusion, emissive).

use crate::mesh::{Mesh, Vertex};
use crate::model::{anim_for_name, Anim, MatKind, Material, Part, TexRefs};
use glam::{Mat4, Vec3};

/// Decoded RGBA8 image (already limited to `MAX_TEX` pixels on the long side).
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Sampled as sRGB colour (base colour / emissive) instead of linear data.
    pub srgb: bool,
}

pub struct Loaded {
    pub parts: Vec<Part>,
    pub images: Vec<Image>,
    pub ground_y: f32,
    pub radius: f32,
}

/// Target model width after normalisation (x extent), in scene units.
const TARGET_WIDTH: f32 = 2.6;
/// Maximum texture size (2K as requested).
pub const MAX_TEX: u32 = 2048;

pub fn load(bytes: &[u8]) -> Result<Loaded, String> {
    let (doc, buffers, images) = gltf::import_slice(bytes).map_err(|e| e.to_string())?;
    let scene = doc.default_scene().or_else(|| doc.scenes().next()).ok_or("glTF has no scene")?;
    let mut parts = Vec::new();
    for n in scene.nodes() {
        walk(&n, Mat4::IDENTITY, None, &buffers, &mut parts);
    }
    if parts.is_empty() {
        return Err("glTF contains no meshes".into());
    }
    let mut imgs: Vec<Image> = images.iter().map(to_rgba).collect();
    // Colour roles decide sRGB-vs-linear sampling.
    for p in &parts {
        for i in [p.material.tex.base, p.material.tex.emissive].into_iter().flatten() {
            if let Some(im) = imgs.get_mut(i) {
                im.srgb = true;
            }
        }
    }
    let (ground_y, radius) = normalize(&mut parts);
    Ok(Loaded { parts, images: imgs, ground_y, radius })
}

fn to_rgba(d: &gltf::image::Data) -> Image {
    use gltf::image::Format as F;
    let (w, h) = (d.width as usize, d.height as usize);
    let mut rgba = vec![255u8; w * h * 4];
    let px = |i: usize| i * 4;
    match d.format {
        F::R8G8B8A8 => rgba.copy_from_slice(&d.pixels[..w * h * 4]),
        F::R8G8B8 => {
            for i in 0..w * h {
                rgba[px(i)..px(i) + 3].copy_from_slice(&d.pixels[i * 3..i * 3 + 3]);
            }
        }
        F::R8G8 => {
            for i in 0..w * h {
                rgba[px(i)] = d.pixels[i * 2];
                rgba[px(i) + 1] = d.pixels[i * 2 + 1];
                rgba[px(i) + 2] = 0;
            }
        }
        F::R8 => {
            for i in 0..w * h {
                let v = d.pixels[i];
                rgba[px(i)..px(i) + 3].copy_from_slice(&[v, v, v]);
            }
        }
        _ => {
            // 16-bit / float formats are not used by the bundled models: keep white.
        }
    }
    let mut img = Image { width: d.width, height: d.height, rgba, srgb: false };
    while img.width.max(img.height) > MAX_TEX {
        img = halve(&img);
    }
    img
}

/// 2× box downsample.
pub fn halve(i: &Image) -> Image {
    let (nw, nh) = ((i.width / 2).max(1), (i.height / 2).max(1));
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..4 {
                let mut s = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(i.width - 1);
                    let sy = (y * 2 + dy).min(i.height - 1);
                    s += u32::from(i.rgba[((sy * i.width + sx) * 4 + c) as usize]);
                }
                out[((y * nw + x) * 4 + c) as usize] = (s / 4) as u8;
            }
        }
    }
    Image { width: nw, height: nh, rgba: out, srgb: i.srgb }
}

fn walk(node: &gltf::Node, parent: Mat4, owner: Option<(Anim, Vec3)>, buffers: &[gltf::buffer::Data], out: &mut Vec<Part>) {
    let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    let name = node.name().unwrap_or("").to_string();
    let own_anim = anim_for_name(&name);
    let owner = if own_anim != Anim::None { Some((own_anim, world.transform_point3(Vec3::ZERO))) } else { owner };
    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(pos) = reader.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.collect();
            let normals: Vec<[f32; 3]> = reader.read_normals().map(|n| n.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
            let uvs: Vec<[f32; 2]> =
                reader.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
            let idx: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..pos.len() as u32).collect(),
            };
            let verts =
                pos.iter().zip(normals.iter()).zip(uvs.iter()).map(|((p, n), uv)| Vertex { pos: *p, normal: *n, uv: *uv }).collect();
            let (anim, pivot) = owner.clone().unwrap_or((Anim::None, world.transform_point3(Vec3::ZERO)));
            out.push(Part {
                name: name.clone(),
                mesh: Mesh { verts, idx },
                transform: world,
                pivot,
                material: material_of(&prim.material()),
                anim,
            });
        }
    }
    for c in node.children() {
        walk(&c, world, owner.clone(), buffers, out);
    }
}

fn material_of(m: &gltf::Material) -> Material {
    let pbr = m.pbr_metallic_roughness();
    let c = pbr.base_color_factor();
    let name = m.name().unwrap_or("").to_ascii_lowercase();
    let e = m.emissive_factor();
    let strength = m.emissive_strength().unwrap_or(1.0);
    let kind = if name.starts_with("glow_guide") {
        MatKind::Guide
    } else if name.starts_with("glow") {
        MatKind::Glow
    } else if name.starts_with("accent") {
        MatKind::Accent
    } else if name.contains("grip") {
        MatKind::Grip
    } else {
        MatKind::Normal
    };
    let emissive = if matches!(kind, MatKind::Glow | MatKind::Guide) { strength.max(1.0) } else { 0.0 };
    let tex = TexRefs {
        base: pbr.base_color_texture().map(|t| t.texture().source().index()),
        metal_rough: pbr.metallic_roughness_texture().map(|t| t.texture().source().index()),
        normal: m.normal_texture().map(|t| t.texture().source().index()),
        occlusion: m.occlusion_texture().map(|t| t.texture().source().index()),
        emissive: m.emissive_texture().map(|t| t.texture().source().index()),
    };
    Material {
        base: [c[0], c[1], c[2]],
        metallic: pbr.metallic_factor(),
        rough: pbr.roughness_factor(),
        emissive,
        emissive_factor: [e[0] * strength, e[1] * strength, e[2] * strength],
        normal_scale: m.normal_texture().map(|t| t.scale()).unwrap_or(1.0),
        kind,
        tex,
    }
}

/// Scale to a fixed width, centre horizontally and vertically; returns (ground_y, bounding radius).
pub fn normalize(parts: &mut [Part]) -> (f32, f32) {
    let (mut mn, mut mx) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in parts.iter() {
        for v in &p.mesh.verts {
            let w = p.transform.transform_point3(Vec3::from(v.pos));
            mn = mn.min(w);
            mx = mx.max(w);
        }
    }
    let width = (mx.x - mn.x).max(1e-6);
    let s = TARGET_WIDTH / width;
    let center = (mn + mx) * 0.5;
    let root = Mat4::from_scale(Vec3::splat(s)) * Mat4::from_translation(-center);
    for p in parts.iter_mut() {
        p.transform = root * p.transform;
        p.pivot = root.transform_point3(p.pivot);
    }
    let half = (mx - mn) * 0.5 * s;
    (-half.y - 0.04, half.length())
}
