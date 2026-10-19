// Wind aura: a shell that shows only at its silhouette (fresnel), where thin streaks of light are combed along a
// wind direction and slide past over time. The streaks are stretched across the wind: the noise is sampled in a
// wind-aligned frame, with a long axis along the gust and a short axis across it, so each streak reads as air being
// pushed rather than as a flat texture. Two layers at different speeds give depth. Additive, unlit.
#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
}

struct WindAura {
    color: vec4<f32>, // rgb tint, a = overall intensity
    wind: vec4<f32>,  // xy = wind direction on the ground, z = gust speed, w = streak density
    clock: vec4<f32>, // x = seconds since start, y = edge sharpness
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> aura: WindAura;

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// One streak layer. `along` runs with the gust, `across` runs over it; `phase` shifts the layer in time.
fn streak_layer(along: f32, across: f32, speed: f32, phase: f32, t: f32) -> f32 {
    let moving = along - t * speed * phase;
    let stretched = value_noise(vec2<f32>(moving * 0.35, across * 3.0 + phase * 17.0));
    // sharpen the noise into thin bright lines and drop the rest
    return pow(smoothstep(0.55, 0.9, stretched), 2.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v = normalize(view.world_position - in.world_position.xyz);

    // silhouette: strong where the surface turns away from the viewer, nearly invisible face-on
    let facing = abs(dot(n, v));
    let edge = pow(1.0 - facing, aura.clock.y);

    // wind-aligned frame on the ground plane
    let dir = normalize(aura.wind.xy + vec2<f32>(1e-5, 0.0));
    let across_dir = vec2<f32>(-dir.y, dir.x);
    let p = in.world_position.xz * aura.wind.w;
    let along = dot(p, dir);
    let across = dot(p, across_dir);

    let t = aura.clock.x;
    let speed = aura.wind.z;
    // height keeps streaks from lining up in one plane: break them up vertically
    let height = in.world_position.y * 0.6;
    let streaks = streak_layer(along + height, across, speed, 1.0, t)
        + 0.6 * streak_layer(along * 1.7 + height, across * 1.3, speed, 0.55, t);

    let glow = edge * (0.25 + streaks * 1.4);
    let colour = aura.color.rgb * glow * aura.color.a;
    // `AlphaMode::Add` blends as premultiplied alpha (result = src + (1 - src.a) * dst), so the
    // alpha must be 0 for a pure add (Bevy's own `premultiply_alpha` does the same). An alpha of 1
    // replaced what was behind with this dim colour: the aura drew as a black capsule.
    return vec4<f32>(colour, 0.0);
}
