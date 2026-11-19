//! Offscreen wgpu product renderer for the controller models.
//!
//! * models: glTF/GLB files prepared by `tools/models/*.py` (fallback: procedural parts)
//! * shading: glTF metallic-roughness PBR (GGX), HDRI image-based lighting, PCF shadow map, ACES tone mapping
//! * scene: soft ground shadow + reflection, 4× MSAA, resolved into a texture shown by the GUI

pub mod env;
pub mod gltf_load;
pub mod mesh;
pub mod model;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mesh::Vertex;
use model::{Anim, MatKind, ModelKind, Part};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub use model::ModelKind as Kind;

pub const MAX_TILT_DEG: f32 = 25.0;
const SAMPLES: u32 = 4;
const OBJ_STRIDE: u64 = 256;
const MAX_OBJECTS: u64 = 2048;
const SHADOW_SIZE: u32 = 2048;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const HDRI: &[u8] = include_bytes!("../../../assets/env/studio_small_03_1k.hdr");

/// Embedded GLB for a model, if it has been built.
fn glb_bytes(kind: ModelKind) -> Option<&'static [u8]> {
    match kind {
        ModelKind::DualSense => Some(include_bytes!("../../../assets/models/dualsense.glb")),
        ModelKind::DualSenseEdge => Some(include_bytes!("../../../assets/models/dualsense_edge.glb")),
        ModelKind::DualShock4 => Some(include_bytes!("../../../assets/models/dualshock4.glb")),
        ModelKind::Xbox => Some(include_bytes!("../../../assets/models/xbox.glb")),
        ModelKind::XboxElite => Some(include_bytes!("../../../assets/models/xbox_elite.glb")),
        _ => None,
    }
}

/// Theme accent (gamma RGB 0..1).
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
    pub offset_y: f32,
    pub scale: f32,
    /// 0..1 sweep of a light across the housing (welcome sequence); < 0 = off.
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
            pitch: 0.72,
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
    light_vp: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    accent: [f32; 4],
    misc: [f32; 4],
    key_dir: [f32; 4],
    sh: [[f32; 4]; 9],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectU {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    params: [f32; 4],
    extra: [f32; 4],
    emissive: [f32; 4],
    uvxf: [f32; 4],
}

struct GpuPart {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    n: u32,
    mat_bg: usize,
}

struct LoadedModel {
    parts: Vec<Part>,
    gpu: Vec<GpuPart>,
    mat_bgs: Vec<wgpu::BindGroup>,
    ground_y: f32,
    radius: f32,
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
    main_pipe: wgpu::RenderPipeline,
    reflect_pipe: wgpu::RenderPipeline,
    blend_pipe: wgpu::RenderPipeline,
    ground_pipe: wgpu::RenderPipeline,
    shadow_pipe: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    globals_shadow_bg: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    obj_buf: wgpu::Buffer,
    obj_bg: wgpu::BindGroup,
    mat_layout: wgpu::BindGroupLayout,
    mat_smp: wgpu::Sampler,
    dummies: [wgpu::TextureView; 2], // white, flat normal
    env: env::Env,
    models: HashMap<u8, LoadedModel>,
    targets: Option<Targets>,
    /// Linear exposure before tone mapping.
    pub exposure: f32,
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
        let uni = |dynamic| wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        };
        let tex2d = || wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let frag = wgpu::ShaderStages::FRAGMENT;
        let g_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: uni(false), count: None },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: frag,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: frag,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: frag, ty: tex2d(), count: None },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: frag,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: frag, ty: tex2d(), count: None },
            ],
        });
        let gs_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals_shadow"),
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX, ty: uni(false), count: None }],
        });
        let o_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: uni(true),
                count: None,
            }],
        });
        let mut m_entries: Vec<wgpu::BindGroupLayoutEntry> =
            (0..5).map(|i| wgpu::BindGroupLayoutEntry { binding: i, visibility: frag, ty: tex2d(), count: None }).collect();
        m_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility: frag,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let mat_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("material"), entries: &m_entries });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout"),
            bind_group_layouts: &[Some(&g_layout), Some(&o_layout), Some(&mat_layout)],
            immediate_size: 0,
        });
        let layout_shadow = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout_shadow"),
            bind_group_layouts: &[Some(&gs_layout), Some(&o_layout)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let vbuf = || wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        };
        let ms = wgpu::MultisampleState { count: SAMPLES, mask: !0, alpha_to_coverage_enabled: false };
        let depth_state = |write, cmp| {
            Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write),
                depth_compare: Some(cmp),
                stencil: Default::default(),
                bias: Default::default(),
            })
        };
        let pm = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let premult = wgpu::BlendState { color: pm, alpha: pm };
        let vb_layouts = [Some(vbuf())];
        let make = |label: &str, vs: &str, fs: &str, cull, depth, blend: Option<wgpu::BlendState>, buffers: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: if buffers { &vb_layouts } else { &[] },
                },
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
                depth_stencil: depth,
                multisample: ms,
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let main_pipe = make("main", "vs", "fs", Some(wgpu::Face::Back), depth_state(true, wgpu::CompareFunction::Less), None, true);
        let reflect_pipe = make(
            "reflect",
            "vs",
            "fs_reflect",
            Some(wgpu::Face::Front),
            depth_state(false, wgpu::CompareFunction::Always),
            Some(premult),
            true,
        );
        let blend_pipe = make(
            "blend",
            "vs",
            "fs_blend",
            None,
            depth_state(false, wgpu::CompareFunction::Less),
            Some(wgpu::BlendState::ALPHA_BLENDING),
            true,
        );
        let ground_pipe =
            make("ground", "vs_ground", "fs_ground", None, depth_state(false, wgpu::CompareFunction::Always), Some(premult), false);
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&layout_shadow),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Some(vbuf())],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&Default::default());
        let shadow_smp = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Image-based lighting resources.
        let env = env::build(HDRI).unwrap_or_else(|e| {
            eprintln!("driftlab-render: HDRI failed ({e}); using a flat environment");
            flat_env()
        });
        let spec_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("env_spec"),
                size: wgpu::Extent3d { width: env::SPEC_W, height: env::SPEC_H, depth_or_array_layers: 1 },
                mip_level_count: env::SPEC_LEVELS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &env.spec,
        );
        let lut_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("brdf_lut"),
                size: wgpu::Extent3d { width: env::LUT_SIZE, height: env::LUT_SIZE, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &env.lut,
        );
        let env_smp = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let spec_view = spec_tex.create_view(&Default::default());
        let lut_view = lut_tex.create_view(&Default::default());
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &g_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_smp) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&spec_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&env_smp) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&lut_view) },
            ],
        });
        let globals_shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &gs_layout,
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
        let mat_smp = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let dummy = |rgba: [u8; 4]| {
            device
                .create_texture_with_data(
                    &queue,
                    &wgpu::TextureDescriptor {
                        label: Some("dummy"),
                        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    &rgba,
                )
                .create_view(&Default::default())
        };
        let dummies = [dummy([255, 255, 255, 255]), dummy([128, 128, 255, 255])];
        Self {
            device,
            queue,
            main_pipe,
            reflect_pipe,
            blend_pipe,
            ground_pipe,
            shadow_pipe,
            globals_buf,
            globals_bg,
            globals_shadow_bg,
            shadow_view,
            obj_buf,
            obj_bg,
            mat_layout,
            mat_smp,
            dummies,
            env,
            models: HashMap::new(),
            targets: None,
            exposure: 0.15,
            generation: 0,
        }
    }

    /// Replace the model of `kind` with GLB bytes (used by tools/tests; the app embeds its models).
    pub fn load_model_bytes(&mut self, kind: ModelKind, bytes: &[u8]) -> Result<(), String> {
        let l = gltf_load::load(bytes)?;
        let m = self.upload(l.parts, l.images, l.ground_y, l.radius);
        self.models.insert(kind_id(kind), m);
        Ok(())
    }

    fn upload(&self, parts: Vec<Part>, images: Vec<gltf_load::Image>, ground_y: f32, radius: f32) -> LoadedModel {
        // Textures: one GPU texture per decoded image, with a CPU-built mip chain.
        let views: Vec<wgpu::TextureView> = images
            .iter()
            .map(|im| {
                let mut chain = vec![gltf_load::Image { width: im.width, height: im.height, rgba: im.rgba.clone(), srgb: im.srgb }];
                while chain.last().is_some_and(|l| l.width > 1 || l.height > 1) {
                    let n = gltf_load::halve(chain.last().expect("non-empty"));
                    chain.push(n);
                }
                let mut data = Vec::new();
                for l in &chain {
                    data.extend_from_slice(&l.rgba);
                }
                self.device
                    .create_texture_with_data(
                        &self.queue,
                        &wgpu::TextureDescriptor {
                            label: Some("model_tex"),
                            size: wgpu::Extent3d { width: im.width, height: im.height, depth_or_array_layers: 1 },
                            mip_level_count: chain.len() as u32,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: if im.srgb { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm },
                            usage: wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        },
                        wgpu::util::TextureDataOrder::LayerMajor,
                        &data,
                    )
                    .create_view(&Default::default())
            })
            .collect();
        let mut mat_bgs: Vec<wgpu::BindGroup> = Vec::new();
        let mut keys: Vec<[Option<usize>; 5]> = Vec::new();
        let mut gpu = Vec::new();
        for p in &parts {
            let t = &p.material.tex;
            let key = [t.base, t.metal_rough, t.normal, t.occlusion, t.emissive];
            let idx = match keys.iter().position(|k| *k == key) {
                Some(i) => i,
                None => {
                    let pick = |o: Option<usize>, d: usize| o.and_then(|i| views.get(i)).unwrap_or(&self.dummies[d]);
                    let entries = [
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(pick(key[0], 0)) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(pick(key[1], 0)) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(pick(key[2], 1)) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(pick(key[3], 0)) },
                        wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(pick(key[4], 0)) },
                        wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.mat_smp) },
                    ];
                    mat_bgs.push(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("material"),
                        layout: &self.mat_layout,
                        entries: &entries,
                    }));
                    keys.push(key);
                    keys.len() - 1
                }
            };
            gpu.push(GpuPart {
                vb: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&p.name),
                    contents: bytemuck::cast_slice(&p.mesh.verts),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                ib: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&p.name),
                    contents: bytemuck::cast_slice(&p.mesh.idx),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                n: p.mesh.idx.len() as u32,
                mat_bg: idx,
            });
        }
        LoadedModel { parts, gpu, mat_bgs, ground_y, radius }
    }

    fn ensure_model(&mut self, kind: ModelKind) {
        if self.models.contains_key(&kind_id(kind)) {
            return;
        }
        let m = match glb_bytes(kind).map(gltf_load::load) {
            Some(Ok(l)) => self.upload(l.parts, l.images, l.ground_y, l.radius),
            other => {
                if let Some(Err(e)) = other {
                    eprintln!("driftlab-render: GLB load failed ({e}); using procedural fallback");
                }
                let mut parts = model::build(kind);
                let (g, r) = gltf_load::normalize(&mut parts);
                self.upload(parts, Vec::new(), g, r)
            }
        };
        self.models.insert(kind_id(kind), m);
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
        let model = &self.models[&kind_id(kind)];
        let aspect = t.size.0 as f32 / t.size.1 as f32;
        let dist = model.radius * 2.9 / pose.zoom.clamp(0.4, 3.0);
        let eye = Vec3::new(0.0, dist * pose.pitch.sin(), dist * pose.pitch.cos());
        let view = Mat4::look_at_rh(eye, Vec3::new(0.0, -0.05, 0.0), Vec3::Y);
        let proj = Mat4::perspective_rh(30f32.to_radians(), aspect, 0.5, 60.0);
        let root = Mat4::from_translation(Vec3::new(0.0, pose.offset_y, 0.0))
            * Mat4::from_rotation_y(pose.yaw)
            * Mat4::from_rotation_x(pose.tilt.0)
            * Mat4::from_rotation_z(pose.tilt.1)
            * Mat4::from_scale(Vec3::splat(pose.scale.max(0.01)));
        // A hair below the lowest vertex: contact points would otherwise z-fight with their own mirrored reflection.
        let ground_y = (model.ground_y - model.radius * 0.006) * pose.scale.max(0.01) + pose.offset_y;
        // Key-light shadow map covering the model.
        let key = self.env.key_dir;
        let r = model.radius * 1.5;
        let light_view = Mat4::look_at_rh(key * model.radius * 4.0, Vec3::ZERO, Vec3::Y);
        let light_proj = Mat4::orthographic_rh(-r, r, -r, r, 0.5, model.radius * 9.0);
        let acc = pose.palette.accent;
        let acc_lin = [acc[0].powf(2.2), acc[1].powf(2.2), acc[2].powf(2.2)];
        let g = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            light_vp: (light_proj * light_view).to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, 1.0],
            accent: [acc_lin[0], acc_lin[1], acc_lin[2], 1.0],
            misc: [ground_y, 0.10, 0.85, self.exposure],
            key_dir: [key.x, key.y, key.z, 0.0],
            sh: self.env.sh,
        };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&g));

        let parts = &model.parts;
        let n = parts.len().min((MAX_OBJECTS / 2) as usize);
        let mut data = vec![0u8; (OBJ_STRIDE * (2 * n) as u64) as usize];
        let pressed = |name: &str| {
            pose.pressed.iter().any(|p| {
                p == name
                    || (name == "l1" && p == "lb")
                    || (name == "r1" && p == "rb")
                    || (name == "home" && p == "guide")
                    || (name == "guide" && p == "home")
                    || (name == "create" && (p == "share" || p == "view"))
                    || (name == "options" && p == "menu")
                    || (name == "view" && p == "create")
                    || (name == "menu" && p == "options")
                    || (name == "share" && p == "create")
            })
        };
        // Matte / satin plastics: the downloaded textures are glossier than the real products.
        let rough_min = match kind {
            ModelKind::DualShock4 => 0.6,
            ModelKind::DualSense | ModelKind::DualSenseEdge => 0.34,
            ModelKind::Xbox | ModelKind::XboxElite => 0.5,
            _ => 0.0,
        };
        let mirror = Mat4::from_translation(Vec3::new(0.0, ground_y, 0.0))
            * Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0))
            * Mat4::from_translation(Vec3::new(0.0, -ground_y, 0.0));
        for (i, p) in parts.iter().enumerate().take(n) {
            let mut m = p.transform;
            let mut glow = 0.0;
            let delta = match &p.anim {
                Anim::Stick { right } => {
                    let (x, y) = if *right { pose.right } else { pose.left };
                    let a = MAX_TILT_DEG.to_radians();
                    glow = (x * x + y * y).sqrt().min(1.0) * 0.12;
                    Some(
                        Mat4::from_translation(p.pivot)
                            * Mat4::from_rotation_z(-x.clamp(-1.0, 1.0) * a)
                            * Mat4::from_rotation_x(y.clamp(-1.0, 1.0) * a)
                            * Mat4::from_translation(-p.pivot),
                    )
                }
                Anim::Trigger { right } => {
                    let v = if *right { pose.r2 } else { pose.l2 }.clamp(0.0, 1.0);
                    glow = v * 0.12;
                    Some(Mat4::from_translation(p.pivot) * Mat4::from_rotation_x(-v * 0.42) * Mat4::from_translation(-p.pivot))
                }
                Anim::Button(name) => {
                    if pressed(name) {
                        glow = 0.8;
                        Some(Mat4::from_translation(Vec3::new(0.0, -0.045, 0.0)))
                    } else {
                        None
                    }
                }
                Anim::Touchpad => {
                    if !pose.touch.is_empty() {
                        glow = 0.35;
                    }
                    None
                }
                Anim::None => None,
            };
            if let Some(d) = delta {
                m = d * m;
            }
            let mat = &p.material;
            let (emissive_rgb, emissive_k, kind_f) = match mat.kind {
                MatKind::Normal => (mat.emissive_factor, 0.0, 0.0),
                MatKind::Accent => ([0.0; 3], 0.0, 1.0),
                MatKind::Grip => ([0.0; 3], 0.0, 3.0),
                MatKind::Glow => (acc_lin, mat.emissive.max(2.5), 2.0),
                MatKind::Guide => ([0.6 + 0.4 * acc_lin[0], 0.6 + 0.4 * acc_lin[1], 0.6 + 0.4 * acc_lin[2]], mat.emissive.max(1.8), 4.0),
            };
            let sweep_boost = if pose.sweep >= 0.0 && mat.kind == MatKind::Normal {
                let x = (p.pivot.x / 2.6 + 0.5).clamp(0.0, 1.0);
                (1.0 - ((x - pose.sweep) * 6.0).abs()).max(0.0) * 0.4
            } else {
                0.0
            };
            let has_normal = if mat.tex.normal.is_some() { 1.0 } else { 0.0 };
            let write = |slot: usize, model_m: Mat4, data: &mut Vec<u8>| {
                let u = ObjectU {
                    model: (root * model_m).to_cols_array_2d(),
                    color: [mat.base[0], mat.base[1], mat.base[2], mat.alpha],
                    params: [emissive_k + sweep_boost, mat.metallic, mat.rough, glow],
                    extra: [kind_f, has_normal, mat.normal_scale, rough_min],
                    emissive: [emissive_rgb[0], emissive_rgb[1], emissive_rgb[2], 1.0],
                    uvxf: mat.uv_xf,
                };
                let o = slot * OBJ_STRIDE as usize;
                data[o..o + std::mem::size_of::<ObjectU>()].copy_from_slice(bytemuck::bytes_of(&u));
            };
            write(i, m, &mut data);
            write(n + i, mirror * root.inverse() * (root * m), &mut data);
        }
        self.queue.write_buffer(&self.obj_buf, 0, &data);

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("driftlab") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipe);
            pass.set_bind_group(0, &self.globals_shadow_bg, &[]);
            for (i, gp) in model.gpu.iter().enumerate().take(n) {
                pass.set_bind_group(1, &self.obj_bg, &[(i as u64 * OBJ_STRIDE) as u32]);
                pass.set_vertex_buffer(0, gp.vb.slice(..));
                pass.set_index_buffer(gp.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..gp.n, 0, 0..1);
            }
        }
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
            pass.set_bind_group(0, &self.globals_bg, &[]);
            // 1. reflection (mirrored model, faded)
            pass.set_pipeline(&self.reflect_pipe);
            for (i, gp) in model.gpu.iter().enumerate().take(n) {
                pass.set_bind_group(1, &self.obj_bg, &[((n + i) as u64 * OBJ_STRIDE) as u32]);
                pass.set_bind_group(2, &model.mat_bgs[gp.mat_bg], &[]);
                pass.set_vertex_buffer(0, gp.vb.slice(..));
                pass.set_index_buffer(gp.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..gp.n, 0, 0..1);
            }
            // 2. ground shadow
            pass.set_pipeline(&self.ground_pipe);
            pass.set_bind_group(1, &self.obj_bg, &[0]);
            if let Some(first) = model.mat_bgs.first() {
                pass.set_bind_group(2, first, &[]);
            }
            pass.draw(0..6, 0..1);
            // 3. the controller (opaque parts first, then alpha-blended parts back to front)
            pass.set_pipeline(&self.main_pipe);
            for (i, gp) in model.gpu.iter().enumerate().take(n) {
                if model.parts[i].material.blend {
                    continue;
                }
                pass.set_bind_group(1, &self.obj_bg, &[(i as u64 * OBJ_STRIDE) as u32]);
                pass.set_bind_group(2, &model.mat_bgs[gp.mat_bg], &[]);
                pass.set_vertex_buffer(0, gp.vb.slice(..));
                pass.set_index_buffer(gp.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..gp.n, 0, 0..1);
                // 4. alpha-blended parts, sorted far to near
                let mut order: Vec<usize> = (0..n).filter(|i| model.parts[*i].material.blend).collect();
                order.sort_by(|a, b| {
                    let d = |i: usize| (root * model.parts[i].transform).transform_point3(Vec3::ZERO).distance(eye);
                    d(*b).total_cmp(&d(*a))
                });
                pass.set_pipeline(&self.blend_pipe);
                for i in order {
                    let gp = &model.gpu[i];
                    pass.set_bind_group(1, &self.obj_bg, &[(i as u64 * OBJ_STRIDE) as u32]);
                    pass.set_bind_group(2, &model.mat_bgs[gp.mat_bg], &[]);
                    pass.set_vertex_buffer(0, gp.vb.slice(..));
                    pass.set_index_buffer(gp.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..gp.n, 0, 0..1);
                }
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

/// Neutral grey environment used only if the bundled HDRI cannot be decoded.
fn flat_env() -> env::Env {
    let mut spec = Vec::new();
    for l in 0..env::SPEC_LEVELS {
        for _ in 0..((env::SPEC_W >> l) * (env::SPEC_H >> l)) {
            for v in [0.5f32, 0.5, 0.5, 1.0] {
                spec.extend_from_slice(&half::f16::from_f32(v).to_le_bytes());
            }
        }
    }
    let mut sh = [[0.0f32; 4]; 9];
    sh[0] = [1.7, 1.7, 1.7, 0.0];
    env::Env { sh, key_dir: Vec3::new(-0.5, 0.8, 0.4).normalize(), spec, lut: vec![128; (env::LUT_SIZE * env::LUT_SIZE * 4) as usize] }
}
