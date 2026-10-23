// Pixel-art flipbook billboards (play/flipbook.rs). A square sheet of 100 px frames; the current and next frame are
// cross-faded so low frame rates still read smooth. `additive` mode returns alpha 0 (premultiplied pure add, see wind_aura.wgsl),
// alpha mode returns straight alpha so smoke and scorch can darken what is behind them.
#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

struct FlipParams {
    tint: vec4<f32>,  // rgb tint, a opacity
    frame: vec4<f32>, // x grid size, y frame, z next frame, w cross-fade
    mode: vec4<f32>,  // x additive, y intensity, z recolor, w flat flag + fractional contour-rim strength
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> f: FlipParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var sheet: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sheet_sampler: sampler;

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let m = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);
    let centre = m[3].xyz;
    let sx = length(m[0].xyz);
    let sy = length(m[1].xyz);
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    var wp = centre + right * vertex_no_morph.position.x * sx + up * vertex_no_morph.position.y * sy;
    if (f.mode.w >= 1.0) {
        // flat plane: keeps the entity's own rotation (spin about Y, lying down)
        wp = mesh_functions::mesh_position_local_to_world(m, vec4<f32>(vertex_no_morph.position, 1.0)).xyz;
    }
    out.world_position = vec4<f32>(wp, 1.0);
    out.position = position_world_to_clip(wp);
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
    out.uv = vertex_no_morph.uv;
    return out;
}

fn cell(uv: vec2<f32>, idx: f32) -> vec4<f32> {
    let n = f.frame.x;
    let col = idx - floor(idx / n) * n;
    let row = floor(idx / n);
    // Half-texel inset prevents adjacent atlas frames bleeding into bright effect edges.
    let inset = vec2<f32>(0.5) * n / vec2<f32>(textureDimensions(sheet));
    let p = (vec2<f32>(col, row) + clamp(uv, inset, vec2<f32>(1.0) - inset)) / n;
    return textureSample(sheet, sheet_sampler, p);
}

fn blended_cell(uv: vec2<f32>) -> vec4<f32> {
    return mix(cell(uv, f.frame.y), cell(uv, f.frame.z), f.frame.w);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var c = blended_cell(in.uv);
    let lum = dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let tinted = mix(c.rgb, f.tint.rgb * (0.35 + 1.3 * lum), f.mode.z);
    // soft edge so the square frame never shows
    let e = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
    let edge = smoothstep(0.0, 0.06, e);
    let alpha = clamp(c.a * f.tint.a * edge, 0.0, 1.0);
    // Alpha dilation makes a narrow light just outside the animated flame silhouette. Keeping
    // it separate from body alpha preserves shape even when stacked aura planes are translucent.
    let d = vec2<f32>(0.018, 0.0);
    let near_alpha = max(max(blended_cell(in.uv + d).a, blended_cell(in.uv - d).a),
                         max(blended_cell(in.uv + d.yx).a, blended_cell(in.uv - d.yx).a));
    let rim_strength = fract(f.mode.w);
    let rim = max(near_alpha - c.a, 0.0) * rim_strength * edge * f.tint.a;
    let rim_rgb = mix(f.tint.rgb, vec3<f32>(1.0, 0.82, 0.5), 0.7) * rim * 2.2;
    if (f.mode.x > 0.5) {
        return vec4<f32>(tinted * alpha * f.mode.y + rim_rgb, 0.0);
    }
    return vec4<f32>(tinted * f.mode.y + rim_rgb, alpha);
}
