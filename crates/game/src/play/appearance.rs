//! Character appearance: a look (`assets/data/appearance.json`, `Wardrobe`) is a base body
//! (male | female) plus slot choices: hair, brows, beard, eye colour, skin tone, hair colour,
//! body-shape morph weights and outfit pieces with their colours. `appearance` owns the dressing
//! of a character root that carries `Appearance`: the parts, their materials, the body groups
//! hidden under clothes and the morph weights.
//!
//! Assets (built by `tools/model/build.py` on ual2's skeleton; see `docs/MODEL_PARTS.md`): the
//! body is `characters/hero/<base>/body.glb` (skeleton + body + eyes), asked for through the
//! root's `ModelRequest`, so `motion::rig` binds it exactly like ual2. Every part is its own
//! `characters/hero/<base>/<part>.glb`, skinned to the same 65 joints with the same inverse bind
//! matrices. Once the root has a `Rig`, each part scene is spawned under the model; when it is
//! ready its skinned meshes are re-pointed at the rig's bone entities (by joint name), moved
//! under the model, its own skeleton copy is despawned, and mount nodes (`sheath_hip_l`,
//! `sheath_hip_r` on the belt, `sheath_back` on the baldric; +Y toward the hilt, +Z away from the
//! body) are re-parented onto the matching rig bones. So swapping a part needs no re-export.
//!
//! Body nodes named `body_hide_<garment>+<garment>...` are hidden while one of those garments is
//! worn (body polygons under clothes never poke through). Materials are named `hero_*`
//! (`hero_skin`, `hero_eyes`, `hero_hair`, `hero_brows`, `hero_beard`, `hero_cloth_<garment>`,
//! `hero_metal_<garment>`): each dressed character gets its own copies recoloured from its look,
//! and `motion::rig`'s `tint` leaves `hero_*` materials alone.
//!
//! The player: `GB_HERO` = `look` (default: the wardrobe's `player` look, or `GB_LOOK=<name>`),
//! `styled` (`anim/hero_styled.glb`, the default look baked into one file), or `mannequin` (the
//! old yellow ual2 model). NPCs keep the mannequin; `dress` puts any look on any root.
use super::HALF_HEIGHT;
use crate::motion::bank::PoseBank;
use crate::motion::{ModelRequest, MotionSet, Rig};
use bevy::gltf::GltfMaterialName;
use bevy::mesh::morph::MorphWeights;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use serde::Deserialize;
use std::collections::HashMap;

/// The baked single-file hero (`GB_HERO=styled`).
pub const STYLED_GLB: &str = "anim/hero_styled.glb";

pub fn plugin(app: &mut App) {
    app.add_systems(Update, dress_rigged)
        .add_systems(PostUpdate, attach_parts.before(MotionSet::Compose));
}

pub fn wardrobe_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/appearance.json")
}

/// A colour in the JSON: sRGB triple or a palette name.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Paint {
    Rgb([f32; 3]),
    Named(String),
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Shape {
    #[serde(default)]
    pub bulk: f32,
    #[serde(default)]
    pub heavy: f32,
    #[serde(default)]
    pub slim: f32,
    #[serde(default)]
    pub jaw: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Look {
    pub base: String,
    pub skin: Paint,
    #[serde(default = "brown")]
    pub eyes: String,
    #[serde(default)]
    pub hair: String,
    pub hair_color: Paint,
    #[serde(default)]
    pub brows: String,
    #[serde(default)]
    pub beard: String,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub outfit: Vec<String>,
    #[serde(default)]
    pub colors: HashMap<String, Paint>,
}

fn brown() -> String {
    "brown".into()
}

#[derive(Deserialize, Clone, Debug)]
pub struct Wardrobe {
    pub player: String,
    #[serde(default)]
    pub palette: HashMap<String, [f32; 3]>,
    pub looks: HashMap<String, Look>,
}

impl Wardrobe {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn colour(&self, paint: &Paint) -> Result<Color, String> {
        match paint {
            Paint::Rgb([r, g, b]) => Ok(Color::srgb(*r, *g, *b)),
            Paint::Named(name) => self
                .palette
                .get(name)
                .map(|[r, g, b]| Color::srgb(*r, *g, *b))
                .ok_or_else(|| format!("unknown palette colour {name:?}")),
        }
    }

    /// Resolve a look by name into what `appearance` applies.
    pub fn appearance(&self, name: &str) -> Result<Appearance, String> {
        let look = self.looks.get(name).ok_or_else(|| format!("no look {name:?}"))?;
        if look.base != "male" && look.base != "female" {
            return Err(format!("look {name:?}: base must be male or female, not {:?}", look.base));
        }
        let mut colors = HashMap::new();
        for (garment, paint) in &look.colors {
            colors.insert(garment.clone(), self.colour(paint)?);
        }
        let parts: Vec<String> = [&look.hair, &look.brows, &look.beard]
            .into_iter()
            .chain(look.outfit.iter())
            .filter(|p| !p.is_empty())
            .cloned()
            .collect();
        Ok(Appearance {
            look: name.to_string(),
            base: look.base.clone(),
            skin: self.colour(&look.skin)?,
            hair_color: self.colour(&look.hair_color)?,
            eyes: look.eyes.clone(),
            parts,
            outfit: look.outfit.clone(),
            colors,
            shape: [look.shape.bulk, look.shape.heavy, look.shape.slim, look.shape.jaw],
        })
    }
}

/// On a character root: the look it wears (resolved). Insert it together with the
/// `ModelRequest` from `model_request` (see `dress`).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Appearance {
    pub look: String,
    pub base: String,
    pub skin: Color,
    pub hair_color: Color,
    pub eyes: String,
    /// Part file stems under `characters/hero/<base>/` (hair, brows, beard, outfit).
    pub parts: Vec<String>,
    /// Garments worn (hide the body groups under them).
    pub outfit: Vec<String>,
    /// Garment name → colour.
    pub colors: HashMap<String, Color>,
    /// Morph weights in the hero GLBs' target order (`tools/model/build.py` `MORPHS`):
    /// bulk, heavy, slim, jaw.
    pub shape: [f32; 4],
}

impl Appearance {
    pub fn body_path(&self) -> String {
        format!("characters/hero/{}/body.glb", self.base)
    }
    pub fn part_path(&self, part: &str) -> String {
        format!("characters/hero/{}/{part}.glb", self.base)
    }
    pub fn eye_texture(&self) -> String {
        format!("characters/hero/tex/eye_{}.png", self.eyes)
    }
    /// Whether a body node named `body_hide_a+b` is hidden by this outfit.
    pub fn hides(&self, node: &str) -> bool {
        node.strip_prefix("body_hide_")
            .is_some_and(|list| list.split('+').any(|g| self.outfit.iter().any(|o| o == g)))
    }
    /// Colour for a `hero_*` material, if this look sets one.
    fn material_colour(&self, material: &str) -> Option<Color> {
        let name = material.strip_prefix("hero_")?;
        match name {
            "skin" => Some(self.skin),
            "hair" | "brows" | "beard" => Some(self.hair_color),
            _ => name
                .strip_prefix("cloth_")
                .and_then(|g| self.colors.get(g).copied()),
        }
    }
}

/// Put a look on a character root: the body model through `ModelRequest`, the rest once rigged.
pub fn dress(commands: &mut Commands, root: Entity, appearance: Appearance) {
    commands.entity(root).insert((
        ModelRequest {
            path: appearance.body_path(),
            offset: Vec3::new(0.0, -HALF_HEIGHT, 0.0),
            tint: None,
        },
        appearance,
    ));
}

/// The player's model per `GB_HERO` (`look` by default, `styled`, `mannequin`) and `GB_LOOK`.
pub fn dress_player(commands: &mut Commands, player: Entity) {
    let mode = std::env::var("GB_HERO").unwrap_or_default();
    match mode.trim() {
        "mannequin" | "ual2" => info!("[appearance] GB_HERO={mode}: the yellow mannequin"),
        "styled" => {
            commands.entity(player).insert(ModelRequest {
                path: STYLED_GLB.to_string(),
                offset: Vec3::new(0.0, -HALF_HEIGHT, 0.0),
                tint: None,
            });
            info!("[appearance] GB_HERO=styled: {STYLED_GLB}");
        }
        _ => match Wardrobe::load(&wardrobe_path()).and_then(|w| {
            let name = std::env::var("GB_LOOK").ok().filter(|s| !s.trim().is_empty()).unwrap_or(w.player.clone());
            w.appearance(name.trim())
        }) {
            Ok(look) => {
                info!("[appearance] player look {:?} ({}, {} parts)", look.look, look.base, look.parts.len());
                dress(commands, player, look);
            }
            Err(e) => warn!("[appearance] {e}; keeping the mannequin"),
        },
    }
}

/// On a root: the model entity its look was applied to (re-dress when the model changes).
#[derive(Component)]
pub struct Dressed(pub Entity);

/// A part scene waiting to be attached: (character root, part file stem).
#[derive(Component)]
struct PartOf(Entity);

#[derive(Component)]
struct PartReady;

fn on_part_ready(event: On<WorldInstanceReady>, mut commands: Commands) {
    commands.entity(event.entity).try_insert(PartReady);
}

/// Rigged roots with a look they don't wear yet: style the body, spawn the parts.
#[allow(clippy::type_complexity)]
fn dress_rigged(
    mut commands: Commands,
    assets: Res<AssetServer>,
    roots: Query<(Entity, &Rig, &Appearance, Option<&Dressed>, &ModelRequest), Or<(Changed<Rig>, Changed<Appearance>, Without<Dressed>)>>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut styling: Styling,
) {
    for (root, rig, look, dressed, request) in &roots {
        if dressed.is_some_and(|d| d.0 == rig.model) {
            continue;
        }
        if request.path != look.body_path() {
            // The look changed base: ask for the new body; dress once it is rigged.
            commands.entity(root).insert(ModelRequest {
                path: look.body_path(),
                offset: request.offset,
                tint: None,
            });
            continue;
        }
        let nodes: Vec<Entity> = children.iter_descendants(rig.model).collect();
        for &node in &nodes {
            if let Ok(name) = names.get(node)
                && name.as_str().starts_with("body_hide_")
            {
                let vis = if look.hides(name.as_str()) { Visibility::Hidden } else { Visibility::Inherited };
                commands.entity(node).insert(vis);
            }
        }
        styling.apply(&mut commands, &assets, &nodes, look);
        for part in &look.parts {
            let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(look.part_path(part)));
            let entity = commands
                .spawn((Name::new(format!("part {part}")), PartOf(root), WorldAssetRoot(scene), Transform::default()))
                .observe(on_part_ready)
                .id();
            commands.entity(rig.model).add_child(entity);
        }
        commands.entity(root).insert(Dressed(rig.model));
        info!("[appearance] {root}: dressing as {:?} ({} parts)", look.look, look.parts.len());
    }
}

/// Ready part scenes: point their skins at the rig's bones, move meshes and mounts onto the rig,
/// drop the part's own skeleton.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn attach_parts(
    mut commands: Commands,
    assets: Res<AssetServer>,
    bank: Option<Res<PoseBank>>,
    parts: Query<(Entity, &PartOf, &ChildOf), With<PartReady>>,
    roots: Query<(&Rig, &Appearance)>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut skins: Query<&mut SkinnedMesh>,
    mut styling: Styling,
) {
    let Some(bank) = bank else { return };
    for (part, &PartOf(root), child_of) in &parts {
        commands.entity(part).remove::<PartReady>();
        let Ok((rig, look)) = roots.get(root) else {
            commands.entity(part).try_despawn();
            continue;
        };
        if child_of.parent() != rig.model {
            commands.entity(part).try_despawn();
            continue;
        }
        let bone = |name: &str| bank.skeleton.find(name).map(|i| rig.bones[i]).filter(|e| *e != Entity::PLACEHOLDER);
        let nodes: Vec<Entity> = children.iter_descendants(part).collect();
        let mut moved = Vec::new();
        for &node in &nodes {
            if let Ok(mut skin) = skins.get_mut(node) {
                let mut ok = true;
                for joint in skin.joints.iter_mut() {
                    match names.get(*joint).ok().and_then(|n| bone(n.as_str())) {
                        Some(b) => *joint = b,
                        None => ok = false,
                    }
                }
                if !ok {
                    warn!("[appearance] {}: a skin joint has no rig bone", look.look);
                }
                // The mesh node (glTF node) is the primitive's parent; move it under the model.
                if let Ok(p) = parents.get(node)
                    && !moved.contains(&p.parent())
                    && p.parent() != part
                {
                    moved.push(p.parent());
                }
            }
            if let Ok(name) = names.get(node)
                && name.as_str().starts_with("sheath_")
                && let Ok(p) = parents.get(node)
                && let Ok(joint) = names.get(p.parent())
                && let Some(b) = bone(joint.as_str())
            {
                commands.entity(b).add_child(node);
            }
        }
        for &node in &moved {
            commands.entity(rig.model).add_child(node);
        }
        styling.apply(&mut commands, &assets, &nodes, look);
        // The part's skeleton copy and scene root go (despawn is recursive); its meshes and
        // mounts were re-parented out above, and those commands run first.
        commands.entity(part).try_despawn();
    }
}

/// Material and morph styling for a set of nodes of one character.
#[derive(bevy::ecs::system::SystemParam)]
struct Styling<'w, 's> {
    material_names: Query<'w, 's, (&'static GltfMaterialName, &'static MeshMaterial3d<StandardMaterial>)>,
    morphs: Query<'w, 's, &'static mut MorphWeights>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

impl Styling<'_, '_> {
    fn apply(&mut self, commands: &mut Commands, assets: &AssetServer, nodes: &[Entity], look: &Appearance) {
        let mut copies: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>> = HashMap::new();
        for &node in nodes {
            if let Ok((name, mat)) = self.material_names.get(node) {
                let colour = look.material_colour(&name.0);
                let eyes = name.0 == "hero_eyes";
                if colour.is_none() && !eyes {
                    continue;
                }
                let handle = match copies.get(&mat.0.id()) {
                    Some(h) => h.clone(),
                    None => {
                        let Some(mut m) = self.materials.get(&mat.0).cloned() else { continue };
                        if let Some(c) = colour {
                            m.base_color = c;
                        }
                        if eyes {
                            m.base_color_texture = Some(assets.load(look.eye_texture()));
                        }
                        let h = self.materials.add(m);
                        copies.insert(mat.0.id(), h.clone());
                        h
                    }
                };
                commands.entity(node).insert(MeshMaterial3d(handle));
            }
            if let Ok(mut w) = self.morphs.get_mut(node) {
                for (slot, value) in w.weights_mut().iter_mut().zip(look.shape) {
                    *slot = value.clamp(0.0, 1.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wardrobe() -> Wardrobe {
        Wardrobe::load(&wardrobe_path()).expect("appearance.json parses")
    }

    #[test]
    fn every_look_resolves_and_its_parts_exist() {
        let w = wardrobe();
        assert!(w.looks.contains_key(&w.player), "the player look exists");
        let root = std::path::Path::new(crate::ASSETS);
        for name in w.looks.keys() {
            let a = w.appearance(name).unwrap_or_else(|e| panic!("{e}"));
            assert!(root.join(a.body_path()).exists(), "{}", a.body_path());
            assert!(root.join(a.eye_texture()).exists(), "{}", a.eye_texture());
            for p in &a.parts {
                assert!(root.join(a.part_path(p)).exists(), "{name}: {}", a.part_path(p));
            }
        }
        assert!(root.join(STYLED_GLB).exists());
    }

    #[test]
    fn hide_groups_follow_the_outfit() {
        let a = wardrobe().appearance("wanderer").unwrap();
        assert!(a.hides("body_hide_tunic"));
        assert!(a.hides("body_hide_shirt+tunic"));
        assert!(!a.hides("body_hide_shirt"));
        assert!(!a.hides("body"));
        assert_eq!(a.material_colour("hero_skin"), Some(a.skin));
        assert_eq!(a.material_colour("hero_brows"), Some(a.hair_color));
        assert_eq!(a.material_colour("hero_cloth_tunic"), a.colors.get("tunic").copied());
        assert_eq!(a.material_colour("M_Main"), None);
    }

    /// The hero GLBs bind to the canonical skeleton: every joint of every part's skin is a
    /// skeleton bone, in the same order (so the rig finds all 65 and parts re-point by name).
    #[test]
    fn hero_glbs_use_the_canonical_joints() {
        let canon = crate::motion::donor::load("ual2", &std::path::Path::new(crate::ASSETS).join(crate::motion::MODEL_GLB)).unwrap();
        let canon_names: Vec<String> = canon.joints.iter().map(|&n| canon.nodes[n].name.clone()).collect();
        let dir = std::path::Path::new(crate::ASSETS).join("characters/hero");
        let mut checked = 0;
        for base in ["male", "female"] {
            for entry in std::fs::read_dir(dir.join(base)).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|e| e == "glb") {
                    let lib = crate::motion::donor::load("hero", &path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                    let names: Vec<String> = lib.joints.iter().map(|&n| lib.nodes[n].name.clone()).collect();
                    assert_eq!(names, canon_names, "{}", path.display());
                    for (&a, &b) in lib.joints.iter().zip(&canon.joints) {
                        let (ra, rb) = (lib.nodes[a].rest, canon.nodes[b].rest);
                        assert!(ra.translation.distance(rb.translation) < 1e-6 && ra.rotation.angle_between(rb.rotation) < 1e-5,
                            "{}: rest of {} differs", path.display(), lib.nodes[a].name);
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked >= 20, "checked {checked} files");
    }
}
