use bevy::prelude::*;
use guildbound_sim::{ActionKind, Source, magic::{Element, SPELLS}};

const LIMIT: usize = 96;

#[derive(Resource)]
pub struct FxAssets { mesh: Handle<Mesh>, materials: [[Handle<StandardMaterial>; 3]; 5] }

#[derive(Component)]
pub struct SpellFx(usize);

pub fn setup(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let textures: [Handle<Image>; 3] = ["light_01", "flame_01", "magic_01"].map(|name| assets.load(format!("vfx/kenney/{name}.png")));
    let elements = [Element::Fire, Element::Lightning, Element::Frost, Element::Arcane, Element::Holy];
    let handles = std::array::from_fn(|i| std::array::from_fn(|j| materials.add(StandardMaterial {
        base_color: Color::LinearRgba(crate::magic_view::element_color(elements[i], 0.0)),
        base_color_texture: Some(textures[j].clone()), unlit: true, alpha_mode: AlphaMode::Add, cull_mode: None, ..default()
    })));
    commands.insert_resource(FxAssets { mesh: meshes.add(Rectangle::new(1.0, 1.0)), materials: handles });
}

fn element_index(e: Element) -> usize { match e { Element::Fire => 0, Element::Lightning => 1, Element::Frost => 2, Element::Arcane => 3, Element::Holy => 4 } }

pub fn update(
    mut commands: Commands,
    sim: Res<crate::sim_bridge::Sim>,
    assets: Res<FxAssets>,
    camera: Single<&GlobalTransform, With<crate::camera::OrbitCamera>>,
    orbs: Query<(&GlobalTransform, &PointLight, &Visibility), With<crate::magic_view::Orb>>,
    mut particles: Query<(Entity, &SpellFx, &mut Transform, &mut GlobalTransform, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility), (Without<crate::magic_view::Orb>, Without<crate::camera::OrbitCamera>)>,
) {
    let now = sim.world.tick as f32 * guildbound_sim::STEP;
    let facing = camera.compute_transform().rotation;
    let ground = |x: f32, z: f32, y: f32| Vec3::new(x, guildbound_sim::terrain::ground(x, z) + y, z);
    let mut wanted: Vec<(Vec3, f32, Quat, usize, usize)> = Vec::new();
    for (global, light, visible) in &orbs {
        if *visible == Visibility::Hidden || light.intensity <= 0.0 { continue; }
        let t = global.compute_transform();
        let r = t.scale.x;
        let c = light.color.to_linear();
        let element = if c.red > c.blue { 0 } else if c.red > c.green { 3 } else { 1 };
        wanted.push((t.translation, r * 5.0, facing, element, 0));
        for k in 0..3 {
            let phase = (now * 1.8 + k as f32 / 3.0).fract();
            let angle = now * 3.0 + k as f32 * 2.1;
            let offset = Vec3::new(angle.cos(), phase * 2.0 - 0.4, angle.sin()) * r;
            wanted.push((t.translation + offset, r * (1.3 - phase), facing, element, 1));
        }
    }
    for f in &sim.world.fighters {
        let ActionKind::Cast(slot) = f.action.kind else { continue };
        let element = element_index(SPELLS[slot].element);
        let center = ground(f.position.x, f.position.z, 0.04);
        let pulse = 1.0 + 0.08 * (now * 8.0).sin();
        wanted.push((center, 1.5 * pulse, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2), element, 2));
        if SPELLS[slot].element == Element::Holy {
            for k in 0..8 {
                let phase = (now * 0.8 + k as f32 / 8.0).fract();
                let angle = k as f32 * std::f32::consts::TAU / 8.0;
                wanted.push((center + Vec3::new(angle.cos() * 0.55, phase * 1.8, angle.sin() * 0.55), 0.25 * (1.0 - phase), facing, element, 0));
            }
        }
    }
    for (contact, at) in &sim.recent {
        let Source::Spell(slot) = contact.source else { continue };
        let age = now - at;
        if !(0.0..0.45).contains(&age) { continue; }
        let element = element_index(SPELLS[slot].element);
        let center = ground(contact.point.x, contact.point.z, 1.1);
        let fade = 1.0 - age / 0.45;
        wanted.push((center, (0.7 + age * 3.0) * fade, facing, element, 0));
        for k in 0..6 {
            let angle = k as f32 * std::f32::consts::TAU / 6.0;
            let spread = 0.1 + age * 3.5;
            let offset = Vec3::new(angle.cos(), angle.sin(), (angle * 2.0).cos() * 0.3) * spread;
            wanted.push((center + offset, 0.28 * fade, facing, element, if element == 0 { 1 } else { 0 }));
        }
    }
    wanted.truncate(LIMIT);
    let mut have: Vec<_> = particles.iter_mut().collect();
    have.sort_by_key(|row| row.1.0);
    for (i, (pos, size, rotation, element, texture)) in wanted.iter().enumerate() {
        let transform = Transform::from_translation(*pos).with_rotation(*rotation).with_scale(Vec3::splat(*size));
        let material = assets.materials[*element][*texture].clone();
        if let Some((_, _, t, global, mat, visible)) = have.get_mut(i) {
            **t = transform;
            **global = GlobalTransform::from(transform);
            if mat.0 != material { mat.0 = material; }
            **visible = Visibility::Inherited;
        } else {
            commands.spawn((SpellFx(i), crate::toon::NoToon, Mesh3d(assets.mesh.clone()), MeshMaterial3d(material), transform, GlobalTransform::from(transform)));
        }
    }
    for (_, index, _, _, _, visible) in &mut have {
        if index.0 >= wanted.len() { **visible = Visibility::Hidden; }
    }
}
