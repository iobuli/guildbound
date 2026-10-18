// Terrain: Bevy's standard material over the painted vertex colours (`world::ground_color`), with procedural detail
// added to the colour before lighting:
// - macro variation (broad light and dark drifts so the meadow never reads as one flat tone);
// - triplanar rock on steep faces: noise sampled on the three planes the normal leans toward, banded into strata;
// - fine grass tone breakup on the level ground;
// - worn ground (vertex alpha: footpaths, the ruins) gets packed-earth mottling and pale pebbles.
// `look.x` > 0.5 shows the masks instead: red = steepness, green = worn, blue = height.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
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

struct Ground {
    look: vec4<f32>, // x: show masks, y: detail strength
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> ground: Ground;

fn hash2(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2(1.0, 0.0)), u.x), mix(hash2(i + vec2(0.0, 1.0)), hash2(i + vec2(1.0, 1.0)), u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    return 0.5 * noise2(p) + 0.3 * noise2(p * 2.07 + 17.0) + 0.2 * noise2(p * 4.31 - 9.0);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
#ifdef VERTEX_COLORS
    let worn = in.color.a;
#else
    let worn = 0.0;
#endif
    let p = in.world_position.xyz;
    let n = normalize(in.world_normal);
    let k = ground.look.y;
    var base = pbr_input.material.base_color.rgb;
    // (the vertex alpha only carries the worn mask; the material is opaque)

    // broad drifts
    base *= 1.0 + k * (0.22 * fbm2(p.xz * 0.045) - 0.11);

    // triplanar rock on steep faces, banded into strata
    let steep = 1.0 - smoothstep(0.72, 0.9, n.y);
    var w = pow(abs(n), vec3(4.0));
    w = w / max(w.x + w.y + w.z, 0.0001);
    let r = fbm2(p.zy * 1.3) * w.x + fbm2(p.xz * 1.3) * w.y + fbm2(p.xy * 1.3) * w.z;
    let strata = 0.5 + 0.5 * sin(p.y * 7.0 + r * 6.0);
    base = mix(base, base * (0.72 + 0.5 * r) * (0.86 + 0.14 * strata), steep * k);

    // grass tone breakup on level ground
    let level = (1.0 - steep) * (1.0 - worn);
    base *= 1.0 + k * level * (0.24 * fbm2(p.xz * 2.2) - 0.12);

    // worn ground: mottled packed earth and pale pebbles
    let pebble = step(0.84, noise2(p.xz * 9.0));
    base *= 1.0 - k * worn * 0.14 * fbm2(p.xz * 4.0);
    base = mix(base, base * 1.3 + vec3(0.03), k * worn * pebble * 0.6);

    if ground.look.x > 0.5 {
        base = vec3(steep, worn, clamp(p.y / 12.0, 0.0, 1.0));
    }
    pbr_input.material.base_color = vec4(base, 1.0);
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
