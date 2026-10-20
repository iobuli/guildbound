//! The outer regions: every imported pack placed on the bigger map from `assets/data/regions.json`
//! (written by tools/kev/layout.py from the GLBs' measured bounds).
//!
//! - Each item is a glTF scene at its `at` footprint, standing on the terrain (`world::ground_height`
//!   plus its `lift`), turned by `yaw`, scaled by `scale`.
//! - Items with `half` get a static box collider, a separate entity at the box centre, so the box
//!   never inherits the scene's scale.
//! - Packs are local-only (licences forbid redistributing some), so an item whose file is not on
//!   disk is skipped; the map still builds without them.
//! - The mech statue's file holds three body sets (Base_S / Base_M / Base_L) on one skeleton: only
//!   the medium set shows (`HIDE`).
//! - Pads flatten the ground under each region (`world::PADS`).
use super::world::{ground_height, world_layers};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;

const FILE: &str = "data/regions.json";
/// Scene node name prefixes hidden under region items (the mech's spare body sets).
const HIDE: [&str; 2] = ["Base_S_", "Base_L_"];

#[derive(serde::Deserialize, Clone, Debug)]
pub struct Item {
    pub region: String,
    pub path: String,
    pub at: [f32; 2],
    pub yaw: f32,
    pub scale: f32,
    pub lift: f32,
    pub half: Option<[f32; 3]>,
    pub offset: [f32; 2],
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct File {
    pub items: Vec<Item>,
}

/// A placed region item (the scene root).
#[derive(Component)]
pub struct RegionItem;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, hide_spares);
}

pub fn load(root: &std::path::Path) -> Result<File, String> {
    let path = root.join(FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Root transform of an item: footprint on the ground, lifted so its box bottom meets it.
pub fn place(item: &Item) -> Transform {
    let (x, z) = (item.at[0], item.at[1]);
    Transform::from_xyz(x, ground_height(x, z) + item.lift, z)
        .with_rotation(Quat::from_rotation_y(item.yaw.to_radians()))
        .with_scale(Vec3::splat(item.scale))
}

fn spawn(mut commands: Commands, assets: Res<AssetServer>) {
    let root = std::path::Path::new(crate::ASSETS);
    let file = match load(root) {
        Ok(f) => f,
        Err(e) => {
            warn!("[regions] {e}");
            return;
        }
    };
    let (mut placed, mut missing) = (0, 0);
    for item in &file.items {
        if !root.join(&item.path).exists() {
            missing += 1;
            continue;
        }
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(item.path.clone()));
        let name = item.path.rsplit('/').next().unwrap_or(&item.path).trim_end_matches(".glb").to_string();
        commands.spawn((Name::new(name.clone()), RegionItem, WorldAssetRoot(scene), place(item), Visibility::default()));
        if let Some(h) = item.half {
            let (x, z) = (item.at[0] + item.offset[0], item.at[1] + item.offset[1]);
            commands.spawn((
                Name::new(format!("{name} collider")),
                Transform::from_xyz(x, ground_height(x, z) + h[1], z).with_rotation(Quat::from_rotation_y(item.yaw.to_radians())),
                RigidBody::Static,
                Collider::cuboid(h[0] * 2.0, h[1] * 2.0, h[2] * 2.0),
                world_layers(),
            ));
        }
        placed += 1;
    }
    info!("[regions] placed {placed} items ({missing} skipped: pack not on disk)");
}

/// Hide the spare body sets of multi-set scenes once their nodes spawn.
fn hide_spares(mut named: Query<(Entity, &Name, &mut Visibility), Added<Name>>, parents: Query<&ChildOf>, items: Query<(), With<RegionItem>>) {
    for (e, name, mut vis) in &mut named {
        if HIDE.iter().any(|p| name.as_str().starts_with(p)) && parents.iter_ancestors(e).any(|a| items.contains(a)) {
            *vis = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_file_parses_and_names_packs() {
        let file = load(std::path::Path::new(crate::ASSETS)).expect("regions.json");
        assert!(file.items.len() > 50, "{} items", file.items.len());
        for r in ["foundry", "market", "plaza"] {
            assert!(file.items.iter().any(|i| i.region == r), "no {r} items");
        }
        for i in &file.items {
            assert!(i.path.starts_with("packs/") && i.path.ends_with(".glb"), "{}", i.path);
            assert!(i.scale > 0.0 && i.scale.is_finite());
            if let Some(h) = i.half {
                assert!(h.iter().all(|v| *v > 0.0), "{}: {h:?}", i.path);
            }
        }
    }

    #[test]
    fn items_stand_on_the_ground_inside_their_pad() {
        let file = load(std::path::Path::new(crate::ASSETS)).unwrap();
        for i in &file.items {
            let tf = place(i);
            let ground = ground_height(i.at[0], i.at[1]);
            assert!((tf.translation.y - i.lift - ground).abs() < 1e-4);
            assert!(super::super::world::in_pad(i.at[0], i.at[1], 4.0), "{} at {:?} is off its pad", i.path, i.at);
        }
    }
}
