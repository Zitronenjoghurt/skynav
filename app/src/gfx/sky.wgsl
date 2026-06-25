struct Uniforms {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    gamma: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

fn encode(color: vec3<f32>) -> vec3<f32> {
    if (u.gamma.x > 0.5) {
        return pow(max(color, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    }
    return color;
}

struct VSOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs(
    @location(0) corner: vec2<f32>,
    @location(1) position: vec3<f32>,
    @location(2) size: f32,
    @location(3) color: vec3<f32>,
    @location(4) brightness: f32,
) -> VSOut {
    var out: VSOut;
    var view_pos = u.view * vec4<f32>(position, 1.0);
    view_pos.x += corner.x * size;
    view_pos.y += corner.y * size;
    out.clip = u.proj * view_pos;
    out.uv = corner;
    out.color = color * brightness;
    return out;
}

@fragment
fn fs(in: VSOut) -> @location(0) vec4<f32> {
    let d = length(in.uv);
    if (d >= 1.0) {
        discard;
    }
    // A crisp point of light: a tight Gaussian core plus a faint, quickly
    // decaying halo - reads as a real star/planet rather than a fuzzy disk, and
    // sums nicely under additive blending so bright objects saturate to white.
    let core = exp(-d * d * 8.0);
    let halo = pow(1.0 - d, 3.0) * 0.22;
    let a = core + halo;
    return vec4<f32>(encode(in.color * a), a);
}

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_line(@location(0) position: vec3<f32>, @location(1) color: vec3<f32>) -> LineOut {
    var out: LineOut;
    out.clip = u.proj * (u.view * vec4<f32>(position, 1.0));
    out.color = color;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    return vec4<f32>(encode(in.color), 1.0);
}
