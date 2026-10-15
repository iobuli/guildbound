//! F3 debug overlay (off by default): COM, capture point, support centre, feet contact, joint torques,
//! and a text line per body. Gizmos + one bevy_ui text node; no egui.

use bevy::prelude::*;

use crate::body::{Bodies, bv};

#[derive(Resource, Default)]
pub struct Overlay {
    pub on: bool,
}

#[derive(Component)]
struct OverlayText;

pub fn plugin(app: &mut App) {
    app.init_resource::<Overlay>()
        .add_systems(Startup, spawn_text)
        .add_systems(Update, (toggle, draw).chain());
}

fn spawn_text(mut commands: Commands) {
    commands.spawn((
        OverlayText,
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(14.0),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: px(8.0),
            left: px(8.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut o: ResMut<Overlay>) {
    if keys.just_pressed(KeyCode::F3) {
        o.on = !o.on;
    }
}

/// One-line summary of a body's state.
pub fn summary(st: &guildbound_sim::api::BodyState, pelvis_y: f32) -> String {
    format!(
        "pelvis {:.3} m  com {:.3} m  feet {:?}  sat_peak {:.2}  faults {}  stand_failed {}",
        pelvis_y, st.com.y, st.feet_down, st.sat_peak, st.faults, st.stand_failed
    )
}

fn draw(
    o: Res<Overlay>,
    bodies: Res<Bodies>,
    mut gizmos: Gizmos,
    mut text: Query<(&mut Text, &mut Visibility), With<OverlayText>>,
) {
    let Ok((mut t, mut vis)) = text.single_mut() else {
        return;
    };
    *vis = if o.on {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !o.on {
        return;
    }
    let mut s = String::from("F3 overlay  (tick ".to_owned() + &bodies.ticks.to_string() + ")\n");
    for (p, st) in bodies.list.iter().zip(&bodies.last) {
        let com = bv(st.com);
        gizmos.sphere(
            Isometry3d::from_translation(com),
            0.04,
            Color::srgb(1.0, 0.9, 0.1),
        );
        gizmos.line(
            com,
            Vec3::new(com.x, 0.0, com.z),
            Color::srgb(1.0, 0.9, 0.1),
        );
        gizmos.sphere(
            Isometry3d::from_translation(bv(st.capture_point)),
            0.03,
            Color::srgb(0.1, 0.9, 1.0),
        );
        gizmos.sphere(
            Isometry3d::from_translation(bv(st.cop)),
            0.03,
            Color::srgb(0.2, 1.0, 0.2),
        );
        for (side, foot) in p.feet.iter().enumerate() {
            let Some(i) = foot else { continue };
            let c = if st.feet_down[side] {
                Color::srgb(0.1, 1.0, 0.1)
            } else {
                Color::srgb(1.0, 0.1, 0.1)
            };
            if let Some(j) = st.joints.get(*i) {
                gizmos.sphere(Isometry3d::from_translation(bv(j.pos)), 0.025, c);
            }
        }
        for j in &st.joints {
            let tau = bv(j.torque);
            gizmos.line(
                bv(j.pos),
                bv(j.pos) + tau * 0.002,
                Color::srgb(1.0, 0.3 + 0.7 * (1.0 - j.sat as f32).clamp(0.0, 1.0), 0.3),
            );
        }
        let pelvis_y = st.joints.first().map(|j| j.pos.y as f32).unwrap_or(0.0);
        s.push_str(&summary(st, pelvis_y));
        s.push('\n');
    }
    t.0 = s;
}
