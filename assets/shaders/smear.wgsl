// Smear streaks (play/smear.rs): thin camera-facing ribbons trailing a fast limb, drawn like a dry brush.
// uv.x runs from the limb (0) to the streak's tail (1); uv.y across the ribbon (0..1); vertex colour = tint, with
// alpha = the streak's strength. The brush: ink-dark at the limb fading to the tint toward the tail, broken up by
// noise along the stroke (dry-brush gaps) and with ragged, tapering tips, so no two streaks end alike.
#import bevy_pbr::forward_io::VertexOutput

fn hash11(x: f32) -> f32 {
    return fract(sin(x * 127.1) * 43758.5453);
}

fn noise1(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(hash11(i), hash11(i + 1.0), u);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_UVS_A
    let uv = in.uv;
#else
    let uv = vec2<f32>(0.0, 0.5);
#endif
#ifdef VERTEX_COLORS
    let tint = in.color;
#else
    let tint = vec4<f32>(0.1, 0.08, 0.08, 1.0);
#endif
    // Seed per streak rides in the tint's fractional green (set on the CPU), so neighbouring streaks differ.
    let seed = fract(tint.g * 97.0) * 50.0;
    let along = uv.x;
    let across = abs(uv.y * 2.0 - 1.0);
    // Taper: full at the limb, thinning to a point at the tail; the edge across the ribbon is soft.
    let width = 1.0 - pow(along, 1.6);
    let body = smoothstep(width, width * 0.55, across);
    // Dry-brush breakup along the stroke, stronger toward the tail.
    let gaps = smoothstep(0.25 + 0.55 * along, 0.9, noise1(along * 14.0 + seed) * 0.6 + noise1(along * 37.0 + seed * 1.7) * 0.4 + 0.25);
    let a = body * gaps * (1.0 - along * 0.85) * tint.a;
    let ink = vec3<f32>(0.06, 0.05, 0.05);
    let rgb = mix(ink, tint.rgb, smoothstep(0.1, 0.9, along));
    return vec4<f32>(rgb, a);
}
