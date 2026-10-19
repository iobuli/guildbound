//! Weather (E22): clear, overcast, rain and fog, changing on their own through the day.
//!
//! - `Weather` (written here only) holds the current sky and three smoothed amounts, `cloud`,
//!   `rain` and `mist` (0..1), easing toward the sky's targets over `EASE` s so a change rolls in
//!   rather than snapping. Each sky lasts `HOLD` game seconds (a random span), then the next is
//!   drawn from what usually follows it (`next`): clear skies cloud over, overcast turns to rain
//!   or clears, rain eases to overcast, fog burns off.
//! - `sky` reads it (`shade`): clouds dim the sun and ambient light and grey the sky; mist and rain
//!   pull the fog in. The sun, sky colour, ambient light and fog stay `sky`'s to write.
//! - Rain falls around the camera: `DROPS` thin streaks in a box that follows it, as many shown as
//!   the rain amount calls for, wrapping round when they fall out of the box.
//! - `GB_WEATHER=clear|overcast|rain|fog` holds that sky from the start at full strength (stills,
//!   recordings). `SetSky` (the god panel) switches the sky now; the amounts then ease toward it.
//!   The minimap readout names the sky after the time.
use super::camera::MainCamera;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

/// Seconds to ease most of the way to a new sky's amounts.
const EASE: f32 = 20.0;
/// How long a sky lasts (game seconds): shortest, longest.
const HOLD: (f32, f32) = (150.0, 420.0);
/// Rain streaks, the box they fall in round the camera (half extents, m), their speed (m/s) and
/// the wind's slant (m/s along x).
const DROPS: usize = 500;
const BOX: Vec3 = Vec3::new(14.0, 8.0, 14.0);
const FALL: f32 = 15.0;
const SLANT: f32 = 1.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sky {
    #[default]
    Clear,
    Overcast,
    Rain,
    Fog,
}

impl Sky {
    pub const ALL: [Sky; 4] = [Sky::Clear, Sky::Overcast, Sky::Rain, Sky::Fog];

    /// Target (cloud, rain, mist).
    pub fn targets(self) -> (f32, f32, f32) {
        match self {
            Sky::Clear => (0.0, 0.0, 0.0),
            Sky::Overcast => (0.75, 0.0, 0.15),
            Sky::Rain => (1.0, 1.0, 0.4),
            Sky::Fog => (0.45, 0.0, 1.0),
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Sky::Clear => "clear",
            Sky::Overcast => "overcast",
            Sky::Rain => "rain",
            Sky::Fog => "fog",
        }
    }

    pub fn parse(s: &str) -> Option<Sky> {
        Sky::ALL.into_iter().find(|k| k.word() == s.trim().to_ascii_lowercase())
    }

    /// What usually follows: (sky, weight).
    fn followers(self) -> &'static [(Sky, f32)] {
        match self {
            Sky::Clear => &[(Sky::Clear, 0.45), (Sky::Overcast, 0.4), (Sky::Fog, 0.15)],
            Sky::Overcast => &[(Sky::Clear, 0.35), (Sky::Overcast, 0.2), (Sky::Rain, 0.45)],
            Sky::Rain => &[(Sky::Clear, 0.15), (Sky::Overcast, 0.6), (Sky::Rain, 0.25)],
            Sky::Fog => &[(Sky::Clear, 0.6), (Sky::Overcast, 0.4)],
        }
    }
}

/// The next sky after `now`, for a roll `r` in 0..1.
pub fn next(now: Sky, r: f32) -> Sky {
    let f = now.followers();
    let total: f32 = f.iter().map(|(_, w)| w).sum();
    let mut acc = 0.0;
    for (sky, w) in f {
        acc += w / total;
        if r < acc {
            return *sky;
        }
    }
    f.last().map_or(now, |(s, _)| *s)
}

/// The weather now (written by `weather` only).
#[derive(Resource, Clone, Debug)]
pub struct Weather {
    pub sky: Sky,
    pub cloud: f32,
    pub rain: f32,
    pub mist: f32,
    /// Game time the current sky ends; `None` = held (`GB_WEATHER`).
    until: Option<f32>,
    rng: u32,
}

impl Default for Weather {
    fn default() -> Self {
        Self { sky: Sky::Clear, cloud: 0.0, rain: 0.0, mist: 0.0, until: Some(HOLD.0), rng: 0x5EED_1234 }
    }
}

impl Weather {
    /// A sky held from the start at full strength.
    pub fn held(sky: Sky) -> Self {
        let (cloud, rain, mist) = sky.targets();
        Self { sky, cloud, rain, mist, until: None, ..default() }
    }

    fn roll(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Switch to `sky` at game time `now`: a held sky stays held, otherwise it lasts `HOLD.1`.
    pub fn set(&mut self, sky: Sky, now: f32) {
        self.sky = sky;
        if self.until.is_some() {
            self.until = Some(now + HOLD.1);
        }
        info!("[weather] set to {}", sky.word());
    }

    /// The sky after this one in `Sky::ALL` (the god panel's cycle).
    pub fn cycled(sky: Sky) -> Sky {
        let i = Sky::ALL.iter().position(|s| *s == sky).unwrap_or(0);
        Sky::ALL[(i + 1) % Sky::ALL.len()]
    }

    /// Advance by `dt` at game time `now`: maybe change sky, ease the amounts.
    pub fn step(&mut self, now: f32, dt: f32) {
        if let Some(until) = self.until
            && now >= until
        {
            let r = self.roll();
            self.sky = next(self.sky, r);
            let span = HOLD.0 + (HOLD.1 - HOLD.0) * self.roll();
            self.until = Some(now + span);
            info!("[weather] {} for {:.0} s", self.sky.word(), span);
        }
        let (c, r, m) = self.sky.targets();
        let k = 1.0 - (-3.0 * dt.max(0.0) / EASE).exp();
        self.cloud += (c - self.cloud) * k;
        self.rain += (r - self.rain) * k;
        self.mist += (m - self.mist) * k;
    }
}

/// Share of the sun and of the ambient light full cloud takes away: overcast light is soft (a
/// faint sun, nearly all the ambient), so shadows fade instead of the scene just going dark.
const SUN_DIM: f32 = 0.85;
const AMBIENT_DIM: f32 = 0.05;

/// The hour's light under this weather: (sun lux, ambient brightness, sky colour, fog start, fog
/// end). Clouds dim the sun most and the ambient a little and grey the sky; mist and rain pull the
/// fog in from its clear-day 50..150 m.
pub fn shade(lux: f32, ambient: f32, sky: Color, w: &Weather) -> (f32, f32, Color, f32, f32) {
    let cloud = w.cloud.clamp(0.0, 1.0);
    let mist = w.mist.clamp(0.0, 1.0);
    let rain = w.rain.clamp(0.0, 1.0);
    let s = sky.to_srgba();
    let lum = 0.3 * s.red + 0.59 * s.green + 0.11 * s.blue;
    let grey = lum * 0.92;
    let g = cloud * 0.75;
    let mixed = Color::srgb(s.red + (grey - s.red) * g, s.green + (grey - s.green) * g, s.blue + (grey + 0.02 - s.blue) * g);
    let near = 50.0 + (6.0 - 50.0) * mist;
    let far = 150.0 + (40.0 - 150.0) * mist.max(rain * 0.45);
    (lux * (1.0 - SUN_DIM * cloud), ambient * (1.0 - AMBIENT_DIM * cloud), mixed, near, far.max(near + 10.0))
}

/// Switch to this sky now (a held sky stays held; otherwise it lasts the longest hold).
#[derive(Message, Clone, Copy, Debug)]
pub struct SetSky(pub Sky);

/// One rain streak.
#[derive(Component)]
struct Drop(usize);

pub fn plugin(app: &mut App) {
    let weather = match std::env::var("GB_WEATHER").ok().as_deref().and_then(Sky::parse) {
        Some(sky) => {
            info!("[weather] GB_WEATHER: {} held", sky.word());
            Weather::held(sky)
        }
        None => Weather::default(),
    };
    app.insert_resource(weather)
        .add_message::<SetSky>()
        .add_systems(Startup, spawn_drops)
        .add_systems(Update, (step, fall).chain());
}

fn step(time: Res<Time>, mut sets: MessageReader<SetSky>, mut w: ResMut<Weather>) {
    let now = time.elapsed_secs();
    if let Some(SetSky(sky)) = sets.read().last().copied() {
        w.set(sky, now);
    }
    let dt = time.delta_secs().clamp(0.0, 0.1);
    w.step(now, dt);
}

/// A repeatable spread in -1..1 for drop `i`, axis `k`.
fn spread(i: usize, k: u32) -> f32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x >> 8) as f32 / (1u32 << 23) as f32 - 1.0
}

fn spawn_drops(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mesh = meshes.add(Cuboid::new(0.012, 0.55, 0.012));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.72, 0.78, 0.86, 0.32),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for i in 0..DROPS {
        let at = Vec3::new(spread(i, 1), spread(i, 2), spread(i, 3)) * BOX;
        commands.spawn((
            Name::new("Rain"),
            Drop(i),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_z(-(SLANT / FALL).atan())),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }
}

/// Streaks fall and wrap round the camera; as many show as the rain amount calls for.
fn fall(time: Res<Time>, w: Res<Weather>, camera: Query<&GlobalTransform, With<MainCamera>>, mut drops: Query<(&Drop, &mut Transform, &mut Visibility)>) {
    let Ok(cam) = camera.single() else { return };
    let c = cam.translation();
    let shown = (w.rain.clamp(0.0, 1.0) * DROPS as f32).round() as usize;
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (d, mut tf, mut vis) in &mut drops {
        let want = if d.0 < shown { Visibility::Visible } else { Visibility::Hidden };
        vis.set_if_neq(want);
        if d.0 >= shown {
            continue;
        }
        tf.translation += Vec3::new(SLANT, -FALL, 0.0) * dt;
        // Keep each streak inside the box round the camera, wrapping on every axis.
        let rel = tf.translation - c;
        let wrap = |v: f32, h: f32| (v + h).rem_euclid(2.0 * h) - h;
        tf.translation = c + Vec3::new(wrap(rel.x, BOX.x), wrap(rel.y, BOX.y), wrap(rel.z, BOX.z));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skies_follow_their_usual_successors() {
        for sky in Sky::ALL {
            let total: f32 = sky.followers().iter().map(|(_, w)| w).sum();
            assert!((total - 1.0).abs() < 1e-5, "{sky:?} weights sum to {total}");
            for r in [0.0, 0.3, 0.6, 0.999] {
                let n = next(sky, r);
                assert!(sky.followers().iter().any(|(s, _)| *s == n));
            }
        }
        // Rain never follows fog directly; fog never follows rain.
        assert!((0..100).all(|i| next(Sky::Fog, i as f32 / 100.0) != Sky::Rain));
        assert!((0..100).all(|i| next(Sky::Rain, i as f32 / 100.0) != Sky::Fog));
    }

    #[test]
    fn amounts_ease_toward_the_sky_and_a_held_sky_never_changes() {
        let mut w = Weather { sky: Sky::Rain, until: None, ..default() };
        w.step(0.0, EASE);
        assert!(w.rain > 0.9 && w.cloud > 0.9, "rolled in: {w:?}");
        let mut h = Weather::held(Sky::Fog);
        for t in 0..2000 {
            h.step(t as f32, 1.0);
        }
        assert_eq!(h.sky, Sky::Fog);
        // An unheld sky changes once its hold runs out.
        let mut u = Weather::default();
        let mut changes = 0;
        let mut last = u.sky;
        for t in 0..20_000 {
            u.step(t as f32, 1.0);
            if u.sky != last {
                changes += 1;
                last = u.sky;
            }
        }
        assert!(changes > 10, "{changes} changes in 20000 s");
    }

    #[test]
    fn clouds_dim_and_mist_pulls_the_fog_in() {
        let sky = Color::srgb(0.55, 0.68, 0.82);
        let (lux0, amb0, _, near0, far0) = shade(8000.0, 400.0, sky, &Weather::held(Sky::Clear));
        assert_eq!((lux0, amb0, near0, far0), (8000.0, 400.0, 50.0, 150.0));
        let (lux, amb, grey, ..) = shade(8000.0, 400.0, sky, &Weather::held(Sky::Rain));
        assert!(lux < 2500.0 && amb < 400.0);
        let g = grey.to_srgba();
        assert!((g.red - g.blue).abs() < (0.55f32 - 0.82).abs(), "greyer");
        let (.., near, far) = shade(8000.0, 400.0, sky, &Weather::held(Sky::Fog));
        assert!(near < 10.0 && far < 50.0);
        assert_eq!(Sky::parse(" Rain "), Some(Sky::Rain));
    }

    #[test]
    fn setting_a_sky_switches_now_and_cycles_round() {
        let mut w = Weather::default();
        w.set(Sky::Fog, 10.0);
        assert_eq!(w.sky, Sky::Fog);
        w.step(11.0, 1.0);
        assert_eq!(w.sky, Sky::Fog, "not drawn again before its hold ends");
        let mut held = Weather::held(Sky::Rain);
        held.set(Sky::Clear, 0.0);
        for t in 0..1000 {
            held.step(t as f32, 1.0);
        }
        assert_eq!(held.sky, Sky::Clear, "a held sky stays held");
        let mut s = Sky::Clear;
        for _ in 0..Sky::ALL.len() {
            s = Weather::cycled(s);
        }
        assert_eq!(s, Sky::Clear);
    }

    #[test]
    fn rain_spread_fills_the_box() {
        let v: Vec<f32> = (0..DROPS).map(|i| spread(i, 1)).collect();
        assert!(v.iter().all(|x| (-1.0..=1.0).contains(x)));
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        assert!(mean.abs() < 0.15, "mean {mean}");
    }
}
