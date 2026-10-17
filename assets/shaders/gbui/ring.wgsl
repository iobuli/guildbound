// gbui ring: the round map. Samples a data texture (R contour, G hill shade, B water, A land)
// around `view.xy`, turned so the camera's forward (yaw `view.z`) points up, `view.w` map-uv to
// the rim; adds range rings, a radar sweep and a rim with cardinal ticks that turn with the
// camera. Parameters: `gbui::ring::RingParams`; the maths matches `gbui::ring::to_screen`.
#import bevy_render::globals::Globals
#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(0) @binding(1) var<uniform> globals: Globals;

struct Ring {
    // centre (map uv), camera yaw (rad), radius (map uv)
    view: vec4<f32>,
    fill: vec4<f32>,
    ring: vec4<f32>,
    land: vec4<f32>,
    water: vec4<f32>,
    contour: vec4<f32>,
    sweep: vec4<f32>,
    // rim width (px), sweep turns per second, range-ring alpha, unused
    look: vec4<f32>,
}

@group(1) @binding(0) var<uniform> ring: Ring;
@group(1) @binding(1) var map_texture: texture_2d<f32>;
@group(1) @binding(2) var map_sampler: sampler;

const TAU: f32 = 6.28318530718;

// Angular distance between two angles (rad), 0..PI.
fn angle_between(a: f32, b: f32) -> f32 {
    return abs(fract((a - b) / TAU + 0.5) - 0.5) * TAU;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // p: x right, y down, the unit circle is the rim
    let p = in.uv * 2.0 - 1.0;
    let r = length(p);
    let pixel = 2.0 / max(in.size.x, 1.0);

    // Screen up is the camera's ground forward (sin yaw, cos yaw); screen right is
    // (-cos yaw, sin yaw); map uv runs along world x and z.
    let c = cos(ring.view.z);
    let s = sin(ring.view.z);
    let a = p.x * ring.view.w;
    let b = -p.y * ring.view.w;
    let uv = ring.view.xy + vec2<f32>(-c * a + s * b, s * a + c * b);
    let m = textureSampleLevel(map_texture, map_sampler, uv, 0.0);
    let on_map = select(0.0, 1.0, all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)));

    var col = ring.fill;
    let land = vec4<f32>(ring.land.rgb * (0.45 + 1.1 * m.g), ring.land.a);
    col = mix(col, land, m.a * on_map);
    col = mix(col, ring.water, m.b * on_map);
    col = mix(col, ring.contour, m.r * on_map);

    // range rings at a third and two thirds of the radius
    let grid = 1.0 - smoothstep(0.0, pixel * 1.5, min(abs(r - 1.0 / 3.0), abs(r - 2.0 / 3.0)));
    col = mix(col, ring.ring, grid * ring.look.z);

    // radar sweep: a line turning clockwise from the top with a fading tail behind it
    let theta = atan2(p.x, -p.y);
    let line = fract(globals.time * ring.look.y) * TAU;
    let behind = fract((line - theta) / TAU);
    let sweep = ring.sweep.a * exp(-behind * 10.0) * smoothstep(0.0, 0.2, r);
    col = vec4<f32>(mix(col.rgb, ring.sweep.rgb, sweep), max(col.a, sweep));

    // a soft shade toward the rim, then the rim band with cardinal ticks (north is yaw clockwise
    // from the top)
    col = vec4<f32>(col.rgb * (1.0 - 0.35 * smoothstep(0.7, 1.0, r)), col.a);
    let width = ring.look.x * pixel;
    let band = smoothstep(1.0 - width - pixel, 1.0 - width, r);
    var tick = 0.0;
    for (var k = 0; k < 4; k = k + 1) {
        let d = angle_between(theta, ring.view.z + f32(k) * TAU / 4.0);
        let size = select(0.03, 0.06, k == 0);
        tick = max(tick, 1.0 - smoothstep(size * 0.5, size, d));
    }
    let tick_band = smoothstep(1.0 - width * 2.6 - pixel, 1.0 - width * 2.6, r);
    col = mix(col, ring.ring, band);
    col = mix(col, vec4<f32>(mix(ring.ring.rgb, vec3<f32>(1.0), 0.5), 1.0), tick * tick_band);

    let edge = 1.0 - smoothstep(1.0 - pixel, 1.0, r);
    return vec4<f32>(col.rgb, col.a * edge);
}
