// Toon shading on top of Bevy's standard material: the usual lighting (sun, shadows, fog), then the brightness is
// banded into flat steps while the hue is kept, plus a soft rim light along silhouettes. Lift and soft band
// edges after the half-Lambert ramp shaders (RyShade, godotshaders Genshin-style toon), fitted to Bevy's PBR.
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
    // light reaching the surface relative to its own colour, banded, then reapplied to the colour
    let luma = vec3(0.2126, 0.7152, 0.0722);
    let light = dot(lit.rgb, luma) / max(dot(base, luma), 0.001);
    // bands with narrow smooth edges (no shimmer as the camera moves), lifted half-Lambert style so the
    // shadow side keeps its form instead of crushing to black
    let x = clamp(light, 0.0, 1.5) * BANDS;
    let band = (floor(x) + smoothstep(0.42, 0.58, fract(x))) / BANDS;
    let soft = 0.3 + 0.75 * band;
    var colour = base * soft + pbr_input.material.emissive.rgb;
    let rim = pow(1.0 - max(dot(pbr_input.N, pbr_input.V), 0.0), 4.0) * 0.25;
    colour += base * rim;
    out.color = main_pass_post_lighting_processing(pbr_input, vec4(colour, lit.a));
#endif
    return out;
}
