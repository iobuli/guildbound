// Energy beam tube (play/energy.rs), ported from the MIT "Energy Beams FX" Godot shader. Additive, unlit.
// Mesh: an open tube along -Z of unit length (scaled to the beam's length on z only); uv.x around, uv.y muzzle (0) to tip (1).
// The cross-section comes from the view-facing term so the tube reads volumetric from any angle. The vertex stage bends
// the whole tube (wave, arc, helix, coil, whip) and runs radial pulses and end swells along it.
#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

struct BeamParams {
    color: vec4<f32>, // rgb beam tint
    core: vec4<f32>,  // rgb core tint, a = core energy
    shape: vec4<f32>, // x beam length (m), y pattern scale, z flow speed, w time
    bend: vec4<f32>,  // x motion style (0 straight 1 wave 2 arc 3 helix 4 coil 5 whip), y amplitude, z frequency, w speed
    pulse: vec4<f32>, // x amplitude, y frequency, z speed, w twist
    misc: vec4<f32>,  // x style (0..9), y seed, z thickness power, w motion phase
    swell: vec4<f32>, // x head swell, y tip swell, z intensity
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> p: BeamParams;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;

fn fmod(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

fn hash11(x: f32) -> f32 {
    var q = fract(x * 0.1031 + p.misc.y * 0.731);
    q *= q + 33.33;
    q *= q + q;
    return fract(q);
}

fn hash21(x: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(x.x, x.y, x.x) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, vec3<f32>(p3.y, p3.z, p3.x) + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn vnoise(x: vec2<f32>) -> f32 {
    let i = floor(x);
    var f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn fbm(x0: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var x = x0;
    for (var i = 0; i < 4; i = i + 1) {
        v += amp * vnoise(x);
        x = x * 2.07 + vec2<f32>(17.3, 9.1);
        amp *= 0.5;
    }
    return v;
}

fn hex_dist(x: vec2<f32>) -> f32 {
    let q = abs(x);
    return max(q.x * 0.866025 + q.y * 0.5, q.y);
}

fn hex_coords(uv: vec2<f32>) -> vec4<f32> {
    let r = vec2<f32>(1.0, 1.7320508);
    let h = r * 0.5;
    let a = vec2<f32>(fmod(uv.x, r.x), fmod(uv.y, r.y)) - h;
    let uh = uv - h;
    let b = vec2<f32>(fmod(uh.x, r.x), fmod(uh.y, r.y)) - h;
    var gv = b;
    if (dot(a, a) < dot(b, b)) {
        gv = a;
    }
    return vec4<f32>(gv, uv - gv);
}

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let q = abs(fract(vec3<f32>(c.x) + vec3<f32>(1.0, 0.6666666, 0.3333333)) * 6.0 - 3.0);
    return c.z * mix(vec3<f32>(1.0), clamp(q - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = vertex_no_morph;
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    let t = vertex.uv.y;
    var q = vertex.position;
    let anim = p.shape.w;
    let motion_style = i32(p.bend.x + 0.5);
    let amp = p.bend.y;
    let env = sin(t * PI);
    let wave = t * p.bend.z * TAU - anim * p.bend.w + p.misc.w;
    if (motion_style == 1) {
        q.x += sin(wave) * amp * env;
        q.y += cos(wave * 1.27 + 0.6) * amp * 0.55 * env;
    } else if (motion_style == 2) {
        q.y += t * t * amp;
        q.x += sin(t * PI) * amp * 0.18;
    } else if (motion_style == 3) {
        q.x += cos(wave) * amp * t;
        q.y += sin(wave) * amp * t;
    } else if (motion_style == 4) {
        let a = t * p.pulse.w * TAU - anim * p.bend.w * 0.5 + p.misc.w;
        let c = cos(a);
        let s = sin(a);
        let xy = vec2<f32>(c * q.x + s * q.y, -s * q.x + c * q.y);
        q.x = xy.x;
        q.y = xy.y;
    } else if (motion_style == 5) {
        q.x += sin(wave) * amp * t;
        q.y += sin(wave * 0.7 + 1.2) * amp * 0.6 * t;
    }

    // Travelling radial pulses, and both ends swelling into a hot bulb.
    let pulse = 0.5 + 0.5 * sin(t * p.pulse.y * TAU - anim * p.pulse.z + p.misc.w);
    var radial = 1.0 + p.pulse.x * 1.35 * pow(pulse, 7.0);
    let head_bell = exp(-pow(t / 0.070, 2.0));
    let tip_bell = exp(-pow((1.0 - t) / 0.075, 2.0));
    radial *= 1.0 + head_bell * (p.swell.x - 1.0);
    radial *= 1.0 + tip_bell * (p.swell.y - 1.0);
    q.x *= radial;
    q.y *= radial;

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(q, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_no_morph.instance_index);
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let u = in.uv.y;
    let v = normalize(view.world_position - in.world_position.xyz);
    let facing = clamp(dot(normalize(in.world_normal), v), 0.0, 1.0);
    let s = 1.0 - facing;

    let body = pow(facing, p.misc.z);
    let core = pow(facing, 18.0);
    let ends = smoothstep(0.0, 0.02, u) * (1.0 - smoothstep(0.978, 1.0, u));
    let head_cap = exp(-pow(u / 0.075, 2.0));
    let tip_cap = exp(-pow((1.0 - u) / 0.085, 2.0));

    let along = u * p.shape.x * p.shape.y;
    let t = p.shape.w * p.shape.z;
    let style = i32(p.misc.x + 0.5);
    let seed = p.misc.y;
    var mask = 1.0;
    var tint = p.color.rgb;
    let hot = p.core.rgb;

    if (style == 0) {
        mask = 0.88 + 0.12 * sin(along * 2.0 - t * 1.5);
    } else if (style == 1) {
        let b = 0.5 + 0.5 * sin(along * 2.2 - t * 2.0);
        mask = 0.32 + 1.25 * pow(b, 5.0);
    } else if (style == 2) {
        let n = fbm(vec2<f32>(along * 1.3 - t * 1.6, s * 3.0 + seed * 3.0));
        let arc = pow(max(n - 0.40, 0.0) * 2.6, 2.0);
        mask = 0.26 + 2.7 * arc;
        tint = mix(p.color.rgb, hot, clamp(arc * 0.55, 0.0, 0.75));
    } else if (style == 3) {
        let hc = hex_coords(vec2<f32>(along * 1.35 - t * 0.7, s * 6.0));
        let edge = smoothstep(0.27, 0.48, hex_dist(hc.xy));
        let pl = 0.55 + 0.45 * sin(t * 2.0 + hash21(hc.zw) * 6.283);
        mask = 0.22 + 1.7 * edge * pl;
    } else if (style == 4) {
        let cell = 0.5 + 0.5 * sin(floor(along * 1.8) * 2.1 + t * 3.0);
        mask = 0.24 + 1.65 * pow(cell, 4.0);
    } else if (style == 5) {
        let f = fract(along * 0.9);
        let id = floor(along * 0.9);
        let facet = pow(0.5 + 0.5 * cos(f * 6.2831 + hash11(id) * 6.2831), 3.0);
        mask = 0.30 + 1.55 * facet;
        tint = mix(p.color.rgb, hot, facet * 0.35);
    } else if (style == 6) {
        tint = hsv2rgb(vec3<f32>(fract(u * 0.95 + p.shape.w * 0.06 + seed * 0.11), 0.90, 1.0));
        mask = 0.92 + 0.08 * sin(along * 4.0 - t * 2.0);
    } else if (style == 7) {
        let n = fbm(vec2<f32>(along * 0.8 - t * 0.9, s * 2.6 + seed * 4.0));
        let n2 = fbm(vec2<f32>(along * 2.8 + t * 2.1, s * 7.5 - seed));
        let plasma = clamp(n * 0.72 + n2 * 0.5, 0.0, 1.0);
        mask = 0.20 + 2.1 * pow(plasma, 1.7);
        tint = mix(p.color.rgb, hot, clamp(n2 * 0.7, 0.0, 0.6));
    } else if (style == 8) {
        let sp = 0.5 + 0.5 * sin(along * 2.7 + s * 9.0 - t * 3.4);
        mask = 0.28 + 1.65 * pow(sp, 3.0);
    } else {
        let b = pow(0.5 + 0.5 * sin(along * 3.6 - t * 5.0), 22.0);
        mask = 0.18 + 3.0 * b;
    }

    // A bright ring travels with every radial bulge.
    let pulse_ring = pow(0.5 + 0.5 * sin(u * p.pulse.y * TAU - p.shape.w * p.pulse.z + p.misc.w), 14.0);
    let ring_gain = clamp(p.pulse.x, 0.0, 1.5) * 2.6;
    mask += pulse_ring * ring_gain;
    tint = mix(tint, hot, pulse_ring * 0.55);

    let flick = 0.94 + 0.06 * sin(p.shape.w * 21.0 + seed * 8.0);
    let lum = p.swell.z * ends * flick * (1.0 + 1.5 * head_cap + 1.9 * tip_cap);

    var col = tint * body * mask * lum;
    col += mix(hot, p.color.rgb, 0.45) * core * p.core.a * ends * (0.65 + 0.35 * mask);
    // Four additive layers overexpose to white at peak: soft-cap the brightest channel and keep the hue.
    let m = max(col.r, max(col.g, col.b));
    col = col * ((1.0 - exp(-m * 1.25)) / max(m, 0.0001));
    // `AlphaMode::Add` blends as premultiplied alpha, so alpha 0 is a pure add (see wind_aura.wgsl).
    return vec4<f32>(col, 0.0);
}
