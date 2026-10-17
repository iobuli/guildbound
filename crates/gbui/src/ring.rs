//! The round map: a UI material (`shaders/gbui/ring.wgsl`) that draws a data texture as a disc
//! centred on a point and turned so the camera's forward is up, with range rings, a radar sweep
//! and a rim whose cardinal ticks turn with the camera. Colours come from `Theme::ring`.
//!
//! The texture is data, not colour (linear RGBA8): R = contour line strength, G = hill shade,
//! B = water, A = land. Map uv runs along world +X (u) and +Z (v).
//!
//! Yaw convention: ground forward = (sin yaw, cos yaw) in (x, z), as `Mover.yaw` /
//! `CameraRig.yaw` in Guildbound. North (+Z) then sits `yaw` radians clockwise from the top.
use crate::RING_SHADER;
use crate::theme::RingLook;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct RingMap {
    #[uniform(0)]
    pub params: RingParams,
    #[texture(1)]
    #[sampler(2)]
    pub map: Handle<Image>,
}

#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct RingParams {
    /// Centre (map uv), camera yaw (rad), radius (map uv).
    pub view: Vec4,
    pub fill: Vec4,
    pub ring: Vec4,
    pub land: Vec4,
    pub water: Vec4,
    pub contour: Vec4,
    pub sweep: Vec4,
    /// Rim width (px), sweep turns per second, range-ring alpha, unused.
    pub look: Vec4,
}

fn linear(c: Color) -> Vec4 {
    LinearRgba::from(c).to_vec4()
}

impl RingParams {
    pub fn new(look: &RingLook, centre: Vec2, yaw: f32, radius: f32) -> Self {
        Self {
            view: Vec4::new(centre.x, centre.y, yaw, radius),
            fill: linear(look.fill),
            ring: linear(look.ring),
            land: linear(look.land),
            water: linear(look.water),
            contour: linear(look.contour),
            sweep: linear(look.sweep),
            look: Vec4::new(look.width, look.sweep_speed, look.grid, 0.0),
        }
    }
}

impl UiMaterial for RingMap {
    fn fragment_shader() -> ShaderRef {
        RING_SHADER.into()
    }
}

/// Where a world offset (dx, dz) from the centre lands on the disc, in px from its centre
/// (x right, y down), for a view of `range` metres over `radius_px`.
pub fn to_screen(offset: Vec2, yaw: f32, range: f32, radius_px: f32) -> Vec2 {
    let (s, c) = yaw.sin_cos();
    let right = -offset.x * c + offset.y * s;
    let up = offset.x * s + offset.y * c;
    Vec2::new(right, -up) * (radius_px / range.max(1e-3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn camera_forward_is_up_and_north_turns_clockwise() {
        // camera looking +Z: a point ahead is straight up, +X is on the left (right-handed, y up)
        let up = to_screen(Vec2::new(0.0, 10.0), 0.0, 10.0, 100.0);
        assert!(up.abs_diff_eq(Vec2::new(0.0, -100.0), 1e-3), "{up}");
        let left = to_screen(Vec2::new(10.0, 0.0), 0.0, 10.0, 100.0);
        assert!(left.abs_diff_eq(Vec2::new(-100.0, 0.0), 1e-3), "{left}");
        // camera looking +X (yaw 90°): north (+Z) is now on the right, a quarter turn clockwise
        let north = to_screen(Vec2::new(0.0, 10.0), FRAC_PI_2, 10.0, 100.0);
        assert!(north.abs_diff_eq(Vec2::new(100.0, 0.0), 1e-3), "{north}");
        let ahead = to_screen(Vec2::new(10.0, 0.0), FRAC_PI_2, 10.0, 100.0);
        assert!(ahead.abs_diff_eq(Vec2::new(0.0, -100.0), 1e-3), "{ahead}");
    }
}
