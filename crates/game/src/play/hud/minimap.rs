//! The round map, bottom left, RuneScape style: turned so the camera looks up, centred on the
//! player, `RANGE` metres to the rim. The terrain is baked once at startup from `world`'s
//! analytic ground (`ground_height`) into a data texture (R contour lines every 0.5 m, heavier
//! every 2.5 m, and the courtyard edge; G hill shade; B water; A land) that `gbui`'s ring material
//! colours from the theme, so theme edits recolour it live. Over it: dots (NPCs, items, trees,
//! other things to use), the player's arrow (its facing relative to the camera), an N on the rim,
//! and a readout below (position and camera heading).
use super::MARGIN;
use crate::play::camera::CameraRig;
use crate::play::items::Item;
use crate::play::world::{TERRAIN_HALF, TrainingDummy, WATER_LEVEL, courtyard_distance, ground_height};
use crate::play::{Interactable, Mover, Npc, Player};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use gbui::prelude::*;
use gbui::ring::to_screen;
use std::f32::consts::FRAC_PI_4;

/// Diameter (px).
pub const SIZE: f32 = 184.0;
/// Metres from the centre to the rim.
const RANGE: f32 = 34.0;
/// Baked area: a square from `MAP_MIN` spanning `MAP_SPAN` metres, `RES` texels a side.
const MAP_MIN: f32 = -64.0;
const MAP_SPAN: f32 = 128.0;
const RES: usize = 512;
/// Contour interval and heavy-line interval (m).
const CONTOUR: f32 = 0.5;
const CONTOUR_MAJOR: f32 = 2.5;
const DOTS: usize = 48;
const DOT: f32 = 6.0;
/// Dots stop this far inside the rim (px), the N mark sits this far in.
const DOT_INSET: f32 = 8.0;
const NORTH_INSET: f32 = 15.0;
/// Readout line height under the map (px).
const READOUT: f32 = 16.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(PostUpdate, update.before(GbuiSet::Animate));
}

#[derive(Resource)]
struct MapMaterial(Handle<RingMap>);

#[derive(Component)]
struct MapDot(usize);

#[derive(Component)]
struct PlayerArrow;

#[derive(Component)]
struct NorthMark;

#[derive(Component)]
struct Readout;

/// World (x, z) → map uv.
fn to_uv(x: f32, z: f32) -> Vec2 {
    Vec2::new((x - MAP_MIN) / MAP_SPAN, (z - MAP_MIN) / MAP_SPAN)
}

/// The terrain as data texels (see the module doc).
fn bake() -> Vec<u8> {
    let cell = MAP_SPAN / RES as f32;
    let at = |i: usize| MAP_MIN + (i as f32 + 0.5) * cell;
    let heights: Vec<f32> = (0..RES * RES).map(|k| ground_height(at(k % RES), at(k / RES))).collect();
    let h = |i: usize, j: usize| heights[j.min(RES - 1) * RES + i.min(RES - 1)];
    let light = Vec2::new(0.6, 0.8); // from +z, a little +x
    let mut data = vec![0u8; RES * RES * 4];
    for j in 0..RES {
        for i in 0..RES {
            let (x, z) = (at(i), at(j));
            let here = h(i, j);
            let across = |step: f32| {
                let l = (here / step).floor();
                l != (h(i + 1, j) / step).floor() || l != (h(i, j + 1) / step).floor()
            };
            let mut contour: f32 = if across(CONTOUR_MAJOR) {
                1.0
            } else if across(CONTOUR) {
                0.42
            } else {
                0.0
            };
            // the courtyard's edge, dashed
            if courtyard_distance(x, z).abs() < cell * 0.75 && ((x + z) * 0.8).rem_euclid(2.0) < 1.2 {
                contour = contour.max(0.75);
            }
            let slope = Vec2::new(h(i.saturating_sub(1), j) - h(i + 1, j), h(i, j.saturating_sub(1)) - h(i, j + 1)) / (2.0 * cell);
            let shade = (0.5 + 1.4 * slope.dot(light)).clamp(0.0, 1.0);
            let water = if here < WATER_LEVEL {
                if (i + j) % 7 < 2 { 1.0 } else { 0.72 }
            } else {
                0.0
            };
            let land = 1.0 - ((x.abs().max(z.abs()) - TERRAIN_HALF) / 3.0).clamp(0.0, 1.0);
            let k = (j * RES + i) * 4;
            for (c, v) in [contour, shade, water, land].into_iter().enumerate() {
                data[k + c] = (v * 255.0).round() as u8;
            }
        }
    }
    data
}

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<RingMap>>, theme: Res<Theme>) {
    let image = Image::new(
        Extent3d {
            width: RES as u32,
            height: RES as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bake(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let material = materials.add(RingMap {
        params: RingParams::new(&theme.ring, to_uv(0.0, 0.0), 0.0, RANGE / MAP_SPAN),
        map: images.add(image),
    });
    commands.insert_resource(MapMaterial(material.clone()));
    let centred = |w: f32| Node {
        position_type: PositionType::Absolute,
        left: px(SIZE * 0.5 - w * 0.5),
        top: px(SIZE * 0.5 - w * 0.5),
        width: px(w),
        height: px(w),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    commands
        .spawn((
            MaterialNode(material),
            Node {
                position_type: PositionType::Absolute,
                left: px(MARGIN),
                bottom: px(MARGIN + READOUT),
                width: px(SIZE),
                height: px(SIZE),
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|map| {
            for i in 0..DOTS {
                map.spawn((MapDot(i), centred(DOT), Styled::new("map.poi"), Visibility::Hidden));
            }
            map.spawn((PlayerArrow, centred(20.0))).with_child(label("▲", "map.player"));
            map.spawn((NorthMark, centred(16.0))).with_child(label("N", "map.north"));
        });
    commands.spawn((
        Readout,
        label("", "map.readout"),
        Node {
            position_type: PositionType::Absolute,
            left: px(MARGIN),
            bottom: px(MARGIN - 2.0),
            width: px(SIZE),
            justify_content: JustifyContent::Center,
            ..default()
        },
        TextLayout::justify(Justify::Center),
        GlobalZIndex(20),
    ));
}

/// Compass point of a camera yaw (north = +Z; east = −X, on the right when facing north).
fn heading(yaw: f32) -> &'static str {
    const POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    POINTS[((-yaw / FRAC_PI_4).round() as i32).rem_euclid(8) as usize]
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update(
    rig: Res<CameraRig>,
    theme: Res<Theme>,
    material: Option<Res<MapMaterial>>,
    mut materials: ResMut<Assets<RingMap>>,
    player: Query<(&Transform, &Mover), With<Player>>,
    npcs: Query<&GlobalTransform, (With<Npc>, Without<Player>)>,
    dummies: Query<&GlobalTransform, With<TrainingDummy>>,
    items: Query<&GlobalTransform, With<Item>>,
    things: Query<(&GlobalTransform, &Interactable)>,
    mut dots: Query<(&MapDot, &mut UiTransform, &mut Visibility, &mut Styled), (Without<PlayerArrow>, Without<NorthMark>)>,
    mut arrow: Query<&mut UiTransform, (With<PlayerArrow>, Without<MapDot>, Without<NorthMark>)>,
    mut north: Query<&mut UiTransform, (With<NorthMark>, Without<MapDot>, Without<PlayerArrow>)>,
    mut readout: Query<&mut Text, With<Readout>>,
    mut marks: Local<Vec<(u8, f32, Vec2, &'static str)>>,
) {
    let Ok((transform, mover)) = player.single() else { return };
    let here = Vec2::new(transform.translation.x, transform.translation.z);
    let yaw = rig.yaw;

    if let Some(material) = material {
        let want = RingParams::new(&theme.ring, to_uv(here.x, here.y), yaw, RANGE / MAP_SPAN);
        if materials.get(&material.0).is_some_and(|m| m.params != want)
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            m.params = want;
        }
    }

    // What to mark, most important first, nearest first within a kind.
    marks.clear();
    let mut mark = |rank: u8, at: Vec3, class: &'static str| {
        let offset = Vec2::new(at.x, at.z) - here;
        let d = offset.length();
        if d > 0.5 && d < RANGE {
            marks.push((rank, d, offset, class));
        }
    };
    for t in &npcs {
        mark(0, t.translation(), "map.npc");
    }
    for t in &dummies {
        mark(1, t.translation(), "map.npc");
    }
    for t in &items {
        mark(2, t.translation(), "map.item");
    }
    for (t, it) in &things {
        mark(3, t.translation(), if it.kind == "tree" { "map.tree" } else { "map.poi" });
    }
    marks.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let radius = SIZE * 0.5 - DOT_INSET;
    for (dot, mut ui, mut vis, mut styled) in &mut dots {
        match marks.get(dot.0) {
            Some(&(_, _, offset, class)) => {
                let p = to_screen(offset, yaw, RANGE, SIZE * 0.5);
                if p.length() > radius {
                    vis.set_if_neq(Visibility::Hidden);
                    continue;
                }
                ui.translation = Val2::px(p.x, p.y);
                vis.set_if_neq(Visibility::Inherited);
                if styled.0 != class {
                    styled.0 = class.into();
                }
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
    for mut ui in &mut arrow {
        ui.rotation = Rot2::radians(yaw - mover.yaw);
    }
    for mut ui in &mut north {
        let r = SIZE * 0.5 - NORTH_INSET;
        ui.translation = Val2::px(yaw.sin() * r, -yaw.cos() * r);
    }
    let line = format!("{:>6.1} {:>6.1}   {:<2}", here.x, here.y, heading(yaw));
    for mut text in &mut readout {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn heading_names_the_camera_direction() {
        assert_eq!(heading(0.0), "N");
        assert_eq!(heading(-FRAC_PI_2), "E", "facing -X is east");
        assert_eq!(heading(FRAC_PI_2), "W");
        assert_eq!(heading(PI), "S");
        assert_eq!(heading(-FRAC_PI_4), "NE");
    }

    #[test]
    fn bake_marks_water_land_and_contours() {
        let data = bake();
        let texel = |x: f32, z: f32| {
            let uv = to_uv(x, z);
            let (i, j) = ((uv.x * RES as f32) as usize, (uv.y * RES as f32) as usize);
            let k = (j * RES + i) * 4;
            [data[k], data[k + 1], data[k + 2], data[k + 3]]
        };
        let pond = crate::play::world::POND_CENTER;
        assert!(texel(pond.x, pond.y)[2] > 150, "pond centre is water");
        assert_eq!(texel(0.0, 0.0)[2], 0, "courtyard is dry");
        assert_eq!(texel(0.0, 0.0)[3], 255, "inside the map is land");
        assert_eq!(texel(63.5, 63.5)[3], 0, "past the rim is not");
        let contours = data.chunks(4).filter(|t| t[0] > 0).count();
        assert!(contours > RES * 4, "the hills draw contour lines ({contours})");
    }
}
