//! Offscreen wgpu renderer for stylised controller models.
//!
//! The renderer draws into its own 4× MSAA colour target (with depth) and resolves into a texture
//! that the GUI shows as a regular image. This keeps it independent from the UI's render pass setup.

pub mod mesh;
pub mod model;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mesh::Vertex;
use model::{Anim, ModelKind, Paint, Part};
use std::collections::HashMap;

pub use model::ModelKind as Kind;

pub const MAX_TILT_DEG: f32 = 25.0;
const SAMPLES: u32 = 4;
const OBJ_STRIDE: u64 = 256;
const MAX_OBJECTS: u64 = 64;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Colours in gamma (sRGB-like) 0..1 space.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub body: [f32; 3],
    pub dark: [f32; 3],
    pub metal: [f32; 3],
    pub accent: [f32; 3],
}

impl Palette {
    pub fn for_theme(key: &str) -> Self {
        let c = |h: u32| [((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0];
        match key {
            "ps5" => Self { body: c(0xEDEFF3), dark: c(0x1B1E26), metal: c(0x9AA3B2), accent: c(0x2D6BFF) },
            "ps5_edge" => Self { body: c(0xE4E7ED), dark: c(0x14161C), metal: c(0x8E97A6), accent: c(0x2D6BFF) },
            "ps4" => Self { body: c(0x20242C), dark: c(0x14171D), metal: c(0x5A6270), accent: c(0x1E90FF) },
            "xbox" => Self { body: c(0x1C1E1C), dark: c(0x101210), metal: c(0x6C726C), accent: c(0x16C60C) },
            _ => Self { body: c(0x3A3D42), dark: c(0x1E2024), metal: c(0xA9AEB6), accent: c(0xC9CED6) },
        }
    }
}

/// Everything that changes per frame.
#[derive(Debug, Clone)]
pub struct Pose {
    pub left: (f32, f32),
    pub right: (f32, f32),
    pub l2: f32,
    pub r2: f32,
    pub pressed: Vec<String>,
    /// Extra model tilt (pitch, roll) in radians, e.g. from gyro.
    pub tilt: (f32, f32),
    pub touch: Vec<(f32, f32)>,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    /// Vertical offset / scale for the fly-in animation.
    pub offset_y: f32,
    pub scale: f32,
    /// 0..1 sweep of a light across the housing (welcome sequence).
    pub sweep: f32,
    pub palette: Palette,
    pub lightbar: [f32; 3],
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            left: (0.0, 0.0),
            right: (0.0, 0.0),
            l2: 0.0,
            r2: 0.0,
            pressed: vec![],
            tilt: (0.0, 0.0),
            touch: vec![],
            yaw: 0.0,
            pitch: 0.55,
            zoom: 1.0,
            offset_y: 0.0,
            scale: 1.0,
            sweep: -1.0,
            palette: Palette::for_theme("neutral"),
            lightbar: [0.1, 0.4, 1.0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    accent: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectU {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    params: [f32; 4],
}

struct GpuPart {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    n: u32,
}

struct Targets {
    size: (u32, u32),
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    obj_buf: wgpu::Buffer,
    obj_bg: wgpu::BindGroup,
    models: HashMap<u8, (Vec<Part>, Vec<GpuPart>)>,
    targets: Option<Targets>,
    pub generation: u64,
}

fn kind_id(k: ModelKind) -> u8 {
    k as u8
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("driftlab"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let g_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let o_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<ObjectU>() as u64),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout"),
            bind_group_layouts: &[Some(&g_layout), Some(&o_layout)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("controller"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: SAMPLES, mask: !0, alpha_to_coverage_enabled: false },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &g_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });
        let obj_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("objects"),
            size: OBJ_STRIDE * MAX_OBJECTS,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let obj_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &o_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &obj_buf,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ObjectU>() as u64),
                }),
            }],
        });
        Self { device, queue, pipeline, globals_buf, globals_bg, obj_buf, obj_bg, models: HashMap::new(), targets: None, generation: 0 }
    }

    fn ensure_model(&mut self, kind: ModelKind) {
        use wgpu::util::DeviceExt;
        self.models.entry(kind_id(kind)).or_insert_with(|| {
            let parts = model::build(kind);
            let gpu = parts
                .iter()
                .map(|p| GpuPart {
                    vb: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(p.name),
                        contents: bytemuck::cast_slice(&p.mesh.verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                    ib: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(p.name),
                        contents: bytemuck::cast_slice(&p.mesh.idx),
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                    n: p.mesh.idx.len() as u32,
                })
                .collect();
            (parts, gpu)
        });
    }

    fn ensure_targets(&mut self, size: (u32, u32)) -> bool {
        let size = (size.0.clamp(16, 4096), size.1.clamp(16, 4096));
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return false;
        }
        let ext = wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 };
        let mk = |format, samples, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: ext,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let msaa = mk(COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
        let depth = mk(DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
        let resolve = mk(
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        );
        let resolve_view = resolve.create_view(&Default::default());
        self.targets = Some(Targets { size, msaa, depth, resolve, resolve_view });
        self.generation += 1;
        true
    }

    /// The resolved colour texture view (valid after `render`).
    pub fn view(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|t| &t.resolve_view)
    }

    pub fn texture(&self) -> Option<&wgpu::Texture> {
        self.targets.as_ref().map(|t| &t.resolve)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Render the model. Returns true if the target was (re)created (caller must re-register the texture).
    pub fn render(&mut self, kind: ModelKind, pose: &Pose, size: (u32, u32)) -> bool {
        self.ensure_model(kind);
        let recreated = self.ensure_targets(size);
        let Some(t) = &self.targets else { return false };
        let aspect = t.size.0 as f32 / t.size.1 as f32;
        let dist = 5.6 / pose.zoom.clamp(0.4, 3.0);
        let eye = Vec3::new(0.0, dist * pose.pitch.sin(), dist * pose.pitch.cos());
        let view = Mat4::look_at_rh(eye, Vec3::new(0.0, pose.offset_y * 0.0, 0.0), Vec3::Y);
        let proj = Mat4::perspective_rh(32f32.to_radians(), aspect, 0.1, 50.0);
        let root = Mat4::from_translation(Vec3::new(0.0, pose.offset_y, 0.0))
            * Mat4::from_rotation_y(pose.yaw)
            * Mat4::from_rotation_x(pose.tilt.0)
            * Mat4::from_rotation_z(pose.tilt.1)
            * Mat4::from_scale(Vec3::splat(pose.scale.max(0.01)));
        let g = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, 1.0],
            accent: [pose.palette.accent[0], pose.palette.accent[1], pose.palette.accent[2], 1.0],
        };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&g));

        let (parts, gpu) = &self.models[&kind_id(kind)];
        let mut data = vec![0u8; (OBJ_STRIDE * parts.len().min(MAX_OBJECTS as usize) as u64) as usize];
        let pressed = |n: &str| pose.pressed.iter().any(|p| p == n);
        for (i, p) in parts.iter().enumerate().take(MAX_OBJECTS as usize) {
            let mut m = p.transform;
            let mut glow = 0.0;
            match &p.anim {
                Anim::Stick { right } => {
                    let (x, y) = if *right { pose.right } else { pose.left };
                    let a = MAX_TILT_DEG.to_radians();
                    let rot = Mat4::from_rotation_z(-x.clamp(-1.0, 1.0) * a) * Mat4::from_rotation_x(-y.clamp(-1.0, 1.0) * a);
                    m = Mat4::from_translation(p.pivot) * rot * Mat4::from_translation(-p.pivot) * m;
                    let mag = (x * x + y * y).sqrt().min(1.0);
                    glow = mag * 0.25;
                }
                Anim::Trigger { right } => {
                    let v = if *right { pose.r2 } else { pose.l2 }.clamp(0.0, 1.0);
                    let pv = p.pivot + Vec3::new(0.0, 0.1, -0.1);
                    m = Mat4::from_translation(pv) * Mat4::from_rotation_x(v * 0.55) * Mat4::from_translation(-pv) * m;
                    glow = v * 0.8;
                }
                Anim::Button(name) => {
                    if pressed(name) {
                        m = Mat4::from_translation(Vec3::new(0.0, -0.035, 0.0)) * m;
                        glow = 1.0;
                    }
                }
                Anim::Touchpad => {
                    if !pose.touch.is_empty() {
                        glow = 0.5;
                    }
                }
                Anim::None => {}
            }
            let pal = &pose.palette;
            let (base, metallic, rough, emissive) = match p.paint {
                Paint::Body => (pal.body, 0.0, 0.75, 0.0),
                Paint::Dark => (pal.dark, 0.0, 0.55, 0.0),
                Paint::Metal => (pal.metal, 0.8, 0.35, 0.0),
                Paint::Accent => (pal.accent, 0.1, 0.4, 0.35),
                Paint::Glow => (pose.lightbar, 0.0, 0.3, 1.6),
            };
            // Welcome sweep: a bright band travelling across the housing.
            let sweep_boost = if pose.sweep >= 0.0 && matches!(p.paint, Paint::Body | Paint::Dark) {
                let x = p.pivot.x / 1.4 * 0.5 + 0.5;
                (1.0 - ((x - pose.sweep) * 6.0).abs()).max(0.0) * 0.6
            } else {
                0.0
            };
            let u = ObjectU {
                model: (root * m).to_cols_array_2d(),
                color: [base[0], base[1], base[2], 1.0],
                params: [emissive + sweep_boost, metallic, rough, glow],
            };
            let o = (i as u64 * OBJ_STRIDE) as usize;
            data[o..o + std::mem::size_of::<ObjectU>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
        self.queue.write_buffer(&self.obj_buf, 0, &data);

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("driftlab") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("controller"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.msaa,
                    depth_slice: None,
                    resolve_target: Some(&t.resolve_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            for (i, gp) in gpu.iter().enumerate().take(MAX_OBJECTS as usize) {
                pass.set_bind_group(1, &self.obj_bg, &[(i as u64 * OBJ_STRIDE) as u32]);
                pass.set_vertex_buffer(0, gp.vb.slice(..));
                pass.set_index_buffer(gp.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..gp.n, 0, 0..1);
            }
        }
        self.queue.submit(Some(enc.finish()));
        recreated
    }

    /// Read back the rendered image as RGBA8 (used for screenshots/tests). Blocks.
    pub fn read_rgba(&self) -> Option<(u32, u32, Vec<u8>)> {
        let t = self.targets.as_ref()?;
        let (w, h) = t.size;
        let bpr = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &t.resolve,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(enc.finish()));
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range().ok()?;
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let s = (row * bpr) as usize;
            out.extend_from_slice(&data[s..s + (w * 4) as usize]);
        }
        drop(data);
        buf.unmap();
        Some((w, h, out))
    }
}
