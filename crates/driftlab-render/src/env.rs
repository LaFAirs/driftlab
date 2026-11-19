//! Image-based lighting from an equirectangular Radiance HDR (Poly Haven `studio_small_03`, CC0).
//!
//! Pre-computed on the CPU at start-up (well under a second):
//! * diffuse irradiance as 9 spherical-harmonics coefficients
//! * GGX-prefiltered specular equirect with one mip level per roughness step
//! * split-sum BRDF lookup table (scale/bias)
//! * direction of the brightest light, used for the shadow-casting key light

use glam::Vec3;
use std::f32::consts::{PI, TAU};

pub const SPEC_LEVELS: u32 = 7;
pub const SPEC_W: u32 = 256;
pub const SPEC_H: u32 = 128;
pub const LUT_SIZE: u32 = 64;

pub struct Env {
    /// SH9 irradiance coefficients (RGB in xyz).
    pub sh: [[f32; 4]; 9],
    pub key_dir: Vec3,
    /// Rgba16F bytes for all mip levels, level 0 first (level k is `SPEC_W>>k` × `SPEC_H>>k`).
    pub spec: Vec<u8>,
    /// Rgba8 BRDF LUT, `LUT_SIZE`², x = NdotV, y = roughness.
    pub lut: Vec<u8>,
}

struct Equirect {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
}

impl Equirect {
    fn dir(&self, x: usize, y: usize) -> Vec3 {
        dir_of(((x as f32) + 0.5) / self.w as f32, ((y as f32) + 0.5) / self.h as f32)
    }

    fn sample(&self, d: Vec3) -> [f32; 3] {
        let (u, v) = uv_of(d);
        let fx = u * self.w as f32 - 0.5;
        let fy = (v * self.h as f32 - 0.5).clamp(0.0, self.h as f32 - 1.0);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let at = |x: f32, y: f32| self.px[(y as usize).min(self.h - 1) * self.w + (x.rem_euclid(self.w as f32) as usize).min(self.w - 1)];
        let (a, b, c, d2) = (at(x0, y0), at(x0 + 1.0, y0), at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
        let mut o = [0.0; 3];
        for k in 0..3 {
            o[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d2[k] * tx) * ty;
        }
        o
    }

    fn downsample(&self, nw: usize, nh: usize) -> Equirect {
        let mut px = vec![[0.0f32; 3]; nw * nh];
        let (sx, sy) = (self.w / nw, self.h / nh);
        for y in 0..nh {
            for x in 0..nw {
                let mut s = [0.0f32; 3];
                for j in 0..sy {
                    for i in 0..sx {
                        let p = self.px[(y * sy + j) * self.w + x * sx + i];
                        for k in 0..3 {
                            s[k] += p[k];
                        }
                    }
                }
                let n = (sx * sy) as f32;
                px[y * nw + x] = [s[0] / n, s[1] / n, s[2] / n];
            }
        }
        Equirect { w: nw, h: nh, px }
    }
}

/// Equirect (u, v) → direction. +Y is up; u = 0.5 looks towards +X.
pub fn dir_of(u: f32, v: f32) -> Vec3 {
    let phi = (u - 0.5) * TAU;
    let theta = v * PI;
    Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin())
}

/// Direction → equirect (u, v); inverse of `dir_of` (the WGSL shader uses the same convention).
pub fn uv_of(d: Vec3) -> (f32, f32) {
    let d = d.normalize_or_zero();
    (d.z.atan2(d.x) / TAU + 0.5, d.y.clamp(-1.0, 1.0).acos() / PI)
}

fn radical_inverse(mut bits: u32) -> f32 {
    bits = bits.rotate_left(16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xAAAA_AAAA) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xCCCC_CCCC) >> 2);
    bits = ((bits & 0x0F0F_0F0F) << 4) | ((bits & 0xF0F0_F0F0) >> 4);
    bits = ((bits & 0x00FF_00FF) << 8) | ((bits & 0xFF00_FF00) >> 8);
    bits as f32 * 2.328_306_4e-10
}

fn importance_ggx(i: u32, n: u32, rough: f32, normal: Vec3) -> Vec3 {
    let (xi1, xi2) = (i as f32 / n as f32, radical_inverse(i));
    let a = rough * rough;
    let phi = TAU * xi1;
    let cos_t = ((1.0 - xi2) / (1.0 + (a * a - 1.0) * xi2)).sqrt();
    let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
    let h = Vec3::new(sin_t * phi.cos(), sin_t * phi.sin(), cos_t);
    let up = if normal.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tx = up.cross(normal).normalize();
    let ty = normal.cross(tx);
    (tx * h.x + ty * h.y + normal * h.z).normalize()
}

fn f16_bytes(out: &mut Vec<u8>, c: [f32; 3]) {
    for v in [c[0], c[1], c[2], 1.0] {
        out.extend_from_slice(&half::f16::from_f32(v.min(60000.0)).to_le_bytes());
    }
}

pub fn build(hdr_bytes: &[u8]) -> Result<Env, String> {
    let img = image::ImageReader::with_format(std::io::Cursor::new(hdr_bytes), image::ImageFormat::Hdr).decode().map_err(|e| e.to_string())?.to_rgb32f();
    let full = Equirect { w: img.width() as usize, h: img.height() as usize, px: img.pixels().map(|p| p.0).collect() };
    let src = full.downsample(SPEC_W as usize, SPEC_H as usize);

    // Key light: luminance-weighted centroid of the brightest 0.5 % of the pixels.
    let lum = |p: &[f32; 3]| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
    let mut sorted: Vec<f32> = src.px.iter().map(lum).collect();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let thr = sorted[(sorted.len() / 200).max(1)];
    let mut key = Vec3::ZERO;
    for y in 0..src.h {
        for x in 0..src.w {
            let l = lum(&src.px[y * src.w + x]);
            if l >= thr {
                key += src.dir(x, y) * l;
            }
        }
    }
    let key_dir = if key.length() > 1e-6 { key.normalize() } else { Vec3::new(-0.5, 0.8, 0.4).normalize() };

    // SH9 irradiance (Ramamoorthi & Hanrahan).
    let mut sh = [Vec3::ZERO; 9];
    for y in 0..src.h {
        let theta = (y as f32 + 0.5) / src.h as f32 * PI;
        let dw = theta.sin() * (PI / src.h as f32) * (TAU / src.w as f32);
        for x in 0..src.w {
            let d = src.dir(x, y);
            let c = Vec3::from(src.px[y * src.w + x]) * dw;
            let b = [0.282095, 0.488603 * d.y, 0.488603 * d.z, 0.488603 * d.x, 1.092548 * d.x * d.y, 1.092548 * d.y * d.z, 0.315392 * (3.0 * d.z * d.z - 1.0), 1.092548 * d.x * d.z, 0.546274 * (d.x * d.x - d.y * d.y)];
            for k in 0..9 {
                sh[k] += c * b[k];
            }
        }
    }
    // Convolve with the cosine lobe and fold the basis constants in so the shader only needs dot products.
    let band = [PI, 2.0 * PI / 3.0, 2.0 * PI / 3.0, 2.0 * PI / 3.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0];
    let mut sh_out = [[0.0f32; 4]; 9];
    for k in 0..9 {
        let v = sh[k] * band[k] / PI;
        sh_out[k] = [v.x, v.y, v.z, 0.0];
    }

    // GGX prefiltered specular, one mip per roughness step.
    let mut spec = Vec::new();
    for lvl in 0..SPEC_LEVELS {
        let (w, h) = ((SPEC_W >> lvl).max(1) as usize, (SPEC_H >> lvl).max(1) as usize);
        let rough = lvl as f32 / (SPEC_LEVELS - 1) as f32;
        let samples: u32 = if lvl == 0 { 1 } else { 160 };
        for y in 0..h {
            for x in 0..w {
                let n = dir_of((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                let mut acc = Vec3::ZERO;
                if lvl == 0 {
                    acc = Vec3::from(src.sample(n));
                } else {
                    let mut wsum = 0.0;
                    for i in 0..samples {
                        let hv = importance_ggx(i, samples, rough.max(0.05), n);
                        let l = (2.0 * n.dot(hv) * hv - n).normalize();
                        let nl = n.dot(l);
                        if nl > 0.0 {
                            acc += Vec3::from(src.sample(l)) * nl;
                            wsum += nl;
                        }
                    }
                    acc /= wsum.max(1e-4);
                }
                f16_bytes(&mut spec, acc.to_array());
            }
        }
    }

    // BRDF LUT.
    let mut lut = Vec::with_capacity((LUT_SIZE * LUT_SIZE * 4) as usize);
    for j in 0..LUT_SIZE {
        let rough = ((j as f32 + 0.5) / LUT_SIZE as f32).max(0.03);
        for i in 0..LUT_SIZE {
            let nv = ((i as f32 + 0.5) / LUT_SIZE as f32).max(0.001);
            let v = Vec3::new((1.0 - nv * nv).sqrt(), 0.0, nv);
            let (mut a, mut b) = (0.0f32, 0.0f32);
            let ns = 96;
            for s in 0..ns {
                let h = importance_ggx(s, ns, rough, Vec3::Z);
                let l = (2.0 * v.dot(h) * h - v).normalize();
                let (nl, nh, vh) = (l.z.max(0.0), h.z.max(0.0), v.dot(h).max(0.0));
                if nl > 0.0 {
                    let k = rough * rough / 2.0;
                    let g = (nl / (nl * (1.0 - k) + k)) * (nv / (nv * (1.0 - k) + k));
                    let g_vis = g * vh / (nh * nv).max(1e-4);
                    let fc = (1.0 - vh).powi(5);
                    a += (1.0 - fc) * g_vis;
                    b += fc * g_vis;
                }
            }
            lut.extend_from_slice(&[(a / ns as f32 * 255.0).clamp(0.0, 255.0) as u8, (b / ns as f32 * 255.0).clamp(0.0, 255.0) as u8, 0, 255]);
        }
    }
    Ok(Env { sh: sh_out, key_dir, spec, lut })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_mapping_roundtrips() {
        for (u, v) in [(0.1, 0.2), (0.5, 0.5), (0.9, 0.8), (0.33, 0.66)] {
            let (u2, v2) = uv_of(dir_of(u, v));
            assert!((u - u2).abs() < 1e-4 && (v - v2).abs() < 1e-4, "{u} {v} -> {u2} {v2}");
        }
    }

    #[test]
    fn builds_from_bundled_hdri() {
        let bytes = include_bytes!("../../../assets/env/studio_small_03_1k.hdr");
        let e = build(bytes).expect("hdri");
        assert!(e.key_dir.y > 0.0, "studio key light comes from above: {:?}", e.key_dir);
        assert!(e.sh[0][0] > 0.05, "non-black irradiance");
        let expect: usize = (0..SPEC_LEVELS).map(|l| ((SPEC_W >> l) * (SPEC_H >> l)) as usize * 8).sum();
        assert_eq!(e.spec.len(), expect);
        assert_eq!(e.lut.len(), (LUT_SIZE * LUT_SIZE * 4) as usize);
    }
}
