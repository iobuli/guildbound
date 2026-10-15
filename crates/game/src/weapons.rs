use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use guildbound_sim::rpg::Weapon;
use bevy_egui::egui;

#[derive(Resource)]
pub struct WeaponGrip { position: Vec3, rotation: Vec3, scale: f32 }

impl Default for WeaponGrip {
    fn default() -> Self {
        let saved = crate::read_json("assets/weapon_grip.json");
        let vector = |key: &str, fallback: Vec3| saved[key].as_array().filter(|a| a.len() == 3)
            .and_then(|a| Some(Vec3::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32)))
            .filter(|v| v.is_finite()).unwrap_or(fallback);
        Self { position: vector("position", Vec3::new(0.0, 0.075, 0.015)), rotation: vector("rotation", Vec3::new(0.0, 0.0, 90.0)), scale: saved["scale"].as_f64().map_or(1.0, |v| v as f32).clamp(0.25, 2.0) }
    }
}

impl WeaponGrip {
    fn transform(&self) -> Transform {
        Transform::from_translation(self.position).with_rotation(Quat::from_euler(EulerRot::XYZ, self.rotation.x.to_radians(), self.rotation.y.to_radians(), self.rotation.z.to_radians())).with_scale(Vec3::splat(self.scale))
    }
}

#[derive(Component)]
pub struct HeldWeapon { root: Entity, hand: Entity, kind: Weapon }

pub fn sync(
    mut commands: Commands,
    sim: Res<crate::sim_bridge::Sim>,
    assets: Res<AssetServer>,
    rigs: Query<(Entity, &crate::anim::FighterRig)>,
    stages: Query<(Entity, &crate::studio::StudioRig)>,
    studio: Res<crate::studio::Studio>,
    grip: Res<WeaponGrip>,
    bones: Query<&Name>,
    mut held: Query<(Entity, &HeldWeapon, &mut Transform)>,
) {
    let mut desired: Vec<_> = rigs.iter().filter_map(|(root, rig)| Some((root, sim.world.fighters.get(rig.fighter)?.kit.weapon()?, rig.bones.clone()))).collect();
    if studio.open && let Some(kind) = studio.preview_weapon && let Some(root) = studio.entity
        && let Ok((_, rig)) = stages.get(root) { desired.push((root, kind, rig.bones.clone())); }
    let pose = grip.transform();
    for (entity, weapon, mut transform) in &mut held {
        if !desired.iter().any(|(root, kind, _)| *root == weapon.root && *kind == weapon.kind) || !bones.contains(weapon.hand) {
            commands.entity(entity).despawn();
        } else if *transform != pose { *transform = pose; }
    }
    for (root, kind, joints) in desired {
        let Some(hand) = joints.iter().copied().find(|&e| bones.get(e).is_ok_and(|n| n.as_str() == "B_R_Hand")) else { continue };
        if held.iter().any(|(_, w, _)| w.root == root && w.kind == kind && w.hand == hand) { continue; }
        let model = match kind { Weapon::Sword => "Sword_Bronze", Weapon::Axe => "Axe_Bronze" };
        let path = format!("packs/_sources/quaternius/Fantasy_Props_MegaKit/Exports/glTF/{model}.gltf");
        let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        commands.entity(hand).with_child((
            Name::new("held weapon"), HeldWeapon { root, hand, kind }, WorldAssetRoot(scene), pose,
        ));
    }
}

pub fn ui(ui: &mut egui::Ui, studio: &mut crate::studio::Studio, grip: &mut WeaponGrip) {
    crate::ui_theme::section(ui, "Weapon preview");
    egui::ComboBox::from_id_salt("preview_weapon").selected_text(match studio.preview_weapon { None => "No weapon", Some(Weapon::Sword) => "Sword", Some(Weapon::Axe) => "Axe" }).show_ui(ui, |ui| {
        ui.selectable_value(&mut studio.preview_weapon, None, "No weapon");
        ui.selectable_value(&mut studio.preview_weapon, Some(Weapon::Sword), "Sword");
        ui.selectable_value(&mut studio.preview_weapon, Some(Weapon::Axe), "Axe");
    });
    if studio.preview_weapon.is_none() { return; }
    crate::ui_theme::hint(ui, "Scrub a sword clip to check the grip. These offsets also apply to the player.");
    egui::CollapsingHeader::new("Adjust weapon grip").show(ui, |ui| {
        for (label, values, speed) in [("position (m)", &mut grip.position, 0.002), ("rotation (deg)", &mut grip.rotation, 1.0)] {
            ui.label(label);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut values.x).speed(speed).prefix("x "));
                ui.add(egui::DragValue::new(&mut values.y).speed(speed).prefix("y "));
                ui.add(egui::DragValue::new(&mut values.z).speed(speed).prefix("z "));
            });
        }
        ui.add(egui::Slider::new(&mut grip.scale, 0.25..=2.0).text("weapon size"));
        if ui.button("Save grip").clicked() {
            let value = serde_json::json!({"position": grip.position.to_array(), "rotation": grip.rotation.to_array(), "scale": grip.scale});
            studio.status = match std::fs::write(format!("{}/assets/weapon_grip.json", crate::ROOT), value.to_string()) { Ok(()) => "Weapon grip saved".into(), Err(e) => format!("Could not save grip: {e}") };
        }
    });
}
