struct Globals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    accent: vec4<f32>,
};
struct Object {
    model: mat4x4<f32>,
    color: vec4<f32>,
    // x = emissive, y = metallic, z = roughness, w = press glow
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var<uniform> o: Object;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) wpos: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>) -> VsOut {
    var out: VsOut;
    let w = o.model * vec4<f32>(pos, 1.0);
    out.wpos = w.xyz;
    out.normal = normalize((o.model * vec4<f32>(normal, 0.0)).xyz);
    out.clip = g.view_proj * w;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let v = normalize(g.cam_pos.xyz - in.wpos);
    let key = normalize(vec3<f32>(-0.45, 0.85, 0.55));
    let fill = normalize(vec3<f32>(0.7, 0.25, 0.4));
    let base = pow(o.color.rgb, vec3<f32>(2.2));
    let metallic = o.params.y;
    let rough = clamp(o.params.z, 0.05, 1.0);

    let hemi = mix(0.55, 1.0, n.y * 0.5 + 0.5);              // baked-style ambient occlusion
    let ambient = base * 0.32 * hemi;
    let diff = max(dot(n, key), 0.0) * 0.85 + max(dot(n, fill), 0.0) * 0.25;
    let h = normalize(key + v);
    let spec_pow = mix(8.0, 96.0, 1.0 - rough);
    let spec = pow(max(dot(n, h), 0.0), spec_pow) * (1.0 - rough) * mix(0.25, 1.0, metallic);
    let fres = pow(1.0 - max(dot(n, v), 0.0), 3.0);
    let accent = pow(g.accent.rgb, vec3<f32>(2.2));

    var col = ambient + base * diff * (1.0 - metallic * 0.7) + vec3<f32>(spec) + accent * fres * 0.35;
    col = col + accent * o.params.w * 1.4;                    // pressed-button glow
    col = col + o.color.rgb * o.params.x;                     // emissive (lightbar)
    col = col / (col + vec3<f32>(1.0)) * 1.25;                // soft tone map
    let outc = pow(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
    return vec4<f32>(outc, 1.0);
}
