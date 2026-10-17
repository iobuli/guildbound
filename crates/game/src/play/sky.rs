//! The day: a `Clock` (game hours, a full day every `DAY_MINUTES` of game time; `GB_HOUR` sets the
//! starting hour, F9 skips an hour) drives the sun, which rises in the east at 6:00 and sets in
//! the west at 18:00, and a moon that lights the night more faintly. Light colour warms at dawn
//! and dusk; the sky (`ClearColor`), the camera's fog and the ambient light follow the same
//! curve, so torches matter at night. `occupations` switches civilians to their night routine and
//! the minimap readout shows the time. Written here only: `Clock`, the sun's light and transform,
//! `ClearColor`, `GlobalAmbientLight`, the camera's `DistanceFog` colour.
use super::camera::MainCamera;
use bevy::prelude::*;

/// Game minutes per full day.
const DAY_MINUTES: f32 = 24.0;
/// Starting hour without `GB_HOUR`.
const START_HOUR: f32 = 9.0;
/// Night is from dusk to dawn (hours).
pub const DUSK: f32 = 20.0;
pub const DAWN: f32 = 6.0;
/// Peak light (lux) for the sun and the moon, ambient brightness by day and by night.
const SUN_LUX: f32 = 8000.0;
const MOON_LUX: f32 = 700.0;
const AMBIENT_DAY: f32 = 400.0;
const AMBIENT_NIGHT: f32 = 110.0;

/// The time of day (written by `sky` only).
#[derive(Resource, Clone, Copy, Debug)]
pub struct Clock {
    /// 0..24.
    pub hour: f32,
}

impl Clock {
    pub fn is_night(&self) -> bool {
        self.hour >= DUSK || self.hour < DAWN
    }
    /// "09:30"
    pub fn label(&self) -> String {
        let h = self.hour.rem_euclid(24.0);
        format!("{:02}:{:02}", h.floor() as u32, ((h.fract() * 60.0).floor() as u32).min(59))
    }
}

/// The light of the hour: direction the light travels, illuminance, colour, sky / fog colour,
/// ambient brightness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub dir: Vec3,
    pub lux: f32,
    pub color: Color,
    pub sky: Color,
    pub ambient: f32,
}

/// Sun elevation (rad) at `hour`: 0 at 6:00 and 18:00, highest at noon, negative at night.
pub fn elevation(hour: f32) -> f32 {
    ((hour - 6.0) / 12.0 * std::f32::consts::PI).sin() * 1.1
}

pub fn light_at(hour: f32) -> Light {
    use std::f32::consts::PI;
    let h = hour.rem_euclid(24.0);
    let el = elevation(h);
    // East (+X) at sunrise, west at sunset, a little south at noon.
    let az = (h - 6.0) / 12.0 * PI;
    let sun = Vec3::new(az.cos(), el.sin().max(-1.0), 0.35 + 0.2 * az.sin()).normalize();
    let day = (el.sin() / 0.25).clamp(0.0, 1.0);
    let night = (-el.sin() / 0.2).clamp(0.0, 1.0);
    let low = 1.0 - (el.sin().abs() / 0.45).clamp(0.0, 1.0);
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    let noon_sky = [0.55, 0.68, 0.82];
    let dusk_sky = [0.86, 0.56, 0.42];
    let night_sky = [0.035, 0.05, 0.11];
    let sky = mix(mix(noon_sky, dusk_sky, low * (1.0 - night)), night_sky, night);
    if el >= 0.0 {
        let warm = mix([1.0, 0.98, 0.95], [1.0, 0.62, 0.38], low);
        Light {
            dir: -sun,
            lux: SUN_LUX * day.powf(0.6),
            color: Color::srgb(warm[0], warm[1], warm[2]),
            sky: Color::srgb(sky[0], sky[1], sky[2]),
            ambient: AMBIENT_NIGHT + (AMBIENT_DAY - AMBIENT_NIGHT) * day,
        }
    } else {
        // The moon: opposite the sun, cool and faint.
        let moon = Vec3::new(-sun.x, -sun.y, sun.z).normalize();
        Light {
            dir: -moon,
            lux: MOON_LUX * night,
            color: Color::srgb(0.62, 0.72, 1.0),
            sky: Color::srgb(sky[0], sky[1], sky[2]),
            ambient: AMBIENT_NIGHT,
        }
    }
}

/// The sun's light entity.
#[derive(Component)]
pub struct Sun;

pub fn plugin(app: &mut App) {
    let hour = std::env::var("GB_HOUR")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|h| h.is_finite())
        .unwrap_or(START_HOUR)
        .rem_euclid(24.0);
    app.insert_resource(Clock { hour })
        .add_message::<SetHour>()
        .add_systems(Update, (tick, shine).chain());
}

/// Jump the clock to an hour (sleeping until dawn).
#[derive(Message, Clone, Copy, Debug)]
pub struct SetHour(pub f32);

fn tick(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut sets: MessageReader<SetHour>, mut clock: ResMut<Clock>) {
    let mut hour = clock.hour + time.delta_secs() * 24.0 / (DAY_MINUTES * 60.0);
    if let Some(h) = sets.read().last().filter(|h| h.0.is_finite()) {
        hour = h.0;
    }
    if keys.just_pressed(KeyCode::F9) {
        hour += 1.0;
    }
    clock.hour = hour.rem_euclid(24.0);
}

fn shine(
    clock: Res<Clock>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut sky: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fog: Query<&mut DistanceFog, With<MainCamera>>,
) {
    let l = light_at(clock.hour);
    for (mut light, mut tf) in &mut sun {
        light.illuminance = l.lux;
        light.color = l.color;
        *tf = Transform::default().looking_to(l.dir, Vec3::Y);
    }
    sky.0 = l.sky;
    ambient.brightness = l.ambient;
    for mut f in &mut fog {
        f.color = l.sky;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_bright_midnight_is_moonlit() {
        let noon = light_at(12.0);
        let midnight = light_at(0.0);
        assert!(noon.lux > 7000.0 && noon.dir.y < -0.8, "{noon:?}");
        assert!(midnight.lux > 0.0 && midnight.lux < noon.lux / 5.0);
        assert!(midnight.dir.y < 0.0, "the moon lights from above");
        assert!(midnight.ambient < noon.ambient);
    }

    #[test]
    fn the_sun_rises_east_and_sets_west() {
        let morning = light_at(7.0);
        let evening = light_at(17.0);
        // Light travels away from the sun: from the east (+X) it heads −X.
        assert!(morning.dir.x < 0.0 && evening.dir.x > 0.0);
        assert!(elevation(6.0).abs() < 1e-5 && elevation(18.0).abs() < 1e-4);
    }

    #[test]
    fn night_and_labels() {
        assert!(Clock { hour: 23.0 }.is_night() && Clock { hour: 3.0 }.is_night());
        assert!(!Clock { hour: 12.0 }.is_night());
        assert_eq!(Clock { hour: 9.5 }.label(), "09:30");
    }
}
