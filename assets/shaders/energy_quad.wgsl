// Camera-facing quads of an energy beam (play/energy.rs): the star / ring / vortex flare at the muzzle and tip, and the
// expanding impact shockwave. Ported from the MIT "Energy Beams FX" beam_flare and impact_shock shaders. Additive.
#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

struct QuadParams {
    color: vec4<f32>, // rgb flare / ring tint
    core: vec4<f32>,  // rgb core tint
    a: vec4<f32>,     // x mode (0 flare, 1 shock), y style (flare silhouette 0..6), z intensity, w core energy
    b: vec4<f32>,     // x spikes, y sharpness, z roll, w progress (shock)
    c: vec4<f32>,     // x thickness (shock), y seed
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> q: QuadParams;

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let m = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);
    let centre = m[3].xyz;
    let sx = length(m[0].xyz);
    let sy = length(m[1].xyz);
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let wp = centre + right * vertex_no_morph.position.x * sx + up * vertex_no_morph.position.y * sy;
    out.world_position = vec4<f32>(wp, 1.0);
    out.position = position_world_to_clip(wp);
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
    out.uv = vertex_no_morph.uv * 2.0 - 1.0;
    return out;
}

fn flare(uv: vec2<f32>) -> vec3<f32> {
    let spikes = q.b.x;
    let sharp = q.b.y;
    let roll = q.b.z;
    let c = cos(roll);
    let s = sin(roll);
    let v = vec2<f32>(c * uv.x - s * uv.y, s * uv.x + c * uv.y);
    let r = length(v);
    let ang = atan2(v.y, v.x);
    let intensity = q.a.z;
    let core_energy = q.a.w;
    let style = i32(q.a.y + 0.5);

    let core = exp(-r * r * 26.0);
    let halo = exp(-r * 4.2) * 0.32;
    let ray = pow(abs(cos(ang * spikes * 0.5)), sharp) * exp(-r * 2.1);
    let streak = exp(-abs(v.y) * 16.0) * exp(-abs(v.x) * 1.5) * 0.55;
    let ring = exp(-abs(r - 0.46) * 26.0) * 0.85;
    let halo_ring = exp(-abs(r - 0.46) * 6.0) * 0.22;
    var arms = pow(0.5 + 0.5 * sin(ang * 3.0 + log(max(r, 0.03)) * 7.0 + roll * 2.0), 8.0);
    arms *= exp(-r * 2.4) * smoothstep(0.02, 0.18, r);
    let cross = pow(abs(cos(ang * 2.0)), 90.0) * exp(-r * 2.6);
    var burst = pow(0.5 + 0.5 * sin(ang * 9.0 + r * 13.0 - roll * 3.0), 14.0);
    burst *= exp(-r * 2.8) * smoothstep(0.02, 0.14, r);

    var a = core * core_energy + halo * intensity;
    if (style == 0) {
        a += ray * intensity * 1.1 + streak * intensity * 0.5;
    } else if (style == 1) {
        a += ring * intensity * 1.5 + halo_ring * intensity + core * 1.4;
    } else if (style == 2) {
        a += arms * intensity * 1.5 + ray * intensity * 0.35;
    } else if (style == 3) {
        a += cross * intensity * 1.8 + core * 2.2;
    } else if (style == 4) {
        a += burst * intensity * 1.2 + ring * intensity * 0.5;
    } else if (style == 5) {
        a += halo * intensity * 0.9;
    } else {
        a += exp(-r * 3.0) * intensity * 0.5;
    }
    a *= smoothstep(1.0, 0.55, r);
    let col = mix(q.color.rgb, q.core.rgb, clamp(core * 1.4 + ring * 0.35, 0.0, 1.0));
    return col * a;
}

fn shock(uv: vec2<f32>) -> vec3<f32> {
    let progress = q.b.w;
    let thickness = q.c.x;
    let seed = q.c.y;
    let r = length(uv);
    let ang = atan2(uv.y, uv.x);
    let radius = mix(0.06, 0.94, progress);
    let wobble = 0.012 * sin(ang * 7.0 + seed * 12.0) + 0.006 * sin(ang * 13.0 - seed * 5.0);
    let d = abs(r - radius - wobble);
    let rim = smoothstep(thickness, 0.0, d);
    let halo = exp(-d * 7.0) * 0.16;
    let inner = exp(-abs(r - radius * 0.55) * 9.0) * 0.10;
    let edge = 1.0 - smoothstep(0.75, 1.0, progress);
    let a = (rim * 1.3 + halo + inner) * q.a.z * edge;
    let col = mix(q.color.rgb, q.core.rgb, rim * 0.8);
    return col * a;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var col = flare(in.uv);
    if (q.a.x > 0.5) {
        col = shock(in.uv);
    }
    return vec4<f32>(col, 0.0);
}
