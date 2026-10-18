// Weathered stone and wood: Bevy's standard material with grime and growth added to the colour before lighting.
// - moss on surfaces facing up, in noisy patches;
// - vertical grime streaks on the sides (rain runs down walls and posts);
// - pale wear specks (chipped edges and scuffs read as lighter flecks).
// All in world space, so pieces of one wall share one pattern.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct Weather {
    amount: vec4<f32>, // x moss, y grime, z wear, w off (1 = plain)
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> weather: Weather;

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3(1.0, 0.0, 0.0)), u.x), mix(hash3(i + vec3(0.0, 1.0, 0.0)), hash3(i + vec3(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3(0.0, 0.0, 1.0)), hash3(i + vec3(1.0, 0.0, 1.0)), u.x), mix(hash3(i + vec3(0.0, 1.0, 1.0)), hash3(i + vec3(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    return 0.55 * noise3(p) + 0.3 * noise3(p * 2.1 + 11.0) + 0.15 * noise3(p * 4.3 - 7.0);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.world_position.xyz;
    let n = normalize(in.world_normal);
    var base = pbr_input.material.base_color.rgb;
    if weather.amount.w < 0.5 {
        // moss on top faces, in patches
        let up = smoothstep(0.5, 0.9, n.y);
        let patchy = smoothstep(0.42, 0.68, fbm3(p * 1.6));
        let moss = up * patchy * weather.amount.x;
        base = mix(base, vec3(0.16, 0.24, 0.09) * (0.8 + 0.4 * noise3(p * 7.0)), moss);
        // grime streaks down the sides
        let side = 1.0 - smoothstep(0.3, 0.7, abs(n.y));
        let streak = smoothstep(0.45, 0.8, fbm3(vec3(p.x * 3.0 + p.z * 3.0, p.y * 0.35, p.z * 0.5)));
        base *= 1.0 - side * streak * 0.3 * weather.amount.y;
        // pale wear specks
        let speck = step(0.86, noise3(p * 11.0));
        base = mix(base, base * 1.35 + vec3(0.02), speck * 0.5 * weather.amount.z);
    }
    pbr_input.material.base_color = vec4(base, pbr_input.material.base_color.a);
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
