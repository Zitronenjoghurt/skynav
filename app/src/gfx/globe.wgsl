struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    sun_dir: vec4<f32>,
    base_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var earth_tex: texture_2d<f32>;
@group(0) @binding(2) var earth_samp: sampler;

struct VSOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VSOut {
    var out: VSOut;
    out.clip = u.view_proj * u.model * vec4<f32>(position, 1.0);
    out.world_normal = (u.model * vec4<f32>(normal, 0.0)).xyz;
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: VSOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let l = normalize(u.sun_dir.xyz);
    let ndl = max(dot(n, l), 0.0);

    let albedo = textureSample(earth_tex, earth_samp, in.uv).rgb;
    // Real day/night: lit hemisphere sees the Sun, dark side stays near-black
    // with a faint ambient. A soft band across the terminator avoids a hard edge.
    let day = smoothstep(-0.08, 0.12, dot(n, l));
    let shade = 0.03 + 1.05 * ndl * day;
    var color = albedo * shade;
    // Encode to sRGB ourselves when the egui target is not an sRGB format.
    if (u.base_color.a > 0.5) {
        color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    }
    return vec4<f32>(color, 1.0);
}
