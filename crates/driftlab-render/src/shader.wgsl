// DriftLab product-render shader.
// glTF metallic-roughness PBR (GGX), HDRI image-based lighting (SH9 irradiance + prefiltered specular + BRDF LUT),
// shadow-mapped key light, ACES tone mapping, ground shadow + reflection. All lighting is computed in linear space.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    cam_pos: vec4<f32>,
    accent: vec4<f32>,
    // x = ground y, y = reflection strength, z = shadow strength, w = exposure
    misc: vec4<f32>,
    key_dir: vec4<f32>,
    sh: array<vec4<f32>, 9>,
};
struct Object {
    model: mat4x4<f32>,
    color: vec4<f32>,
    // x = emissive strength (glow kinds), y = metallic, z = roughness, w = press glow
    params: vec4<f32>,
    // x = kind (0 normal, 1 accent colour, 2 glow, 3 grip detail, 4 guide glow), y = has normal map, z = normal scale
    extra: vec4<f32>,
    emissive: vec4<f32>,
    // KHR_texture_transform: scale.xy, offset.xy
    uvxf: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_smp: sampler_comparison;
@group(0) @binding(3) var env_spec: texture_2d<f32>;
@group(0) @binding(4) var env_smp: sampler;
@group(0) @binding(5) var brdf_lut: texture_2d<f32>;
@group(1) @binding(0) var<uniform> o: Object;
@group(2) @binding(0) var t_base: texture_2d<f32>;
@group(2) @binding(1) var t_mr: texture_2d<f32>;
@group(2) @binding(2) var t_normal: texture_2d<f32>;
@group(2) @binding(3) var t_occ: texture_2d<f32>;
@group(2) @binding(4) var t_emis: texture_2d<f32>;
@group(2) @binding(5) var mat_smp: sampler;

const PI: f32 = 3.14159265;
const SPEC_LEVELS: f32 = 7.0;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) wpos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    let w = o.model * vec4<f32>(pos, 1.0);
    out.wpos = w.xyz;
    out.normal = normalize((o.model * vec4<f32>(normal, 0.0)).xyz);
    out.uv = uv;
    out.clip = g.view_proj * w;
    return out;
}

@vertex
fn vs_shadow(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return g.light_vp * (o.model * vec4<f32>(pos, 1.0));
}

fn env_uv(d: vec3<f32>) -> vec2<f32> {
    let u = atan2(d.z, d.x) / (2.0 * PI) + 0.5;
    let v = acos(clamp(d.y, -1.0, 1.0)) / PI;
    return vec2<f32>(u, v);
}

fn sh_irradiance(n: vec3<f32>) -> vec3<f32> {
    var c = g.sh[0].rgb * 0.282095;
    c = c + g.sh[1].rgb * (0.488603 * n.y) + g.sh[2].rgb * (0.488603 * n.z) + g.sh[3].rgb * (0.488603 * n.x);
    c = c + g.sh[4].rgb * (1.092548 * n.x * n.y) + g.sh[5].rgb * (1.092548 * n.y * n.z);
    c = c + g.sh[6].rgb * (0.315392 * (3.0 * n.z * n.z - 1.0));
    c = c + g.sh[7].rgb * (1.092548 * n.x * n.z) + g.sh[8].rgb * (0.546274 * (n.x * n.x - n.y * n.y));
    return max(c, vec3<f32>(0.0));
}

fn shadow_factor(wpos: vec3<f32>, n: vec3<f32>) -> f32 {
    let p = g.light_vp * vec4<f32>(wpos + n * 0.015, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 1.0 - (p.y * 0.5 + 0.5));
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) { return 1.0; }
    let texel = 1.0 / 2048.0;
    var s = 0.0;
    for (var i = -2; i <= 2; i = i + 1) {
        for (var j = -2; j <= 2; j = j + 1) {
            s = s + textureSampleCompare(shadow_tex, shadow_smp, uv + vec2<f32>(f32(i), f32(j)) * texel * 1.6, p.z - 0.0012);
        }
    }
    return s / 25.0;
}

fn ggx(nh: f32, rough: f32) -> f32 {
    let a = rough * rough;
    let a2 = a * a;
    let d = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

fn smith_vis(nl: f32, nv: f32, rough: f32) -> f32 {
    let a = rough * rough;
    let gv = nl * sqrt(nv * nv * (1.0 - a) + a);
    let gl = nv * sqrt(nl * nl * (1.0 - a) + a);
    return 0.5 / max(gv + gl, 1e-4);
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    // Narkowicz ACES filmic fit
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn shade(wpos: vec3<f32>, n_geo: vec3<f32>, uv_in: vec2<f32>, with_shadow: bool) -> vec3<f32> {
    let uv = uv_in * o.uvxf.xy + o.uvxf.zw;
    // Derivatives first (uniform control flow); texture reads use explicit gradients afterwards.
    let dp1 = dpdx(wpos);
    let dp2 = dpdy(wpos);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);

    var n = normalize(n_geo);
    let kind = i32(o.extra.x + 0.5);
    if (o.extra.y > 0.5) {
        let nt = textureSampleGrad(t_normal, mat_smp, uv, duv1, duv2).xyz * 2.0 - 1.0;
        let dp2perp = cross(dp2, n);
        let dp1perp = cross(n, dp1);
        let t = dp2perp * duv1.x + dp1perp * duv2.x;
        let b = dp2perp * duv1.y + dp1perp * duv2.y;
        let invmax = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1e-12));
        n = normalize(mat3x3<f32>(t * invmax, b * invmax, n) * vec3<f32>(nt.xy * o.extra.z, nt.z));
    }
    if (kind == 3) {
        // fine grip texture (procedural normal map)
        let p = wpos * 420.0;
        n = normalize(n + 0.07 * vec3<f32>(cos(p.x) * sin(p.y), cos(p.y) * sin(p.z), cos(p.z) * sin(p.x)));
    }
    let base_s = textureSampleGrad(t_base, mat_smp, uv, duv1, duv2);
    let mr = textureSampleGrad(t_mr, mat_smp, uv, duv1, duv2);
    let occ = textureSampleGrad(t_occ, mat_smp, uv, duv1, duv2).r;
    let emis_t = textureSampleGrad(t_emis, mat_smp, uv, duv1, duv2).rgb;

    var base = o.color.rgb * base_s.rgb;
    if (kind == 1) { base = g.accent.rgb; }
    let metallic = clamp(o.params.y * mr.b, 0.0, 1.0);
    let rough = clamp(max(o.params.z * mr.g, o.extra.w), 0.045, 1.0);
    let v = normalize(g.cam_pos.xyz - wpos);
    let nv = max(dot(n, v), 0.001);
    let f0 = mix(vec3<f32>(0.04), base, metallic);
    let kd = (1.0 - metallic) * base;

    // --- image-based lighting
    let r = reflect(-v, n);
    let lod = rough * (SPEC_LEVELS - 1.0);
    let spec_pf = textureSampleLevel(env_spec, env_smp, env_uv(r), lod).rgb;
    let ab = textureSampleLevel(brdf_lut, env_smp, vec2<f32>(nv, rough), 0.0).rg;
    let fr = max(vec3<f32>(1.0 - rough), f0) - f0;
    let fs = f0 + fr * pow(1.0 - nv, 5.0);
    let spec_ibl = spec_pf * (fs * ab.x + vec3<f32>(ab.y));
    let diff_ibl = kd * sh_irradiance(n);
    // specular occlusion from the baked AO term plus a cheap orientation-based sky visibility
    let sky_vis = mix(0.55, 1.0, clamp(n.y * 0.5 + 0.5, 0.0, 1.0));
    let ao = mix(1.0, occ, 1.0) * sky_vis;
    var col = (diff_ibl + spec_ibl) * ao;

    // --- shadow-casting key light (brightest direction of the HDRI)
    let l = normalize(g.key_dir.xyz);
    let h = normalize(l + v);
    let nl = max(dot(n, l), 0.0);
    var sh = 1.0;
    if (with_shadow) { sh = mix(1.0, shadow_factor(wpos, n_geo), g.misc.z); }
    let fres = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);
    let spec_dir = ggx(max(dot(n, h), 0.0), rough) * smith_vis(nl, nv, rough) * fres;
    col = col + (kd / PI + spec_dir) * nl * 1.1 * sh;

    // --- emission (glTF emissive, lightbar / guide in the theme accent, pressed buttons)
    var emis = o.emissive.rgb * emis_t;
    if (kind == 2 || kind == 4) { emis = o.emissive.rgb * o.params.x; }
    col = col + emis + g.accent.rgb * o.params.w * 1.4;
    return col;
}

fn finish_colour(c: vec3<f32>) -> vec3<f32> {
    return to_srgb(aces(c * g.misc.w));
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(finish_colour(shade(in.wpos, in.normal, in.uv, true)), 1.0);
}

@fragment
fn fs_reflect(in: VsOut) -> @location(0) vec4<f32> {
    let c = finish_colour(shade(in.wpos, in.normal, in.uv, false));
    let depth = max(g.misc.x - in.wpos.y, 0.0);
    let fade = clamp(1.0 - depth / 0.9, 0.0, 1.0);
    let a = g.misc.y * fade * fade;
    return vec4<f32>(c * a, a); // premultiplied
}

struct GroundOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) wpos: vec3<f32>,
};

@vertex
fn vs_ground(@builtin(vertex_index) i: u32) -> GroundOut {
    var q = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    var out: GroundOut;
    let p = vec3<f32>(q[i].x * 9.0, g.misc.x, q[i].y * 9.0);
    out.wpos = p;
    out.clip = g.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@fragment
fn fs_ground(in: GroundOut) -> @location(0) vec4<f32> {
    let sh = shadow_factor(in.wpos, vec3<f32>(0.0, 1.0, 0.0));
    // Fade to zero well inside the viewport so the shadow never ends in a hard rectangle on light themes.
    let d = length(in.wpos.xz * vec2<f32>(1.0, 1.25)) / 2.3;
    let f0 = clamp(1.0 - d * d, 0.0, 1.0);
    let falloff = f0 * f0;
    let blob = exp(-pow(length(in.wpos.xz * vec2<f32>(0.75, 1.1)), 2.0) * 0.22) * 0.30;
    let a = ((1.0 - sh) * 0.5 + blob) * g.misc.z * falloff;
    return vec4<f32>(0.0, 0.0, 0.0, clamp(a, 0.0, 0.8));
}

@fragment
fn fs_blend(in: VsOut) -> @location(0) vec4<f32> {
    let uvt = in.uv * o.uvxf.xy + o.uvxf.zw;
    let duv1 = dpdx(uvt);
    let duv2 = dpdy(uvt);
    let a = o.color.w * textureSampleGrad(t_base, mat_smp, uvt, duv1, duv2).a;
    let c = finish_colour(shade(in.wpos, in.normal, in.uv, false));
    return vec4<f32>(c, clamp(a, 0.0, 1.0));
}

