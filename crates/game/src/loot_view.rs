//! Pickups and chests on the floor. Views are rebuilt only when the sim bumps `loot_rev`.
use bevy::prelude::*;
use guildbound_sim::rpg::{ITEMS, ItemKind, Slot};

use crate::sim_bridge::Sim;

#[derive(Component)]
pub struct LootView;

/// Spins and bobs; index into the sim's pickups.
#[derive(Component)]
pub struct PickupBob(pub usize);

fn item_color(id: u8) -> Color {
    match ITEMS[id as usize].kind {
        ItemKind::Consumable { heal, .. } if heal > 0.0 => Color::srgb(0.9, 0.25, 0.25),
        ItemKind::Consumable { .. } => Color::srgb(0.95, 0.75, 0.25),
        ItemKind::Gear(Slot::Hands) => Color::srgb(0.75, 0.75, 0.8),
        ItemKind::Gear(_) => Color::srgb(0.45, 0.65, 0.95),
    }
}

pub fn sync(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sim: Res<Sim>,
    views: Query<Entity, With<LootView>>,
    mut seen: Local<Option<u32>>,
) {
    let w = &sim.world;
    if *seen == Some(w.loot_rev) {
        return;
    }
    *seen = Some(w.loot_rev);
    for e in &views {
        commands.entity(e).despawn();
    }
    let gem = meshes.add(Cuboid::new(0.22, 0.22, 0.22));
    let flask = meshes.add(Cylinder::new(0.09, 0.26));
    for (k, p) in w.pickups.iter().enumerate() {
        let color = item_color(p.item);
        let mat = materials.add(StandardMaterial { base_color: color, emissive: (color.to_linear() * 2.0).into(), ..default() });
        let mesh = if matches!(ITEMS[p.item as usize].kind, ItemKind::Consumable { .. }) { flask.clone() } else { gem.clone() };
        commands.spawn((LootView, PickupBob(k), Mesh3d(mesh), MeshMaterial3d(mat), Transform::from_xyz(p.position.x, 0.35 + guildbound_sim::terrain::ground(p.position.x, p.position.z), p.position.z)));
    }
    let wood = materials.add(StandardMaterial { base_color: Color::srgb(0.42, 0.27, 0.14), perceptual_roughness: 0.8, ..default() });
    let trim = materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.62, 0.25), metallic: 0.8, perceptual_roughness: 0.4, ..default() });
    let body = meshes.add(Cuboid::new(0.9, 0.5, 0.6));
    let lid = meshes.add(Cuboid::new(0.92, 0.12, 0.62));
    let band = meshes.add(Cuboid::new(0.94, 0.06, 0.08));
    for c in &w.chests {
        // lid hinged at the back edge; open = tipped back
        let lid_rot = if c.open { Quat::from_rotation_x(-1.9) } else { Quat::IDENTITY };
        commands
            .spawn((LootView, Transform::from_xyz(c.position.x, guildbound_sim::terrain::ground(c.position.x, c.position.z), c.position.z), Visibility::default()))
            .with_children(|p| {
                p.spawn((Mesh3d(body.clone()), MeshMaterial3d(wood.clone()), Transform::from_xyz(0.0, 0.25, 0.0)));
                p.spawn((Mesh3d(band.clone()), MeshMaterial3d(trim.clone()), Transform::from_xyz(0.0, 0.42, 0.0)));
                p.spawn((Transform::from_xyz(0.0, 0.5, -0.31).with_rotation(lid_rot), Visibility::default()))
                    .with_child((Mesh3d(lid.clone()), MeshMaterial3d(wood.clone()), Transform::from_xyz(0.0, 0.06, 0.31)));
            });
    }
}

pub fn bob(time: Res<Time>, sim: Res<Sim>, mut q: Query<(&PickupBob, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (b, mut tr) in &mut q {

        tr.rotation = Quat::from_rotation_y(t * 1.4 + b.0 as f32);
        if let Some(p) = sim.world.pickups.get(b.0) {
            tr.translation.y = 0.35 + 0.06 * (t * 2.5 + b.0 as f32).sin() + guildbound_sim::terrain::ground(p.position.x, p.position.z);
            tr.translation.x = p.position.x;
            tr.translation.z = p.position.z;
        }
    }
}
