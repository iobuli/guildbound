//! Debug toggles: one registry of views (`DebugToggles`, written here only). F12 shows the list
//! (top right); while it is open, 1–9 and 0 flip toggles 1–10 and Shift+1–3 flip 11–13.
//! `GB_DEBUG=1,2,4` turns toggles on at start (smoke runs and screenshots).
//! Views that belong to a system live with it and read the flag (`senses`: vision volumes, sense
//! rays, aura radius, awareness; `wind_aura`: the test shell). The general ones are drawn here
//! with gizmos, each only while its flag is on: AI intent, IK targets, grass pushers and gusts,
//! balance tilt, projectile trails.
use super::Npc;
use super::beam::Balancing;
use super::grass::GrassWind;
use super::select::{FootTargets, HandTargets};
use super::{HALF_HEIGHT, Intent, Mover};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use gbui::prelude::*;
use std::collections::VecDeque;

/// Every debug view, in list order (toggle number = index + 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    VisionVolumes,
    SenseRays,
    AuraRadius,
    Awareness,
    AiIntent,
    IkTargets,
    Grass,
    Balance,
    Projectiles,
    WindShell,
    ShaderMode,
    GroundLayers,
    ModelPieces,
}

impl View {
    pub const ALL: [View; 13] = [
        View::VisionVolumes,
        View::SenseRays,
        View::AuraRadius,
        View::Awareness,
        View::AiIntent,
        View::IkTargets,
        View::Grass,
        View::Balance,
        View::Projectiles,
        View::WindShell,
        View::ShaderMode,
        View::GroundLayers,
        View::ModelPieces,
    ];

    pub fn label(self) -> &'static str {
        match self {
            View::VisionVolumes => "vision volumes",
            View::SenseRays => "sense rays",
            View::AuraRadius => "aura radius",
            View::Awareness => "awareness",
            View::AiIntent => "AI intent",
            View::IkTargets => "IK targets",
            View::Grass => "grass pushers / gust",
            View::Balance => "balance tilt",
            View::Projectiles => "projectile trails",
            View::WindShell => "wind-aura test shell",
            View::ShaderMode => "alt skin shader",
            View::GroundLayers => "ground layers",
            View::ModelPieces => "hide model pieces",
        }
    }

    /// The key that flips this view while the list is open (1–9, 0, then Shift+1–3).
    pub fn key(self) -> (bool, KeyCode) {
        const DIGITS: [KeyCode; 10] = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
            KeyCode::Digit0,
        ];
        let i = View::ALL.iter().position(|v| *v == self).unwrap_or(0);
        if i < 10 { (false, DIGITS[i]) } else { (true, DIGITS[i - 10]) }
    }

    fn key_label(self) -> String {
        let (shift, key) = self.key();
        let d = format!("{key:?}").replace("Digit", "");
        if shift { format!("⇧{d}") } else { d }
    }
}

#[derive(Resource, Default)]
pub struct DebugToggles {
    on: HashMap<View, bool>,
    pub list_open: bool,
}

impl DebugToggles {
    pub fn get(&self, v: View) -> bool {
        self.on.get(&v).copied().unwrap_or(false)
    }
    pub fn flip(&mut self, v: View) {
        let now = !self.get(v);
        self.on.insert(v, now);
    }
    /// Toggles from `GB_DEBUG=1,2,4` (1-based numbers).
    fn from_env(spec: &str) -> Self {
        let mut t = Self::default();
        for n in spec.split(',').filter_map(|p| p.trim().parse::<usize>().ok()) {
            if let Some(v) = View::ALL.get(n.wrapping_sub(1)) {
                t.on.insert(*v, true);
            }
        }
        t
    }
}

/// Run condition: the view is on.
pub fn shown(v: View) -> impl Fn(Res<DebugToggles>) -> bool {
    move |t: Res<DebugToggles>| t.get(v)
}

pub fn plugin(app: &mut App) {
    let mut toggles = std::env::var("GB_DEBUG").map(|s| DebugToggles::from_env(&s)).unwrap_or_default();
    toggles.list_open = std::env::var("GB_DEBUG_LIST").is_ok_and(|v| v.trim() == "1");
    app.insert_resource(toggles)
        .add_systems(Startup, spawn_list)
        .add_systems(Update, (keys, show_list).chain())
        .add_systems(
            Update,
            (
                ai_intent.run_if(shown(View::AiIntent)),
                ik_targets.run_if(shown(View::IkTargets)),
                grass.run_if(shown(View::Grass)),
                balance.run_if(shown(View::Balance)),
                projectiles.run_if(shown(View::Projectiles)),
            ),
        );
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut t: ResMut<DebugToggles>, talking: Option<Res<super::talk::Talking>>) {
    if keys.just_pressed(KeyCode::F12) {
        t.list_open = !t.list_open;
    }
    // Talk options own the digits while talking.
    if !t.list_open || talking.is_some_and(|t| t.active()) {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for v in View::ALL {
        let (needs_shift, key) = v.key();
        if needs_shift == shift && keys.just_pressed(key) {
            t.flip(v);
            info!("[debug] {} {}", v.label(), if t.get(v) { "on" } else { "off" });
        }
    }
}

#[derive(Component)]
struct List;
#[derive(Component)]
struct ListText;

fn spawn_list(mut commands: Commands) {
    commands
        .spawn((
            List,
            panel(
                "stats",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    right: px(16.0),
                    top: px(16.0),
                    width: px(300.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(12.0)),
                    row_gap: px(4.0),
                    ..default()
                },
            ),
            GlobalZIndex(60),
        ))
        .with_children(|p| {
            p.spawn(label("▌DEBUG  [F12]", "title"));
            p.spawn((ListText, label("", "row.text")));
        });
}

/// The list text: one row per view, its key and whether it is on.
pub fn list_text(t: &DebugToggles) -> String {
    View::ALL
        .iter()
        .map(|v| format!("{:>2}  {} {}", v.key_label(), if t.get(*v) { "■" } else { "□" }, v.label()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn show_list(t: Res<DebugToggles>, mut root: Query<&mut Node, With<List>>, mut text: Query<&mut Text, With<ListText>>) {
    if !t.is_changed() {
        return;
    }
    for mut node in &mut root {
        node.display = if t.list_open { Display::Flex } else { Display::None };
    }
    let s = list_text(&t);
    for mut tx in &mut text {
        tx.0 = s.clone();
    }
}

/// Each NPC's wanted move (green arrow) and its target (orange line).
fn ai_intent(mut gizmos: Gizmos, npcs: Query<(&Transform, &Intent), With<Npc>>, globals: Query<&GlobalTransform>) {
    for (tf, intent) in &npcs {
        let at = tf.translation + Vec3::Y * 0.2;
        if intent.move_dir.length_squared() > 1e-4 {
            gizmos.arrow(at, at + intent.move_dir * 1.5, Color::srgb(0.3, 1.0, 0.4));
        }
        if let Some(target) = intent.target.and_then(|t| globals.get(t).ok()) {
            gizmos.line(at + Vec3::Y * 0.6, target.translation() + Vec3::Y * 0.8, Color::srgb(1.0, 0.6, 0.2));
        }
    }
}

/// Hand (cyan) and foot (magenta) IK targets, sized by weight.
fn ik_targets(mut gizmos: Gizmos, hands: Query<&HandTargets>, feet: Query<&FootTargets>) {
    for h in &hands {
        for (p, w) in h.hands.iter().flatten() {
            gizmos.sphere(Isometry3d::from_translation(*p), 0.04 + 0.06 * w, Color::srgb(0.2, 0.9, 1.0));
        }
    }
    for f in &feet {
        for (p, w) in f.feet.iter().flatten() {
            gizmos.sphere(Isometry3d::from_translation(*p), 0.04 + 0.06 * w, Color::srgb(1.0, 0.3, 0.9));
        }
    }
}

/// Where bodies push the grass, and the magic gust.
fn grass(mut gizmos: Gizmos, wind: Res<GrassWind>, movers: Query<&Transform, With<Mover>>) {
    let flat = |p: Vec3| Isometry3d::new(p, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    for tf in &movers {
        gizmos.circle(flat(tf.translation - Vec3::Y * (HALF_HEIGHT - 0.03)), 0.6, Color::srgb(0.6, 1.0, 0.3));
    }
    let g = wind.gust;
    if g.radius > 0.0 && g.strength != 0.0 {
        gizmos.circle(flat(g.centre - Vec3::Y * (HALF_HEIGHT - 0.05)), g.radius, Color::srgb(0.4, 0.9, 1.0));
    }
    let d = wind.direction.normalize_or(Vec2::X);
    if let Some(tf) = movers.iter().next() {
        let at = tf.translation + Vec3::Y * 1.4;
        gizmos.arrow(at, at + Vec3::new(d.x, 0.0, d.y) * (0.5 + wind.strength), Color::srgb(0.7, 0.9, 1.0));
    }
}

/// A tilt dial over each walker on a beam: the bar leans with the sway, red past the wobble line.
fn balance(mut gizmos: Gizmos, walkers: Query<(&Transform, &Balancing, &Mover)>) {
    for (tf, b, mover) in &walkers {
        let head = tf.translation + Vec3::Y * 1.3;
        let right = Vec3::new(mover.yaw.cos(), 0.0, -mover.yaw.sin());
        let lean = (Vec3::Y * b.tilt.cos() + right * b.tilt.sin()) * 0.5;
        let colour = if b.tilt.abs() > 0.7 { Color::srgb(1.0, 0.25, 0.2) } else { Color::srgb(0.9, 0.9, 0.3) };
        gizmos.line(head, head + lean, colour);
        gizmos.line(head - right * 0.3, head + right * 0.3, Color::srgb(0.5, 0.5, 0.5));
    }
}

/// Arrows and spells in flight, with a short trail.
fn projectiles(
    mut gizmos: Gizmos,
    mut trails: Local<HashMap<Entity, VecDeque<Vec3>>>,
    shots: Query<(Entity, &GlobalTransform), Or<(With<super::ranged::Shot>, With<super::magic::Projectile>)>>,
) {
    let live: Vec<Entity> = shots.iter().map(|(e, _)| e).collect();
    trails.retain(|e, _| live.contains(e));
    for (e, g) in &shots {
        let trail = trails.entry(e).or_default();
        trail.push_back(g.translation());
        if trail.len() > 40 {
            trail.pop_front();
        }
        gizmos.linestrip(trail.iter().copied(), Color::srgb(1.0, 0.9, 0.4));
        gizmos.sphere(Isometry3d::from_translation(g.translation()), 0.08, Color::srgb(1.0, 0.5, 0.2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_view_has_its_own_key() {
        let keys: Vec<(bool, KeyCode)> = View::ALL.iter().map(|v| v.key()).collect();
        for (i, k) in keys.iter().enumerate() {
            assert!(!keys[i + 1..].contains(k), "duplicate key for {:?}", View::ALL[i]);
        }
        assert_eq!(View::VisionVolumes.key(), (false, KeyCode::Digit1));
        assert_eq!(View::WindShell.key(), (false, KeyCode::Digit0));
        assert_eq!(View::ShaderMode.key(), (true, KeyCode::Digit1));
    }

    #[test]
    fn env_spec_turns_views_on() {
        let t = DebugToggles::from_env("1, 4,99,x");
        assert!(t.get(View::VisionVolumes) && t.get(View::Awareness));
        assert!(!t.get(View::SenseRays));
        assert!(list_text(&t).lines().count() == View::ALL.len());
    }
}
