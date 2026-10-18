// Character skin: Bevy's standard material (lighting, shadows, fog), then one of three looks, plus a presence rim.
// - mode 0, plain: the lit colour as is.
// - mode 1, toon: brightness banded into flat steps (hue kept), lifted half-Lambert style (as toon.wgsl).
// - mode 2, ink: toon bands, an ink line darkening the silhouette, and brush hatching that only lives in the shadow
//   bands (strokes run diagonally in world space so they stay put on the body as it moves).
// Presence (enemies): a rim glow in `rim.rgb` at strength `rim.a` (their awareness of you), and `look.x` flashes the
// whole silhouette white for the alert "blink".
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

struct Skin {
    rim: vec4<f32>,  // presence rim: rgb colour, a strength
    look: vec4<f32>, // x alert flash 0..1, y mode (0 plain, 1 toon, 2 ink), z ink weight, w unused
    tint: vec4<f32>, // rgb accent mixed into the shadow bands (per-character palette), a amount
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> skin: Skin;

const BANDS: f32 = 4.0;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    let lit = apply_pbr_lighting(pbr_input);
    let base = max(pbr_input.material.base_color.rgb, vec3(0.001));
    let n_dot_v = max(dot(pbr_input.N, pbr_input.V), 0.0);
    let edge = 1.0 - n_dot_v;
    var colour = lit.rgb;
    let mode = skin.look.y;
    if mode > 0.5 {
        let luma = vec3(0.2126, 0.7152, 0.0722);
        let light = dot(lit.rgb, luma) / max(dot(base, luma), 0.001);
        let x = clamp(light, 0.0, 1.5) * BANDS;
        let band = (floor(x) + smoothstep(0.42, 0.58, fract(x))) / BANDS;
        let soft = 0.3 + 0.75 * band;
        // shadow bands lean toward the character's accent colour
        let shade = mix(base, base * skin.tint.rgb * 1.4, skin.tint.a * (1.0 - band));
        colour = shade * soft + pbr_input.material.emissive.rgb + base * pow(edge, 4.0) * 0.25;
        if mode > 1.5 {
            let ink = skin.look.z;
            // ink line along the silhouette
            colour *= mix(1.0, 0.25, ink * smoothstep(0.62, 0.86, edge));
            // brush hatching in the shadows: diagonal world-space strokes, broken up along their length
            let p = in.world_position.xyz;
            let stroke = fract((p.x + p.y * 1.3 - p.z * 0.7) * 22.0);
            let breakup = fract(sin(dot(floor(p * 9.0), vec3(12.9898, 78.233, 37.719))) * 43758.5453);
            let in_shadow = 1.0 - smoothstep(0.35, 0.6, band);
            let hatch = step(stroke, 0.32) * step(0.25, breakup) * in_shadow;
            colour *= 1.0 - 0.28 * ink * hatch;
        }
    }
    // presence rim and the alert blink
    colour += skin.rim.rgb * skin.rim.a * pow(edge, 3.0) * 1.6;
    colour += vec3(1.0) * skin.look.x * pow(edge, 1.5) * 2.5;
    out.color = main_pass_post_lighting_processing(pbr_input, vec4(colour, lit.a));
#endif
    return out;
}
