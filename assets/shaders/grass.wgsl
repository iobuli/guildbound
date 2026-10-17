// Grass blades (`play::grass`): Bevy 0.19's mesh vertex shader (bevy_pbr `mesh.wgsl`, same outputs and
// #ifdef guards; grass meshes are never skinned or morphed) plus wind sway, rolling gust fronts, a
// magic gust slot and characters trampling the blades. Lighting, shadow receiving and fog come from
// StandardMaterial's own fragment shader (vertex colours are the base colour).
//
// Vertex layout written by `play::grass` (chunk-local space):
//   POSITION  rest position          NORMAL  rest normal of the blade face
//   UV_0      x = t along the blade (0 root .. 1 tip), y = root height
//   UV_1      root x, z
//   COLOR     rgb = linear colour, a = per-blade random 0..1 (the output alpha is forced to 1)
//
// Bending: every influence (wind, gust, characters) adds a horizontal vector whose length is a bend
// angle in radians. A vertex at fraction t of the blade is bent by angle * t along a circular arc,
// which keeps the blade's length (the tip moves ~ t^2, like a cantilever) and tilts the normal by the
// same angle.
#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}
#import bevy_pbr::mesh_view_bindings::view

// Must match `GrassWindData` in play/grass.rs (storage layout: 6 + MAX_PUSHERS vec4s).
struct GrassWind {
    // xy: unit wind direction (world x, z), z: strength, w: gust-front phase (rad)
    wind: vec4<f32>,
    // xy: noise scroll (m), z: flutter clock (s), w: pushers in use
    motion: vec4<f32>,
    // xyz: magic gust centre, w: radius (m)
    gust: vec4<f32>,
    // x: magic gust bend (rad), y: distance where blades start to shrink, z: where they are gone,
    // w: trample bend (rad)
    params: vec4<f32>,
    // x: gust-front wave number (rad/m), y: max bend (rad), z: normal up-bias 0..1, w: flutter amount
    shape: vec4<f32>,
    // xyz: character feet, w: push radius (m)
    pushers: array<vec4<f32>, 8>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<storage, read> grass: GrassWind;

const GRASS_TAU: f32 = 6.2831853;
// Must match grass.rs: the flutter clock wraps after FLUTTER_WRAP seconds and the gust-noise
// scroll after SCROLL_WRAP metres, so flutter frequencies are whole cycles per wrap and the
// noise lattices repeat over the scroll period (2000 m x 0.035 = 70 cells, x 0.09 = 180 cells).
const FLUTTER_WRAP: f32 = 3600.0;
const SCROLL_WRAP: f32 = 2000.0;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + vec3<f32>(33.33));
    return fract((p3.x + p3.y) * p3.z);
}

// Hash of a lattice cell wrapped to `period` cells, so the noise repeats seamlessly.
fn hash21_periodic(i: vec2<f32>, period: f32) -> f32 {
    return hash21(i - period * floor(i / period));
}

// Smooth value noise 0..1, repeating every `period` lattice cells.
fn value_noise(p: vec2<f32>, period: f32) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let a = hash21_periodic(i, period);
    let b = hash21_periodic(i + vec2<f32>(1.0, 0.0), period);
    let c = hash21_periodic(i + vec2<f32>(0.0, 1.0), period);
    let d = hash21_periodic(i + vec2<f32>(1.0, 1.0), period);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// A flutter frequency (rad/s) rounded to a whole number of cycles per FLUTTER_WRAP.
fn whole_cycles(w: f32) -> f32 {
    return round(w * FLUTTER_WRAP / GRASS_TAU) * GRASS_TAU / FLUTTER_WRAP;
}

// Ambient wind: a steady lean, gust fronts rolling downwind (sine waves warped by drifting noise,
// sharpened so calm troughs separate them), patchy gust intensity, and per-blade flutter.
fn wind_bend(root: vec3<f32>, rand: f32) -> vec2<f32> {
    let dir = grass.wind.xy;
    let strength = grass.wind.z;
    let p = root.xz;
    let drift = p - grass.motion.xy;
    let warp = value_noise(drift * 0.035, SCROLL_WRAP * 0.035);
    let wave = 0.5 + 0.5 * sin(dot(p, dir) * grass.shape.x - grass.wind.w + warp * 5.0);
    let patchy = value_noise(drift * 0.09 + vec2<f32>(17.0, -9.0), SCROLL_WRAP * 0.09);
    let gust = wave * wave * wave * (0.35 + 0.9 * patchy);

    let clock = grass.motion.z;
    let phase = rand * GRASS_TAU;
    let side = vec2<f32>(-dir.y, dir.x);
    let flutter = grass.shape.w * (0.6 + 1.4 * gust);
    let along = sin(clock * whole_cycles(1.9 + 1.4 * rand) + phase + dot(p, dir) * 0.8);
    let across = sin(clock * whole_cycles(3.1 + 2.3 * rand) + phase * 1.7);
    let lean = 0.16 + 0.8 * gust + flutter * along;
    return strength * (dir * lean + side * (0.7 * flutter * across));
}

// Magic gust slot: blades within `radius` of the centre are pushed away from it (negative bend pulls).
fn magic_bend(root: vec3<f32>, fallback: vec2<f32>) -> vec2<f32> {
    let radius = grass.gust.w;
    let strength = grass.params.x;
    if (radius <= 0.0 || strength == 0.0) {
        return vec2<f32>(0.0);
    }
    let away = root.xz - grass.gust.xz;
    let dist = length(away);
    let reach = 1.0 - smoothstep(0.0, radius, distance(root, grass.gust.xyz));
    let dir = select(fallback, away / max(dist, 1e-4), dist > 1e-3);
    return dir * (reach * strength);
}

// Characters: blades within the push radius of a character's feet bend down and away from them.
fn trample_bend(root: vec3<f32>, fallback: vec2<f32>) -> vec2<f32> {
    var bend = vec2<f32>(0.0);
    let count = min(u32(max(grass.motion.w, 0.0)), 8u);
    for (var i = 0u; i < count; i = i + 1u) {
        let pusher = grass.pushers[i];
        let away = root.xz - pusher.xz;
        let dist = length(away);
        let radius = max(pusher.w, 0.01);
        let reach = 1.0 - smoothstep(0.2 * radius, radius, dist);
        // feet well above (jumping) or below (other ground) the root do not touch the blade
        let touch = 1.0 - smoothstep(0.5, 1.2, abs(pusher.y - root.y));
        let dir = select(fallback, away / max(dist, 1e-4), dist > 1e-3);
        bend = bend + dir * (reach * touch * grass.params.w);
    }
    return bend;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);

    var world_normal = vec3<f32>(0.0, 1.0, 0.0);
#ifdef VERTEX_NORMALS
    world_normal = mesh_functions::mesh_normal_local_to_world(vertex_in.normal, vertex_in.instance_index);
#endif

#ifdef VERTEX_POSITIONS
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex_in.position, 1.0)
    );

    // Blade data (missing attributes leave the blade rigid).
    var t = 0.0;
    var root_local = vertex_in.position;
#ifdef VERTEX_UVS_A
    t = clamp(vertex_in.uv.x, 0.0, 1.0);
    root_local.y = vertex_in.uv.y;
#endif
#ifdef VERTEX_UVS_B
    root_local.x = vertex_in.uv_b.x;
    root_local.z = vertex_in.uv_b.y;
#endif
    var rand = 0.5;
#ifdef VERTEX_COLORS
    rand = vertex_in.color.a;
#endif
    let root = (world_from_local * vec4<f32>(root_local, 1.0)).xyz;
    var rel = world_position.xyz - root;

    // Far blades shrink into the ground instead of popping out.
    let camera_distance = distance(view.world_position, root);
    rel = rel * (1.0 - smoothstep(grass.params.y, grass.params.z, camera_distance));

    let fallback = grass.wind.xy;
    let bend = wind_bend(root, rand) + magic_bend(root, fallback) + trample_bend(root, fallback);
    let amount = length(bend);
    let angle = min(amount, grass.shape.y) * t;
    if (amount > 1e-4 && angle > 1e-4) {
        let d = bend / amount;
        let toward = vec3<f32>(d.x, 0.0, d.y);
        // arc of a vertically measured blade: height v bends to (v (1 - cos a) / a, v sin a / a)
        let v = max(rel.y, 0.0);
        let c = cos(angle);
        let s = sin(angle);
        rel = rel + toward * (v * (1.0 - c) / angle) + vec3<f32>(0.0, v * s / angle - v, 0.0);
        // rotate the normal about the horizontal axis that turns +Y toward `toward`
        let k = vec3<f32>(d.y, 0.0, -d.x);
        world_normal = world_normal * c + cross(k, world_normal) * s + k * (dot(k, world_normal) * (1.0 - c));
    }

    world_position = vec4<f32>(root + rel, 1.0);
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);
#endif

    // Soft meadow shading: lean every normal toward world up (both faces share it; no flip).
    let biased = mix(world_normal, vec3<f32>(0.0, 1.0, 0.0), grass.shape.z) + vec3<f32>(0.0, 1e-3, 0.0);
    out.world_normal = normalize(biased);

#ifdef VERTEX_UVS_A
    out.uv = vertex_in.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex_in.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex_in.tangent,
        vertex_in.instance_index
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vec4<f32>(vertex_in.color.rgb, 1.0);
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex_in.instance_index;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);
#endif

    return out;
}
