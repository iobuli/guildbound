// gbui glass: the terminal panel surface drawn over a panel's background colour. Scanlines, a slow
// bright band sweeping down, per-pixel grain and an inner glow along the edges, all in the tint
// colour, clipped to the node's rounded corners. Parameters: `gbui::glass::GlassParams`.
#import bevy_render::globals::Globals
#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(0) @binding(1) var<uniform> globals: Globals;

struct Glass {
    tint: vec4<f32>,
    // scanline alpha, scanline spacing (px), grain alpha, edge glow alpha
    look: vec4<f32>,
    // sweep alpha, sweep period (s), unused, unused
    sweep: vec4<f32>,
}

@group(1) @binding(0) var<uniform> glass: Glass;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

// Signed distance (px) to a box of half size `half` centred on the origin, corner radii in the
// order top left, top right, bottom right, bottom left (y down), as Bevy's UI shader.
fn sd_rounded_box(p: vec2<f32>, half: vec2<f32>, r: vec4<f32>) -> f32 {
    let top = select(r.x, r.y, p.x > 0.0);
    let bottom = select(r.w, r.z, p.x > 0.0);
    let rad = select(bottom, top, p.y < 0.0);
    let q = abs(p) - half + rad;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rad;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let px = in.uv * in.size;
    let half = in.size * 0.5;
    let d = sd_rounded_box(px - half, half, in.border_radius);
    let inside = 1.0 - smoothstep(-0.5, 0.5, d);
    let t = globals.time;

    let scan = glass.look.x * step(0.5, fract(px.y / glass.look.y));
    let band_y = fract(t / glass.sweep.y) * (in.size.y + 160.0) - 80.0;
    let band = (px.y - band_y) / 26.0;
    let sweep = glass.sweep.x * exp(-band * band);
    let grain = glass.look.z * hash(floor(px) + vec2<f32>(floor(t * 30.0), 0.0));
    let edge = glass.look.w * exp(-max(-d, 0.0) / 7.0);

    let a = clamp(scan + sweep + grain + edge, 0.0, 1.0) * inside * glass.tint.a;
    return vec4<f32>(glass.tint.rgb, a);
}
