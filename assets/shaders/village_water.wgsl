// Brookmere water: analytical ripple normals, shoreline foam and a restrained sky reflection.
// No screen-space/refraction texture dependency; works with the ordinary forward camera.
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

struct WaterParams { color: vec4<f32>, lake: vec4<f32>, clock: vec4<f32> }
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> water: WaterParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xz;
    let t = water.clock.x;
    let a = dot(p, vec2<f32>(0.8, 0.6)) * 2.1 + t * 1.3;
    let b = dot(p, vec2<f32>(-0.4, 0.9)) * 3.4 - t * 1.8;
    let n = normalize(vec3<f32>(-0.065 * cos(a) + 0.04 * cos(b), 1.0, -0.05 * cos(a) - 0.07 * cos(b)));
    let v = normalize(view.world_position - in.world_position.xyz);
    let fresnel = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);
    let shore = smoothstep(0.68, 0.99, distance(p, water.lake.xy) / water.lake.z);
    let ripples = sin(a) * sin(b);
    let foam = shore * smoothstep(0.15, 0.65, ripples + shore * 0.6) * 0.45;
    let half_light = normalize(v + normalize(vec3<f32>(0.4, 0.9, 0.25)));
    let sparkle = pow(max(dot(n, half_light), 0.0), 120.0) * 0.35;
    let deep = water.color.rgb * (0.8 + 0.12 * ripples);
    let shallow = vec3<f32>(0.13, 0.39, 0.35);
    var color = mix(deep, shallow, shore * 0.55);
    color = mix(color, vec3<f32>(0.28, 0.46, 0.58), fresnel * 0.6);
    color += vec3<f32>(foam + sparkle);
    return vec4<f32>(color, clamp(water.color.a + fresnel * 0.12 + foam * 0.1, 0.0, 0.96));
}
