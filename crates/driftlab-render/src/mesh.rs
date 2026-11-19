//! Procedural mesh generation (no external assets, original designs).

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Default, Clone)]
pub struct Mesh {
    pub verts: Vec<Vertex>,
    pub idx: Vec<u32>,
}

impl Mesh {
    /// Superellipsoid with half-extents `r` and boxiness exponents (`e` = 1 → ellipsoid, small → box).
    pub fn superellipsoid(r: [f32; 3], e_ns: f32, e_ew: f32, rings: u32, segs: u32) -> Self {
        let mut m = Mesh::default();
        let sgnpow = |v: f32, p: f32| v.signum() * v.abs().powf(p);
        for i in 0..=rings {
            let v = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / rings as f32;
            for j in 0..=segs {
                let u = -std::f32::consts::PI + std::f32::consts::TAU * j as f32 / segs as f32;
                let (cv, sv, cu, su) = (v.cos(), v.sin(), u.cos(), u.sin());
                let p = [r[0] * sgnpow(cv, e_ns) * sgnpow(cu, e_ew), r[1] * sgnpow(sv, e_ns), r[2] * sgnpow(cv, e_ns) * sgnpow(su, e_ew)];
                // Normal of a superellipsoid: gradient of the implicit surface.
                let n = [
                    sgnpow(cv, 2.0 - e_ns) * sgnpow(cu, 2.0 - e_ew) / r[0],
                    sgnpow(sv, 2.0 - e_ns) / r[1],
                    sgnpow(cv, 2.0 - e_ns) * sgnpow(su, 2.0 - e_ew) / r[2],
                ];
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                m.verts.push(Vertex { pos: p, normal: [n[0] / l, n[1] / l, n[2] / l], uv: [0.0, 0.0] });
            }
        }
        for i in 0..rings {
            for j in 0..segs {
                let a = i * (segs + 1) + j;
                let b = a + segs + 1;
                m.idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        m
    }

    pub fn sphere(r: f32) -> Self {
        Self::superellipsoid([r, r, r], 1.0, 1.0, 16, 32)
    }

    /// Rounded box.
    pub fn rbox(half: [f32; 3]) -> Self {
        Self::superellipsoid(half, 0.25, 0.25, 14, 28)
    }

    /// Capped cylinder along +Y with smooth side normals.
    pub fn cylinder(radius: f32, height: f32, segs: u32) -> Self {
        let mut m = Mesh::default();
        let h = height / 2.0;
        for j in 0..=segs {
            let a = std::f32::consts::TAU * j as f32 / segs as f32;
            let (c, s) = (a.cos(), a.sin());
            m.verts.push(Vertex { pos: [radius * c, -h, radius * s], normal: [c, 0.0, s], uv: [0.0, 0.0] });
            m.verts.push(Vertex { pos: [radius * c, h, radius * s], normal: [c, 0.0, s], uv: [0.0, 0.0] });
        }
        for j in 0..segs {
            let a = j * 2;
            m.idx.extend_from_slice(&[a, a + 1, a + 2, a + 2, a + 1, a + 3]);
        }
        for (y, ny) in [(h, 1.0f32), (-h, -1.0)] {
            let c = m.verts.len() as u32;
            m.verts.push(Vertex { pos: [0.0, y, 0.0], normal: [0.0, ny, 0.0], uv: [0.0, 0.0] });
            for j in 0..=segs {
                let a = std::f32::consts::TAU * j as f32 / segs as f32;
                m.verts.push(Vertex { pos: [radius * a.cos(), y, radius * a.sin()], normal: [0.0, ny, 0.0], uv: [0.0, 0.0] });
            }
            for j in 0..segs {
                if ny > 0.0 {
                    m.idx.extend_from_slice(&[c, c + 2 + j, c + 1 + j]);
                } else {
                    m.idx.extend_from_slice(&[c, c + 1 + j, c + 2 + j]);
                }
            }
        }
        m
    }

    /// Mushroom-shaped stick cap: cylinder shaft + dished disc.
    pub fn stick_cap() -> Self {
        let mut m = Self::cylinder(0.11, 0.30, 24);
        let mut disc = Self::superellipsoid([0.25, 0.06, 0.25], 0.5, 1.0, 10, 28);
        for v in &mut disc.verts {
            v.pos[1] += 0.20;
        }
        m.append(&disc);
        m
    }

    pub fn append(&mut self, o: &Mesh) {
        let base = self.verts.len() as u32;
        self.verts.extend_from_slice(&o.verts);
        self.idx.extend(o.idx.iter().map(|i| i + base));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(m: &Mesh) {
        assert!(!m.verts.is_empty() && m.idx.len() % 3 == 0);
        assert!(m.idx.iter().all(|&i| (i as usize) < m.verts.len()));
        assert!(m.verts.iter().all(|v| v.pos.iter().chain(v.normal.iter()).all(|c| c.is_finite())));
        assert!(m.verts.iter().all(|v| {
            let l = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            (l - 1.0).abs() < 1e-3
        }));
    }

    #[test]
    fn primitives_are_valid() {
        valid(&Mesh::sphere(1.0));
        valid(&Mesh::rbox([1.0, 0.5, 0.7]));
        valid(&Mesh::cylinder(0.5, 1.0, 16));
        valid(&Mesh::stick_cap());
    }
}
