//! The guildhouse: the two-storey guild headquarters on the east terrace, built at startup from
//! `assets/data/guildhouse.json` (sizes read off the Higgsfield views G2/G3/G4/I1/I9, see
//! `assets/art/guildhouse/` and `GUILDHOUSE.md` on the `agent-log` branch) with the Higgsfield
//! textures in `assets/textures/guildhouse/`.
//!
//! It stands where the old hall stood (`hall::HALL_MIN..HALL_MAX` is its main block) and faces
//! west, toward the courtyard: building "front" is −x, "right" (seen from the front) is +z.
//! - a stone **podium** (`hall::FLOOR_Y` high) with six steps up to the west door and six more
//!   down from the yard door on the +z side to the training ground;
//! - a stone **ground floor** (west door, yard door, windows front and back), lined inside with
//!   oak wainscot and plaster; a timber-framed plaster **upper floor** that overhangs it, reached
//!   by a straight stair along the +z wall; rooms upstairs: records (front-left), lounge with the
//!   stair landing (front-right) and the guildmaster's office (back strip);
//! - a slate **roof** with its ridge along z, plaster gables, a **bell tower** in the back-right
//!   corner (pyramid roof, open belfry), a stone **chimney** on the −z wall, a **porch** over the
//!   west door with guild banners and iron lanterns either side.
//!
//! Only the shell, the upstairs furniture and the decoration live here. The doors, front desk,
//! bounty board, beds and the lodging table (everything people use) stay in `hall`, which also
//! owns the walking routes. Six point lights in all, none casting shadows (two here outside,
//! one at the hearth, one upstairs; two in `hall`).
use super::hall::{DOOR_Z, FLOOR_Y, HALL_MAX, HALL_MIN, WALL_T, YARD_DOOR_X};
use super::surfaces::{PROP_TILE, world_uvs};
use super::world::world_layers;
use avian3d::prelude::*;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::prelude::*;
use serde::Deserialize;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

/// Upper walls: thickness (m). Window sills and heads above their floor (m).
const UPPER_T: f32 = 0.3;
const SILL: f32 = 1.0;
const HEAD: f32 = 2.5;
const UPPER_SILL: f32 = 0.9;
const UPPER_HEAD: f32 = 2.1;
/// The yard door's width and height (m), and the steps down to the yard: width (m).
const YARD_DOOR_W: f32 = 2.6;
const YARD_STEPS_W: f32 = 3.0;
/// Upper floor slab thickness (m) and the stair: steps, width (m), x range (bottom → top).
const SLAB: f32 = 0.25;
const STAIR_STEPS: usize = 15;
const STAIR_W: f32 = 1.1;
const STAIR_X: (f32, f32) = (28.9, 32.3);
/// Upstairs: the wall between the front rooms and the back strip (x), and the one between
/// records and lounge (z), partition thickness (m).
const BACK_STRIP_X: f32 = 32.4;
const ROOMS_Z: f32 = DOOR_Z;
const PARTITION_T: f32 = 0.12;
/// The hearth's centre along the −z wall (x).
const HEARTH_X: f32 = 30.7;

/// The part of `guildhouse.json` the build reads (the rest is provenance and notes).
#[derive(Deserialize, Debug, Clone)]
pub struct Spec {
    pub podium: Podium,
    pub steps: Steps,
    pub main_block: Block,
    pub storeys: Vec<Storey>,
    pub roof: Roof,
    pub tower: Tower,
    pub chimney: Chimney,
    pub door: DoorSpec,
    pub porch: Porch,
    pub banners: Banners,
    pub lanterns: Lanterns,
    pub windows_ground: Windows,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Podium {
    /// Width (along the front) and depth (m).
    pub size_xy: [f32; 2],
    pub height: f32,
    pub aprons_m: Aprons,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Aprons {
    pub front: f32,
    pub back: f32,
    pub sides: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Steps {
    pub count: usize,
    pub rise: f32,
    pub width: f32,
    /// How far the flight reaches out from the podium edge (m).
    pub projection: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Block {
    pub width: f32,
    pub depth: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Storey {
    pub height: f32,
    #[serde(default)]
    pub overhang: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Roof {
    /// Eaves to ridge (m).
    pub height: f32,
    /// Eave overhang past the walls: sides, gable ends (m).
    pub eave_overhang: [f32; 2],
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tower {
    pub size_xy: [f32; 2],
    pub wall_top_above_podium: f32,
    pub point_top_above_podium: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Chimney {
    pub width: f32,
    pub top_above_podium: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DoorSpec {
    pub width: f32,
    pub height: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Porch {
    pub width: f32,
    pub depth: f32,
    pub top_above_podium: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Banners {
    /// Offsets along the front from the door centre (m; + is right seen from the front, +z).
    pub x: Vec<f32>,
    pub size: [f32; 2],
    pub top_above_podium: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Lanterns {
    pub x: Vec<f32>,
    pub height_above_podium: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Windows {
    pub front_x: Vec<f32>,
    pub width: f32,
    pub back_x: Vec<f32>,
}

pub fn spec_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/guildhouse.json")
}

impl Spec {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let spec: Self = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        spec.check()?;
        Ok(spec)
    }

    /// The sizes must add up, and the block must be the hall footprint the rest of the game
    /// (routes, desk, doors) is laid out on.
    pub fn check(&self) -> Result<(), String> {
        let near = |a: f32, b: f32| (a - b).abs() < 0.02;
        let (p, a, b) = (&self.podium, &self.podium.aprons_m, &self.main_block);
        if self.storeys.len() < 2 {
            return Err("guildhouse.json: two storeys expected".into());
        }
        let checks = [
            (near(p.size_xy[0], b.width + 2.0 * a.sides), "podium width = block width + 2 side aprons"),
            (near(p.size_xy[1], b.depth + a.front + a.back), "podium depth = block depth + front and back aprons"),
            (near(self.steps.count as f32 * self.steps.rise, p.height), "steps climb the podium"),
            (near(p.height, FLOOR_Y), "podium height = hall::FLOOR_Y"),
            (near(b.depth, HALL_MAX.x - HALL_MIN.x), "block depth = hall footprint (x)"),
            (near(b.width, HALL_MAX.y - HALL_MIN.y), "block width = hall footprint (z)"),
            (near((HALL_MIN.y + HALL_MAX.y) * 0.5, DOOR_Z), "door centred on the front"),
            (self.door.height < self.storeys[0].height, "door fits the ground floor"),
        ];
        match checks.iter().find(|(ok, _)| !ok) {
            Some((_, what)) => Err(format!("guildhouse.json: {what}")),
            None => Ok(()),
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

/// The materials, one per slot in `GUILDHOUSE.md` §4, tiled at "one tile covers" metres.
struct Mats {
    podium: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    plaster: Handle<StandardMaterial>,
    timber: Handle<StandardMaterial>,
    slate: Handle<StandardMaterial>,
    planks: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    inner: Handle<StandardMaterial>,
    wainscot: Handle<StandardMaterial>,
    ceiling: Handle<StandardMaterial>,
    banner: Handle<StandardMaterial>,
    rug: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    fire: Handle<StandardMaterial>,
    soot: Handle<StandardMaterial>,
    bronze: Handle<StandardMaterial>,
    paper: Handle<StandardMaterial>,
}

impl Mats {
    fn new(assets: &AssetServer, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut tiled = |slot: &str, covers: f32, roughness: f32| {
            materials.add(tiled_material(assets, slot, covers, StandardMaterial { perceptual_roughness: roughness, ..default() }))
        };
        let podium = tiled("podium_stone", 3.0, 0.95);
        let wall = tiled("wall_stone", 6.0, 0.95);
        let plaster = tiled("plaster", 2.0, 0.9);
        let timber = tiled("timber", 1.5, 0.8);
        let slate = tiled("roof_slate", 2.0, 0.85);
        let planks = tiled("floor_planks", 2.0, 0.85);
        let iron = tiled("iron_trim", 1.0, 0.5);
        let inner = tiled("interior_plaster", 2.0, 0.9);
        let wainscot = tiled("wainscot", 2.0, 0.75);
        let ceiling = tiled("ceiling_boards", 2.0, 0.85);
        let picture = |path: &'static str| StandardMaterial {
            base_color_texture: Some(assets.load(path)),
            perceptual_roughness: 0.95,
            double_sided: true,
            cull_mode: None,
            ..default()
        };
        let flat = |c: Color, rough: f32| StandardMaterial { base_color: c, perceptual_roughness: rough, ..default() };
        Self {
            podium,
            wall,
            plaster,
            timber,
            slate,
            planks,
            iron,
            inner,
            wainscot,
            ceiling,
            banner: materials.add(picture("textures/guildhouse/banner_cloth_game.png")),
            rug: materials.add(picture("textures/guildhouse/rug_game.png")),
            glass: materials.add(StandardMaterial {
                base_color: Color::srgb(0.20, 0.27, 0.36),
                perceptual_roughness: 0.15,
                metallic: 0.3,
                ..default()
            }),
            glow: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.8, 0.45),
                emissive: LinearRgba::rgb(6.0, 3.6, 1.4),
                ..default()
            }),
            fire: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.55, 0.2),
                emissive: LinearRgba::rgb(14.0, 5.0, 1.2),
                ..default()
            }),
            soot: materials.add(flat(Color::srgb(0.08, 0.07, 0.07), 1.0)),
            bronze: materials.add(StandardMaterial {
                base_color: Color::srgb(0.70, 0.52, 0.26),
                metallic: 0.9,
                perceptual_roughness: 0.35,
                ..default()
            }),
            paper: materials.add(flat(Color::srgb(0.86, 0.80, 0.66), 0.95)),
        }
    }
}

/// A repeating albedo (sRGB) + normal (linear) material for slot `slot`, scaled so one texture
/// tile spans `covers` metres under `world_uvs` (which gives one UV per `PROP_TILE` m).
fn tiled_material(assets: &AssetServer, slot: &str, covers: f32, base: StandardMaterial) -> StandardMaterial {
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    let colour_sampler = sampler.clone();
    let albedo = assets
        .load_builder()
        .with_settings(move |s: &mut ImageLoaderSettings| s.sampler = colour_sampler.clone())
        .load(format!("textures/guildhouse/{slot}_albedo.png"));
    let normal = assets
        .load_builder()
        .with_settings(move |s: &mut ImageLoaderSettings| {
            s.sampler = sampler.clone();
            s.is_srgb = false;
        })
        .load(format!("textures/guildhouse/{slot}_normal.png"));
    StandardMaterial {
        base_color_texture: Some(albedo),
        normal_map_texture: Some(normal),
        uv_transform: Affine2::from_scale(Vec2::splat(PROP_TILE / covers)),
        ..base
    }
}

/// An opening in a wall: centre along the wall from its start, width, bottom and top (world y).
#[derive(Clone, Copy, Debug)]
struct Hole {
    at: f32,
    w: f32,
    bottom: f32,
    top: f32,
}

/// Spawning helpers for the guildhouse pieces.
struct Build<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    m: Mats,
}

impl Build<'_, '_, '_> {
    /// A static mesh piece with world-scale UVs.
    fn mesh(&mut self, name: &str, mesh: impl Into<Mesh>, mat: &Handle<StandardMaterial>, tf: Transform) -> Entity {
        let mut mesh = mesh.into();
        world_uvs(&mut mesh, tf.scale, PROP_TILE);
        let mesh = self.meshes.add(mesh);
        self.commands.spawn((Name::new(name.to_string()), Mesh3d(mesh), MeshMaterial3d(mat.clone()), tf)).id()
    }

    /// A box of `size` at `tf`, solid when `collide`.
    fn block(&mut self, name: &str, size: Vec3, mat: &Handle<StandardMaterial>, tf: Transform, collide: bool) -> Entity {
        let e = self.mesh(name, Cuboid::from_size(size), mat, tf);
        if collide {
            self.commands.entity(e).insert((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), world_layers()));
        }
        e
    }

    /// An axis-aligned box between two corners.
    fn span(&mut self, name: &str, a: Vec3, b: Vec3, mat: &Handle<StandardMaterial>, collide: bool) -> Entity {
        let (lo, hi) = (a.min(b), a.max(b));
        self.block(name, hi - lo, mat, Transform::from_translation((lo + hi) * 0.5), collide)
    }

    /// A picture quad (its own 0..1 UVs, no tiling) of `size` facing `normal`.
    fn picture(&mut self, name: &str, size: Vec2, mat: &Handle<StandardMaterial>, at: Vec3, normal: Vec3) -> Entity {
        let mesh = self.meshes.add(Rectangle::new(size.x, size.y));
        let rot = Quat::from_rotation_arc(Vec3::Z, normal.normalize());
        self.commands
            .spawn((Name::new(name.to_string()), Mesh3d(mesh), MeshMaterial3d(mat.clone()), Transform::from_translation(at).with_rotation(rot), NotShadowCaster))
            .id()
    }

    /// A wall from `a` to `b` (x, z), heights `y0..y1`, `thick` m, with rectangular `holes`;
    /// each solid piece (see `split`) is its own box.
    #[allow(clippy::too_many_arguments)]
    fn wall(&mut self, name: &str, a: Vec2, b: Vec2, y0: f32, y1: f32, thick: f32, holes: &[Hole], mat: &Handle<StandardMaterial>, collide: bool) {
        let d = b - a;
        let len = d.length();
        if len < 0.05 {
            return;
        }
        let dir = d / len;
        let rot = Quat::from_rotation_y(-dir.y.atan2(dir.x));
        for (t0, t1, h0, h1) in split(len, y0, y1, holes) {
            let mid = a + dir * ((t0 + t1) * 0.5);
            let size = Vec3::new(t1 - t0, h1 - h0, thick);
            self.block(name, size, mat, Transform::from_xyz(mid.x, (h0 + h1) * 0.5, mid.y).with_rotation(rot), collide);
        }
    }

    /// A slab between `lo` and `hi` (x, z) at heights `y0..y1` with a rectangular hole.
    #[allow(clippy::too_many_arguments)]
    fn slab_with_hole(&mut self, name: &str, lo: Vec2, hi: Vec2, y0: f32, y1: f32, hole: (Vec2, Vec2), mat: &Handle<StandardMaterial>, collide: bool) {
        let (hl, hh) = (hole.0.max(lo), hole.1.min(hi));
        let v = |x: f32, z: f32, y: f32| Vec3::new(x, y, z);
        // Strips: −x of the hole, +x of the hole (full z), then −z and +z between them.
        for (a, b) in [
            (v(lo.x, lo.y, y0), v(hl.x, hi.y, y1)),
            (v(hh.x, lo.y, y0), v(hi.x, hi.y, y1)),
            (v(hl.x, lo.y, y0), v(hh.x, hl.y, y1)),
            (v(hl.x, hh.y, y0), v(hh.x, hi.y, y1)),
        ] {
            if (b.x - a.x) > 0.02 && (b.z - a.z) > 0.02 {
                self.span(name, a, b, mat, collide);
            }
        }
    }

    /// A glazed window in the plane of a wall: glass, frame, mullion and sill. `centre` is the
    /// opening's centre on the wall line, `along` the wall direction, `out` its outward normal.
    #[allow(clippy::too_many_arguments)]
    fn window(&mut self, centre: Vec3, along: Vec3, out: Vec3, w: f32, h: f32, thick: f32) {
        let rot = Quat::from_mat3(&Mat3::from_cols(along, Vec3::Y, along.cross(Vec3::Y)));
        let at = |s: Vec3| centre + rot * s;
        let (glass, timber) = (self.m.glass.clone(), self.m.timber.clone());
        self.block("Window glass", Vec3::new(w, h, 0.03), &glass, Transform::from_translation(centre).with_rotation(rot), false);
        let f = 0.1;
        for (pos, size) in [
            (Vec3::new(0.0, h * 0.5 - f * 0.5, 0.0), Vec3::new(w, f, thick * 0.6)),
            (Vec3::new(0.0, -h * 0.5 + f * 0.5, 0.0), Vec3::new(w, f, thick * 0.6)),
            (Vec3::new(w * 0.5 - f * 0.5, 0.0, 0.0), Vec3::new(f, h, thick * 0.6)),
            (Vec3::new(-w * 0.5 + f * 0.5, 0.0, 0.0), Vec3::new(f, h, thick * 0.6)),
            (Vec3::ZERO, Vec3::new(0.06, h, 0.08)),
            (Vec3::new(0.0, h * 0.1, 0.0), Vec3::new(w, 0.05, 0.08)),
        ] {
            self.block("Window frame", size, &timber, Transform::from_translation(at(pos)).with_rotation(rot), false);
        }
        let sill = centre + out * (thick * 0.5 + 0.06) - Vec3::Y * (h * 0.5 + 0.04);
        self.block("Window sill", Vec3::new(w + 0.2, 0.08, 0.2), &timber, Transform::from_translation(sill).with_rotation(rot), false);
    }

    /// A point light that casts no shadows.
    fn light(&mut self, name: &str, at: Vec3, color: Color, intensity: f32, range: f32) {
        self.commands.spawn((
            Name::new(name.to_string()),
            PointLight { color, intensity, range, shadow_maps_enabled: false, ..default() },
            Transform::from_translation(at),
        ));
    }
}

/// Derived heights and lines (world).
struct Frame {
    floor: f32,
    upper: f32,
    eaves: f32,
    ridge: f32,
    jetty: f32,
    /// Main block faces: front (−x), back (+x), left (−z), right (+z).
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
}

impl Frame {
    fn new(s: &Spec) -> Self {
        let floor = FLOOR_Y;
        let upper = floor + s.storeys[0].height;
        let eaves = upper + s.storeys[1].height;
        Self {
            floor,
            upper,
            eaves,
            ridge: eaves + s.roof.height,
            jetty: s.storeys[1].overhang,
            x0: HALL_MIN.x,
            x1: HALL_MAX.x,
            z0: HALL_MIN.y,
            z1: HALL_MAX.y,
        }
    }

    /// The tower's footprint: the back-right corner, flush with the upper floor's outer faces.
    fn tower(&self, s: &Spec) -> (Vec2, Vec2) {
        let hi = Vec2::new(self.x1 + self.jetty, self.z1 + self.jetty);
        (hi - Vec2::new(s.tower.size_xy[1], s.tower.size_xy[0]), hi)
    }
}

fn spawn(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let spec = match Spec::load(&spec_path()) {
        Ok(s) => s,
        Err(e) => {
            error!("[guildhouse] {e}; not built");
            return;
        }
    };
    let m = Mats::new(&assets, &mut materials);
    let mut b = Build { commands: &mut commands, meshes: &mut meshes, m };
    let f = Frame::new(&spec);
    podium(&mut b, &spec, &f);
    ground_floor(&mut b, &spec, &f);
    stairs_and_upper_floor(&mut b, &f);
    upper_storey(&mut b, &spec, &f);
    roof(&mut b, &spec, &f);
    tower(&mut b, &spec, &f);
    chimney_and_hearths(&mut b, &spec, &f);
    porch_banners_lanterns(&mut b, &spec, &f);
    furniture(&mut b, &f);
    info!(
        "[guildhouse] built: block {:.1}x{:.1} m, floor {:.2}, upper {:.2}, eaves {:.2}, ridge {:.2}",
        spec.main_block.depth, spec.main_block.width, f.floor, f.upper, f.eaves, f.ridge
    );
}

/// The podium, its paving, the west steps and the yard steps.
fn podium(b: &mut Build, s: &Spec, f: &Frame) {
    let (a, p) = (&s.podium.aprons_m, &s.podium);
    let lo = Vec2::new(f.x0 - a.front, f.z0 - a.sides);
    let hi = Vec2::new(f.x1 + a.back, f.z1 + a.sides);
    let stone = b.m.podium.clone();
    // Down to 0.4 m under the terrace so no gap shows at its edge.
    b.span("Podium", Vec3::new(lo.x, -0.4, lo.y), Vec3::new(hi.x, p.height, hi.y), &stone, true);
    // A coping band round the top edge.
    let cap = 0.12;
    for (ca, cb) in [
        (Vec3::new(lo.x - cap, p.height - 0.1, lo.y - cap), Vec3::new(hi.x + cap, p.height + 0.04, lo.y + 0.3)),
        (Vec3::new(lo.x - cap, p.height - 0.1, hi.y - 0.3), Vec3::new(hi.x + cap, p.height + 0.04, hi.y + cap)),
        (Vec3::new(hi.x - 0.3, p.height - 0.1, lo.y), Vec3::new(hi.x + cap, p.height + 0.04, hi.y)),
    ] {
        b.span("Podium coping", ca, cb, &stone, false);
    }
    // The west flight, centred on the door, and the yard flight down from the yard door.
    let run = s.steps.projection / s.steps.count as f32;
    for k in 0..s.steps.count {
        let top = p.height - s.steps.rise * k as f32;
        let x_out = lo.x - run * (k + 1) as f32;
        let hw = s.steps.width * 0.5;
        b.span("Front step", Vec3::new(x_out, -0.2, DOOR_Z - hw), Vec3::new(lo.x - run * k as f32, top, DOOR_Z + hw), &stone, true);
        let z_out = hi.y + run * (k + 1) as f32;
        let yw = YARD_STEPS_W * 0.5;
        b.span("Yard step", Vec3::new(YARD_DOOR_X - yw, -0.2, hi.y + run * k as f32), Vec3::new(YARD_DOOR_X + yw, top, z_out), &stone, true);
    }
    // Cheek walls either side of the west flight.
    for side in [-1.0f32, 1.0] {
        let z = DOOR_Z + side * (s.steps.width * 0.5 + 0.2);
        let (x_out, x_in) = (lo.x - s.steps.projection, lo.x);
        let len = x_in - x_out;
        let rise = p.height;
        let slope = (rise / len).atan();
        let mid = Vec3::new((x_out + x_in) * 0.5, rise * 0.5, z);
        let size = Vec3::new((len * len + rise * rise).sqrt(), 0.35, 0.4);
        b.block("Step cheek", size, &stone, Transform::from_translation(mid).with_rotation(Quat::from_rotation_z(slope)), false);
    }
    // Plank floor inside the block.
    let planks = b.m.planks.clone();
    let i = WALL_T * 0.5;
    b.span("Hall floor", Vec3::new(f.x0 + i, p.height, f.z0 + i), Vec3::new(f.x1 - i, p.height + 0.02, f.z1 - i), &planks, false);
}

/// Ground floor walls: stone outside, wainscot and plaster inside, with the doors and windows.
fn ground_floor(b: &mut Build, s: &Spec, f: &Frame) {
    let (y0, y1) = (f.floor, f.upper);
    let door = Hole { at: DOOR_Z - f.z0, w: s.door.width, bottom: y0, top: y0 + s.door.height };
    let win = |z: f32| Hole { at: z - f.z0, w: s.windows_ground.width, bottom: y0 + SILL, top: y0 + HEAD };
    let mut front: Vec<Hole> = s.windows_ground.front_x.iter().map(|x| win(DOOR_Z + x)).collect();
    front.push(door);
    let back: Vec<Hole> = s.windows_ground.back_x.iter().map(|x| win(DOOR_Z + x)).collect();
    let yard = Hole { at: YARD_DOOR_X - f.x0, w: YARD_DOOR_W, bottom: y0, top: y0 + s.door.height };
    let (x0, x1, z0, z1) = (f.x0, f.x1, f.z0, f.z1);
    let walls: [(&str, Vec2, Vec2, Vec<Hole>); 4] = [
        ("Front wall", Vec2::new(x0, z0), Vec2::new(x0, z1), front),
        ("Back wall", Vec2::new(x1, z0), Vec2::new(x1, z1), back),
        ("Left wall", Vec2::new(x0, z0), Vec2::new(x1, z0), vec![]),
        ("Right wall", Vec2::new(x0, z1), Vec2::new(x1, z1), vec![yard]),
    ];
    let (stone, wainscot, inner) = (b.m.wall.clone(), b.m.wainscot.clone(), b.m.inner.clone());
    for (name, a, c, holes) in walls.iter() {
        // Corners: run the end walls' length past the side walls so the corners close.
        let (a, c) = extend(*a, *c, WALL_T * 0.5);
        let shift = WALL_T * 0.5;
        let holes: Vec<Hole> = holes.iter().map(|h| Hole { at: h.at + shift, ..*h }).collect();
        b.wall(name, a, c, y0, y1, WALL_T, &holes, &stone, true);
        // Lining on the inner face: wainscot to 1.1 m, plaster above.
        let inward = inward_of(*name);
        let (la, lc) = (a + inward * (WALL_T * 0.5 + 0.015), c + inward * (WALL_T * 0.5 + 0.015));
        let (la, lc) = extend(la, lc, -WALL_T);
        let holes: Vec<Hole> = holes.iter().map(|h| Hole { at: h.at - WALL_T, ..*h }).collect();
        b.wall("Wainscot", la, lc, y0, y0 + 1.1, 0.03, &holes, &wainscot, false);
        b.wall("Inner plaster", la, lc, y0 + 1.1, y1, 0.03, &holes, &inner, false);
    }
    // Glass and frames in every window, a stone surround and arch over the west door.
    for x in &s.windows_ground.front_x {
        b.window(Vec3::new(x0, y0 + (SILL + HEAD) * 0.5, DOOR_Z + x), Vec3::Z, Vec3::NEG_X, s.windows_ground.width, HEAD - SILL, WALL_T);
    }
    for x in &s.windows_ground.back_x {
        b.window(Vec3::new(x1, y0 + (SILL + HEAD) * 0.5, DOOR_Z + x), Vec3::Z, Vec3::X, s.windows_ground.width, HEAD - SILL, WALL_T);
    }
    let surround = b.m.podium.clone();
    let (dw, dh) = (s.door.width, s.door.height);
    for side in [-1.0f32, 1.0] {
        b.block("Door jamb", Vec3::new(WALL_T + 0.16, dh, 0.3), &surround, Transform::from_xyz(x0, y0 + dh * 0.5, DOOR_Z + side * (dw * 0.5 + 0.15)), false);
    }
    // Arch: voussoirs round a half circle above the lintel line.
    let r = dw * 0.5 + 0.15;
    for k in 0..7 {
        let a = PI * (k as f32 + 0.5) / 7.0;
        let at = Vec3::new(x0 - 0.02, y0 + dh + a.sin() * r * 0.45, DOOR_Z + a.cos() * r);
        b.block("Door arch", Vec3::new(WALL_T + 0.18, 0.3, 0.42), &surround, Transform::from_translation(at).with_rotation(Quat::from_rotation_x(-(a - FRAC_PI_2))), false);
    }
    // The yard door's timber frame.
    let timber = b.m.timber.clone();
    for side in [-1.0f32, 1.0] {
        b.block("Yard jamb", Vec3::new(0.18, dh, WALL_T + 0.1), &timber, Transform::from_xyz(YARD_DOOR_X + side * (YARD_DOOR_W * 0.5 + 0.09), y0 + dh * 0.5, z1), false);
    }
    b.block("Yard lintel", Vec3::new(YARD_DOOR_W + 0.5, 0.22, WALL_T + 0.1), &timber, Transform::from_xyz(YARD_DOOR_X, y0 + dh + 0.11, z1), false);
}

/// Unit (x, z) pointing into the block from the named outer wall.
fn inward_of(name: &str) -> Vec2 {
    match name {
        "Front wall" => Vec2::X,
        "Back wall" => Vec2::NEG_X,
        "Left wall" => Vec2::Y,
        _ => Vec2::NEG_Y,
    }
}

/// `a`..`b` lengthened by `by` at both ends (shortened when negative).
fn extend(a: Vec2, b: Vec2, by: f32) -> (Vec2, Vec2) {
    let d = (b - a).normalize_or_zero();
    (a - d * by, b + d * by)
}

/// The solid pieces of a wall `len` long between heights `y0..y1` around `holes`: (start, end,
/// bottom, top) — full-height piers between the holes, and the parts under and over each hole.
/// Holes are clipped to the wall; slivers under 2 cm are dropped.
fn split(len: f32, y0: f32, y1: f32, holes: &[Hole]) -> Vec<(f32, f32, f32, f32)> {
    let mut out = Vec::new();
    if y1 <= y0 {
        return out;
    }
    let mut push = |t0: f32, t1: f32, h0: f32, h1: f32| {
        if t1 - t0 >= 0.02 && h1 - h0 >= 0.02 {
            out.push((t0, t1, h0, h1));
        }
    };
    let mut holes: Vec<Hole> = holes.iter().copied().filter(|h| h.at + h.w * 0.5 > 0.0 && h.at - h.w * 0.5 < len).collect();
    holes.sort_by(|p, q| p.at.total_cmp(&q.at));
    let mut t = 0.0f32;
    for h in holes {
        let (h0, h1) = ((h.at - h.w * 0.5).max(t), (h.at + h.w * 0.5).min(len));
        push(t, h0, y0, y1);
        push(h0, h1, y0, h.bottom.max(y0));
        push(h0, h1, h.top.min(y1), y1);
        t = t.max(h1);
    }
    push(t, len, y0, y1);
    out
}

/// The stair along the +z wall, the upper floor (with the stairwell) and the ground ceiling.
fn stairs_and_upper_floor(b: &mut Build, f: &Frame) {
    let (timber, planks, ceiling) = (b.m.timber.clone(), b.m.planks.clone(), b.m.ceiling.clone());
    let rise = (f.upper - f.floor) / STAIR_STEPS as f32;
    let run = (STAIR_X.1 - STAIR_X.0) / STAIR_STEPS as f32;
    let z_in = f.z1 - WALL_T * 0.5;
    let (sz0, sz1) = (z_in - STAIR_W, z_in);
    for k in 0..STAIR_STEPS {
        let x = STAIR_X.0 + run * k as f32;
        let top = f.floor + rise * (k + 1) as f32;
        b.span("Stair step", Vec3::new(x, f.floor, sz0), Vec3::new(x + run, top, sz1), &planks, true);
    }
    // A stringer and a handrail on the open side.
    let len = STAIR_X.1 - STAIR_X.0;
    let h = f.upper - f.floor;
    let slope = (h / len).atan();
    let mid = Vec3::new((STAIR_X.0 + STAIR_X.1) * 0.5, f.floor + h * 0.5, sz0 - 0.04);
    let diag = (len * len + h * h).sqrt();
    b.block("Stair stringer", Vec3::new(diag, 0.3, 0.08), &timber, Transform::from_translation(mid).with_rotation(Quat::from_rotation_z(slope)), false);
    b.block("Stair rail", Vec3::new(diag, 0.07, 0.07), &timber, Transform::from_translation(mid + Vec3::Y * 0.95).with_rotation(Quat::from_rotation_z(slope)), false);
    for k in [2usize, 6, 10, 14] {
        let x = STAIR_X.0 + run * (k as f32 + 0.5);
        let y = f.floor + rise * (k + 1) as f32;
        b.block("Baluster", Vec3::new(0.07, 0.95, 0.07), &timber, Transform::from_xyz(x, y + 0.47, sz0 - 0.04), false);
    }
    // Upper floor (planks on top, ceiling boards under), out over the jetty, with the stairwell.
    let j = f.jetty;
    let lo = Vec2::new(f.x0 - j, f.z0 - j);
    let hi = Vec2::new(f.x1 + j, f.z1 + j);
    let well = (Vec2::new(STAIR_X.0 - 0.1, sz0 - 0.1), Vec2::new(STAIR_X.1 + 0.05, f.z1));
    b.slab_with_hole("Upper floor", lo, hi, f.upper - 0.05, f.upper, well, &planks, true);
    b.slab_with_hole("Ground ceiling", lo, hi, f.upper - SLAB, f.upper - 0.05, well, &ceiling, false);
    // Heavy beams across the hall (along x), and joist ends showing under the jetty.
    let mut z = f.z0 + 1.2;
    while z < f.z1 - 0.6 {
        let clear = z < well.0.y - 0.15;
        let (bx0, bx1) = if clear { (f.x0, f.x1) } else { (f.x0, STAIR_X.0 - 0.15) };
        b.span("Ceiling beam", Vec3::new(bx0, f.upper - SLAB - 0.26, z - 0.13), Vec3::new(bx1, f.upper - SLAB, z + 0.13), &timber, false);
        z += 1.6;
    }
    // Railing round the stairwell upstairs (the −z and −x sides).
    let ry = f.upper;
    b.span("Well rail", Vec3::new(well.0.x, ry + 0.95, well.0.y - 0.03), Vec3::new(well.1.x, ry + 1.02, well.0.y + 0.03), &timber, false);
    b.span("Well rail", Vec3::new(well.0.x - 0.03, ry + 0.95, well.0.y), Vec3::new(well.0.x + 0.03, ry + 1.02, well.1.y), &timber, false);
    for x in [well.0.x, (well.0.x + well.1.x) * 0.5, well.1.x - 1.0] {
        b.span("Well post", Vec3::new(x - 0.04, ry, well.0.y - 0.04), Vec3::new(x + 0.04, ry + 1.0, well.0.y + 0.04), &timber, true);
    }
    // Invisible guards along the rails (the −z side short of the stair head, and the −x end)
    // stop a fall into the well.
    for (at, size) in [
        (Vec3::new((well.0.x + well.1.x) * 0.5 - 0.5, ry + 0.5, well.0.y), Vec3::new(well.1.x - well.0.x - 1.0, 1.0, 0.06)),
        (Vec3::new(well.0.x, ry + 0.5, (well.0.y + well.1.y) * 0.5), Vec3::new(0.06, 1.0, well.1.y - well.0.y)),
    ] {
        b.commands.spawn((Name::new("Well guard"), Transform::from_translation(at), RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), world_layers()));
    }
}

/// Upper storey: timber-framed plaster walls over the jetty, windows, partitions, ceiling.
fn upper_storey(b: &mut Build, s: &Spec, f: &Frame) {
    let (y0, y1) = (f.upper, f.eaves);
    let j = f.jetty;
    let (x0, x1, z0, z1) = (f.x0 - j + UPPER_T * 0.5, f.x1 + j - UPPER_T * 0.5, f.z0 - j + UPPER_T * 0.5, f.z1 + j - UPPER_T * 0.5);
    let ww = 1.2;
    let win = |at: f32| Hole { at, w: ww, bottom: y0 + UPPER_SILL, top: y0 + UPPER_HEAD };
    let (tlo, _) = f.tower(s);
    let back: Vec<Hole> = s.windows_ground.back_x.iter().filter(|x| DOOR_Z + **x < tlo.y - 0.8).map(|x| win(DOOR_Z + x - z0)).collect();
    let mid = (x0 + x1) * 0.5;
    let walls: [(&str, Vec2, Vec2, Vec<Hole>, Vec3); 4] = [
        ("Front wall", Vec2::new(x0, z0), Vec2::new(x0, z1), vec![], Vec3::NEG_X),
        ("Back wall", Vec2::new(x1, z0), Vec2::new(x1, tlo.y), back.clone(), Vec3::X),
        ("Left wall", Vec2::new(x0, z0), Vec2::new(x1, z0), vec![win(mid - 2.2 - x0), win(mid + 2.6 - x0)], Vec3::NEG_Z),
        ("Right wall", Vec2::new(x0, z1), Vec2::new(tlo.x, z1), vec![win(29.5 - x0)], Vec3::Z),
    ];
    let (plaster, inner, timber) = (b.m.plaster.clone(), b.m.inner.clone(), b.m.timber.clone());
    for (name, a, c, holes, out) in walls.iter() {
        let (a, c) = extend(*a, *c, UPPER_T * 0.5);
        let holes: Vec<Hole> = holes.iter().map(|h| Hole { at: h.at + UPPER_T * 0.5, ..*h }).collect();
        b.wall(name, a, c, y0, y1, UPPER_T, &holes, &plaster, true);
        let inward = -Vec2::new(out.x, out.z);
        let (la, lc) = (a + inward * (UPPER_T * 0.5 + 0.015), c + inward * (UPPER_T * 0.5 + 0.015));
        b.wall("Inner plaster", la, lc, y0, y1, 0.03, &holes, &inner, false);
        // Timber frame on the outer face: sill and head plates, posts between the windows and
        // corner braces.
        let along = (c - a).normalize();
        let along3 = Vec3::new(along.x, 0.0, along.y);
        let face = |t: f32, y: f32| Vec3::new(a.x, y, a.y) + along3 * t + *out * (UPPER_T * 0.5 + 0.04);
        let len = (c - a).length();
        let rot = Quat::from_rotation_y(-along.y.atan2(along.x));
        for y in [y0 + 0.1, y1 - 0.1, y0 + UPPER_SILL - 0.05] {
            b.block("Frame plate", Vec3::new(len, 0.18, 0.1), &timber, Transform::from_translation(face(len * 0.5, y)).with_rotation(rot), false);
        }
        let blocked = |t: f32| holes.iter().any(|h| (t - h.at).abs() < h.w * 0.5 + 0.12);
        let mut t = 0.08;
        while t < len {
            if !blocked(t) {
                b.block("Frame post", Vec3::new(0.18, y1 - y0, 0.1), &timber, Transform::from_translation(face(t, (y0 + y1) * 0.5)).with_rotation(rot), false);
            }
            t += 1.6;
        }
        for (t0, dirn) in [(0.25f32, 1.0f32), (len - 0.25, -1.0)] {
            let span = 1.2;
            let h = y1 - y0 - 0.4;
            let ang = (h / span).atan() * dirn;
            let centre = face(t0 + dirn * span * 0.5, (y0 + y1) * 0.5);
            b.block(
                "Frame brace",
                Vec3::new((span * span + h * h).sqrt(), 0.14, 0.09),
                &timber,
                Transform::from_translation(centre).with_rotation(rot * Quat::from_rotation_z(ang)),
                false,
            );
        }
        // The jetty bressumer: a heavy beam along the overhang's foot.
        b.block("Jetty beam", Vec3::new(len, 0.28, 0.24), &timber, Transform::from_translation(face(len * 0.5, y0 - 0.1)).with_rotation(rot), false);
        for h in &holes {
            let centre = Vec3::new(a.x, (h.bottom + h.top) * 0.5, a.y) + along3 * h.at;
            b.window(centre, along3, *out, h.w, h.top - h.bottom, UPPER_T);
        }
    }
    // Partitions: the back strip (office) and records | lounge, with doorways.
    let inner_x0 = f.x0 - j + UPPER_T;
    let door_h = 2.1;
    b.wall(
        "Partition",
        Vec2::new(BACK_STRIP_X, f.z0 - j + UPPER_T),
        Vec2::new(BACK_STRIP_X, tlo.y),
        y0,
        y1,
        PARTITION_T,
        &[Hole { at: 8.0, w: 1.1, bottom: y0, top: y0 + door_h }],
        &inner,
        true,
    );
    b.wall(
        "Partition",
        Vec2::new(inner_x0, ROOMS_Z),
        Vec2::new(BACK_STRIP_X, ROOMS_Z),
        y0,
        y1,
        PARTITION_T,
        &[Hole { at: BACK_STRIP_X - inner_x0 - 1.2, w: 1.1, bottom: y0, top: y0 + door_h }],
        &inner,
        true,
    );
    // Flat ceiling at the eaves (boards), so the rooms read as rooms.
    let ceiling = b.m.ceiling.clone();
    b.span("Upper ceiling", Vec3::new(f.x0 - j, y1 - 0.04, f.z0 - j), Vec3::new(f.x1 + j, y1, f.z1 + j), &ceiling, false);
}

/// Slate roof (ridge along z), plaster gables with a king post, ridge cap.
fn roof(b: &mut Build, s: &Spec, f: &Frame) {
    let (slate, plaster, timber) = (b.m.slate.clone(), b.m.plaster.clone(), b.m.timber.clone());
    let j = f.jetty;
    let half = (f.x1 - f.x0) * 0.5 + j;
    let cx = (f.x0 + f.x1) * 0.5;
    let cz = (f.z0 + f.z1) * 0.5;
    let [side, gable] = s.roof.eave_overhang;
    let rise = s.roof.height;
    let slope = rise / half;
    let run = half + side;
    let angle = slope.atan();
    let len = run / angle.cos();
    let width = (f.z1 - f.z0) + 2.0 * (j + gable);
    let t = 0.22;
    for dir in [-1.0f32, 1.0] {
        // The slab's top surface runs from the ridge out to the eave, `run` horizontally.
        let centre = Vec3::new(cx + dir * run * 0.5, f.ridge - slope * run * 0.5, cz);
        let rot = Quat::from_rotation_z(-dir * angle);
        let lift = rot * Vec3::new(0.0, -t * 0.5, 0.0);
        b.block("Roof", Vec3::new(len, t, width), &slate, Transform::from_translation(centre + lift).with_rotation(rot), true);
        // Fascia board along the eave.
        let eave = Vec3::new(cx + dir * run, f.ridge - slope * run - t * 0.5, cz);
        b.block("Fascia", Vec3::new(0.06, 0.26, width), &timber, Transform::from_translation(eave), false);
    }
    b.block("Ridge cap", Vec3::new(0.36, 0.2, width + 0.05), &slate, Transform::from_xyz(cx, f.ridge + 0.04, cz).with_rotation(Quat::from_rotation_x(0.0)), false);
    // Gables: plaster triangles (flush with the upper walls) with a king post and a collar.
    for (z, out) in [(f.z0 - j + UPPER_T * 0.5, -1.0f32), (f.z1 + j - UPPER_T * 0.5, 1.0)] {
        let tri = Extrusion::new(Triangle2d::new(Vec2::new(-half, 0.0), Vec2::new(half, 0.0), Vec2::new(0.0, rise)), UPPER_T);
        b.mesh("Gable", tri, &plaster, Transform::from_xyz(cx, f.eaves, z));
        let face = z + out * (UPPER_T * 0.5 + 0.04);
        b.block("King post", Vec3::new(0.2, rise - 0.3, 0.1), &timber, Transform::from_xyz(cx, f.eaves + (rise - 0.3) * 0.5, face), false);
        b.block("Collar", Vec3::new(half * 1.1, 0.18, 0.1), &timber, Transform::from_xyz(cx, f.eaves + rise * 0.45, face), false);
        // Bargeboards along the gable's two slopes.
        for dir in [-1.0f32, 1.0] {
            let centre = Vec3::new(cx + dir * run * 0.5, f.ridge - slope * run * 0.5 - 0.15, z + out * (UPPER_T * 0.5 + gable));
            b.block("Bargeboard", Vec3::new(len, 0.26, 0.07), &timber, Transform::from_translation(centre).with_rotation(Quat::from_rotation_z(-dir * angle)), false);
        }
    }
}

/// The bell tower in the back-right corner: stone base, plaster shaft with corner posts, open
/// belfry with a bell, pyramid slate roof.
fn tower(b: &mut Build, s: &Spec, f: &Frame) {
    let (lo, hi) = f.tower(s);
    let top = f.floor + s.tower.wall_top_above_podium;
    let point = f.floor + s.tower.point_top_above_podium;
    let belfry = top - 2.4;
    let (stone, plaster, timber, slate, planks, bronze, iron) =
        (b.m.wall.clone(), b.m.plaster.clone(), b.m.timber.clone(), b.m.slate.clone(), b.m.planks.clone(), b.m.bronze.clone(), b.m.iron.clone());
    b.span("Tower base", Vec3::new(lo.x, f.floor, lo.y), Vec3::new(hi.x, f.upper, hi.y), &stone, true);
    b.span("Tower shaft", Vec3::new(lo.x, f.upper, lo.y), Vec3::new(hi.x, belfry, hi.y), &plaster, true);
    let c = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let x = c.x + sx * (half.x - 0.12);
        let z = c.y + sz * (half.y - 0.12);
        b.span("Tower post", Vec3::new(x - 0.14, f.upper, z - 0.14), Vec3::new(x + 0.14, top, z + 0.14), &timber, false);
    }
    // Bands at the belfry floor and under the roof, the belfry floor itself.
    for y in [belfry, top - 0.15] {
        b.span("Tower band", Vec3::new(lo.x - 0.05, y - 0.08, lo.y - 0.05), Vec3::new(hi.x + 0.05, y + 0.15, hi.y + 0.05), &timber, false);
    }
    b.span("Belfry floor", Vec3::new(lo.x, belfry, lo.y), Vec3::new(hi.x, belfry + 0.12, hi.y), &planks, true);
    // Low parapets between the posts.
    for (a, c2) in [
        (Vec3::new(lo.x, belfry, lo.y), Vec3::new(hi.x, belfry + 0.8, lo.y + 0.12)),
        (Vec3::new(lo.x, belfry, hi.y - 0.12), Vec3::new(hi.x, belfry + 0.8, hi.y)),
        (Vec3::new(lo.x, belfry, lo.y), Vec3::new(lo.x + 0.12, belfry + 0.8, hi.y)),
        (Vec3::new(hi.x - 0.12, belfry, lo.y), Vec3::new(hi.x, belfry + 0.8, hi.y)),
    ] {
        b.span("Belfry parapet", a, c2, &plaster, true);
    }
    // The bell on a yoke.
    b.span("Bell yoke", Vec3::new(lo.x + 0.2, top - 0.55, c.y - 0.08), Vec3::new(hi.x - 0.2, top - 0.4, c.y + 0.08), &iron, false);
    let bell = Cone { radius: 0.5, height: 0.85 }.mesh().resolution(16).build();
    b.mesh("Bell", bell, &bronze, Transform::from_xyz(c.x, top - 1.05, c.y));
    b.mesh("Bell lip", Torus::new(0.42, 0.52), &bronze, Transform::from_xyz(c.x, top - 1.47, c.y));
    // Pyramid roof: a four-sided cone turned 45°.
    let r = (half.max_element() + 0.35) * std::f32::consts::SQRT_2;
    let h = point - top;
    let pyramid = Cone { radius: r, height: h }.mesh().resolution(4).build();
    b.mesh("Tower roof", pyramid, &slate, Transform::from_xyz(c.x, top + h * 0.5, c.y).with_rotation(Quat::from_rotation_y(FRAC_PI_4)));
    b.mesh("Finial", Sphere::new(0.12), &iron, Transform::from_xyz(c.x, point + 0.08, c.y));
    // The office door into the tower (shut; the tower stair is for later).
    let face = lo.y - 0.04;
    b.span("Tower door", Vec3::new(c.x - 0.45, f.upper, face - 0.05), Vec3::new(c.x + 0.45, f.upper + 2.0, face + 0.02), &timber, false);
    b.span("Tower door strap", Vec3::new(c.x - 0.42, f.upper + 0.5, face - 0.08), Vec3::new(c.x + 0.42, f.upper + 0.58, face - 0.04), &iron, false);
    b.span("Tower door strap", Vec3::new(c.x - 0.42, f.upper + 1.5, face - 0.08), Vec3::new(c.x + 0.42, f.upper + 1.58, face - 0.04), &iron, false);
}

/// The chimney stack on the −z wall and the two hearths it serves (hall and records room).
fn chimney_and_hearths(b: &mut Build, s: &Spec, f: &Frame) {
    let (stone, soot, fire, timber, wall) = (b.m.podium.clone(), b.m.soot.clone(), b.m.fire.clone(), b.m.timber.clone(), b.m.wall.clone());
    let w = s.chimney.width;
    let top = f.floor + s.chimney.top_above_podium;
    let out = f.z0 - f.jetty - 0.05;
    // The stack stands just outside the upper wall line, from the podium to above the ridge line.
    b.span("Chimney", Vec3::new(HEARTH_X - w * 0.5, f.floor, out - 1.0), Vec3::new(HEARTH_X + w * 0.5, top, out), &wall, true);
    b.span("Chimney cap", Vec3::new(HEARTH_X - w * 0.5 - 0.1, top, out - 1.1), Vec3::new(HEARTH_X + w * 0.5 + 0.1, top + 0.2, out + 0.1), &stone, false);
    b.span("Chimney pot", Vec3::new(HEARTH_X - 0.2, top + 0.2, out - 0.7), Vec3::new(HEARTH_X + 0.2, top + 0.55, out - 0.3), &soot, false);
    // The hall hearth: a stone surround with a firebox, a mantel and a breast up to the beams.
    let zin = f.z0 + WALL_T * 0.5;
    let (hw, hd, hh) = (1.9, 0.75, 1.25);
    let y = f.floor;
    b.span("Hearth", Vec3::new(HEARTH_X - hw * 0.5, y, zin), Vec3::new(HEARTH_X + hw * 0.5, y + hh, zin + hd), &stone, true);
    b.span("Firebox", Vec3::new(HEARTH_X - 0.55, y + 0.05, zin + hd - 0.45), Vec3::new(HEARTH_X + 0.55, y + 0.85, zin + hd + 0.01), &soot, false);
    b.span("Hearthstone", Vec3::new(HEARTH_X - hw * 0.5 - 0.1, y, zin + hd), Vec3::new(HEARTH_X + hw * 0.5 + 0.1, y + 0.06, zin + hd + 0.5), &stone, false);
    b.span("Mantel", Vec3::new(HEARTH_X - hw * 0.5 - 0.1, y + hh, zin), Vec3::new(HEARTH_X + hw * 0.5 + 0.1, y + hh + 0.15, zin + hd + 0.1), &timber, false);
    b.span("Chimney breast", Vec3::new(HEARTH_X - 0.65, y + hh + 0.15, zin), Vec3::new(HEARTH_X + 0.65, f.upper - SLAB, zin + 0.5), &stone, false);
    b.mesh("Hearth fire", Sphere::new(0.22), &fire, Transform::from_xyz(HEARTH_X, y + 0.25, zin + hd - 0.2).with_scale(Vec3::new(1.4, 1.0, 0.8)));
    for dx in [-0.25f32, 0.25] {
        b.block("Log", Vec3::new(0.6, 0.1, 0.1), &timber, Transform::from_xyz(HEARTH_X + dx * 0.4, y + 0.1, zin + hd - 0.2).with_rotation(Quat::from_rotation_y(dx * 2.0)), false);
    }
    b.light("Hearth glow", Vec3::new(HEARTH_X, y + 0.6, zin + hd + 0.3), Color::srgb(1.0, 0.55, 0.25), 90_000.0, 9.0);
    // The records-room hearth upstairs: smaller, on the same breast.
    let zu = f.z0 - f.jetty + UPPER_T;
    let yu = f.upper;
    b.span("Small hearth", Vec3::new(HEARTH_X - 0.6, yu, zu), Vec3::new(HEARTH_X + 0.6, yu + 0.95, zu + 0.5), &stone, true);
    b.span("Small firebox", Vec3::new(HEARTH_X - 0.35, yu + 0.05, zu + 0.2), Vec3::new(HEARTH_X + 0.35, yu + 0.6, zu + 0.51), &soot, false);
    b.mesh("Small fire", Sphere::new(0.14), &fire, Transform::from_xyz(HEARTH_X, yu + 0.18, zu + 0.38));
    b.span("Small breast", Vec3::new(HEARTH_X - 0.45, yu + 0.95, zu), Vec3::new(HEARTH_X + 0.45, f.eaves, zu + 0.35), &stone, false);
}

/// The porch over the west door, the banners and the lanterns either side of it.
fn porch_banners_lanterns(b: &mut Build, s: &Spec, f: &Frame) {
    let (slate, timber, iron, glow, banner) = (b.m.slate.clone(), b.m.timber.clone(), b.m.iron.clone(), b.m.glow.clone(), b.m.banner.clone());
    let p = &s.porch;
    let top = f.floor + p.top_above_podium;
    let eave = top - 1.1;
    let x_out = f.x0 - WALL_T * 0.5 - p.depth;
    let hw = p.width * 0.5;
    // Posts and the tie beam at the front, a wall plate at the back.
    for side in [-1.0f32, 1.0] {
        let z = DOOR_Z + side * (hw - 0.2);
        b.span("Porch post", Vec3::new(x_out, f.floor, z - 0.11), Vec3::new(x_out + 0.22, eave, z + 0.11), &timber, true);
        b.span("Porch brace", Vec3::new(x_out + 0.05, eave - 0.6, z - 0.06), Vec3::new(x_out + 0.17, eave - 0.05, z + 0.06), &timber, false);
    }
    b.span("Porch beam", Vec3::new(x_out - 0.05, eave - 0.05, DOOR_Z - hw), Vec3::new(x_out + 0.25, eave + 0.2, DOOR_Z + hw), &timber, false);
    b.span("Porch beam", Vec3::new(x_out, eave - 0.05, DOOR_Z - 0.1), Vec3::new(f.x0, eave + 0.15, DOOR_Z + 0.1), &timber, false);
    // Two slate slabs with the ridge along x (gable facing the courtyard).
    let rise = top - eave;
    let run = hw + 0.25;
    let angle = (rise / hw).atan();
    let len = run / angle.cos();
    let depth = p.depth + WALL_T * 0.5 + 0.3;
    for dir in [-1.0f32, 1.0] {
        let centre = Vec3::new(x_out + depth * 0.5 - 0.3, top - (rise / hw) * run * 0.5 - 0.08, DOOR_Z + dir * run * 0.5);
        b.block("Porch roof", Vec3::new(depth, 0.16, len), &slate, Transform::from_translation(centre).with_rotation(Quat::from_rotation_x(dir * angle)), false);
    }
    let tri = Extrusion::new(Triangle2d::new(Vec2::new(-hw, 0.0), Vec2::new(hw, 0.0), Vec2::new(0.0, rise)), 0.08);
    let plaster = b.m.plaster.clone();
    b.mesh("Porch gable", tri, &plaster, Transform::from_xyz(x_out + 0.11, eave + 0.2, DOOR_Z).with_rotation(Quat::from_rotation_y(FRAC_PI_2)));
    // Banners: the cloth keeps the texture's proportions at the spec's width, top at the spec's
    // height; an iron rod across the top on two brackets.
    let bw = s.banners.size[0];
    let bh = bw * (1024.0 / 494.0);
    let btop = f.floor + s.banners.top_above_podium;
    let face = f.x0 - f.jetty - UPPER_T * 0.5 - 0.12;
    for x in &s.banners.x {
        let z = DOOR_Z + x;
        b.picture("Banner", Vec2::new(bw, bh), &banner, Vec3::new(face, btop - bh * 0.5, z), Vec3::NEG_X);
        b.span("Banner rod", Vec3::new(face - 0.04, btop, z - bw * 0.5 - 0.12), Vec3::new(face + 0.04, btop + 0.06, z + bw * 0.5 + 0.12), &iron, false);
        for dz in [-0.5f32, 0.5] {
            b.span("Banner bracket", Vec3::new(face, btop - 0.02, z + dz * bw - 0.03), Vec3::new(face + 0.3, btop + 0.04, z + dz * bw + 0.03), &iron, false);
        }
    }
    // Lanterns on iron arms either side of the door.
    for x in &s.lanterns.x {
        let at = Vec3::new(f.x0 - WALL_T * 0.5 - 0.35, f.floor + s.lanterns.height_above_podium, DOOR_Z + x);
        b.span("Lantern arm", Vec3::new(at.x, at.y + 0.3, at.z - 0.03), Vec3::new(f.x0 - WALL_T * 0.5, at.y + 0.36, at.z + 0.03), &iron, false);
        b.block("Lantern cage", Vec3::new(0.26, 0.38, 0.26), &iron, Transform::from_translation(at), false);
        b.block("Lantern glow", Vec3::new(0.2, 0.28, 0.2), &glow, Transform::from_translation(at), false);
        b.light("Lantern", at - Vec3::X * 0.2, Color::srgb(1.0, 0.72, 0.4), 45_000.0, 7.0);
    }
}

/// Upstairs furniture (records, lounge, office) and the hall rug. The ground floor's working
/// furniture (desk, board, beds, table) is `hall`'s.
fn furniture(b: &mut Build, f: &Frame) {
    let (timber, planks, paper, iron, rug, glow, wainscot) =
        (b.m.timber.clone(), b.m.planks.clone(), b.m.paper.clone(), b.m.iron.clone(), b.m.rug.clone(), b.m.glow.clone(), b.m.wainscot.clone());
    let y = f.upper;
    let inner_x0 = f.x0 - f.jetty + UPPER_T;
    let inner_z0 = f.z0 - f.jetty + UPPER_T;
    // The hall rug between the door and the desk.
    let rug_size = Vec2::new(2.4, 2.4 * (1024.0 / 687.0));
    b.picture("Rug", Vec2::new(rug_size.y, rug_size.x), &rug, Vec3::new(f.x0 + 4.2, f.floor + 0.03, DOOR_Z), Vec3::Y);
    // Records (front-left): ledger shelves along the front wall, filing chests, a standing desk.
    for k in 0..3 {
        let z = inner_z0 + 1.6 + k as f32 * 1.6;
        if z > ROOMS_Z - 0.8 {
            break;
        }
        shelf(b, Vec3::new(inner_x0 + 0.22, y, z), 1.4, &timber, &paper, true);
    }
    for k in 0..2 {
        let x = inner_x0 + 2.4 + k as f32 * 0.9;
        b.span("Filing chest", Vec3::new(x, y, ROOMS_Z - 0.9), Vec3::new(x + 0.75, y + 0.7, ROOMS_Z - 0.3), &wainscot, true);
        b.span("Chest band", Vec3::new(x - 0.01, y + 0.6, ROOMS_Z - 0.91), Vec3::new(x + 0.76, y + 0.65, ROOMS_Z - 0.29), &iron, false);
    }
    b.span("Clerk desk", Vec3::new(28.6, y, inner_z0 + 2.4), Vec3::new(29.4, y + 1.15, inner_z0 + 3.4), &timber, true);
    b.span("Ledger", Vec3::new(28.75, y + 1.15, inner_z0 + 2.6), Vec3::new(29.25, y + 1.2, inner_z0 + 3.0), &paper, false);
    // Lounge (front-right): long table with benches along x, by the stair landing.
    let (tz, tx0, tx1) = (-6.2, 26.6, 29.6);
    b.span("Long table", Vec3::new(tx0, y + 0.72, tz - 0.45), Vec3::new(tx1, y + 0.8, tz + 0.45), &planks, true);
    for x in [tx0 + 0.2, tx1 - 0.3] {
        b.span("Table leg", Vec3::new(x, y, tz - 0.35), Vec3::new(x + 0.1, y + 0.72, tz + 0.35), &timber, false);
    }
    for dz in [-0.85f32, 0.85] {
        b.span("Bench", Vec3::new(tx0 + 0.1, y + 0.4, tz + dz - 0.17), Vec3::new(tx1 - 0.1, y + 0.46, tz + dz + 0.17), &planks, true);
        b.span("Bench foot", Vec3::new(tx0 + 0.3, y, tz + dz - 0.12), Vec3::new(tx0 + 0.4, y + 0.4, tz + dz + 0.12), &timber, false);
        b.span("Bench foot", Vec3::new(tx1 - 0.4, y, tz + dz - 0.12), Vec3::new(tx1 - 0.3, y + 0.4, tz + dz + 0.12), &timber, false);
    }
    for (k, x) in [27.0f32, 27.9, 28.9].into_iter().enumerate() {
        b.mesh("Mug", Cylinder::new(0.05, 0.12), &iron, Transform::from_xyz(x, y + 0.86, tz + if k % 2 == 0 { 0.2 } else { -0.15 }));
    }
    b.span("Map", Vec3::new(28.0, y + 0.8, tz - 0.3), Vec3::new(28.8, y + 0.805, tz + 0.25), &paper, false);
    // A hanging lantern over the table: the one light upstairs.
    let lamp = Vec3::new((tx0 + tx1) * 0.5, f.eaves - 0.9, tz);
    b.span("Lamp chain", lamp + Vec3::new(-0.02, 0.15, -0.02), Vec3::new(lamp.x + 0.02, f.eaves, lamp.z + 0.02), &iron, false);
    b.block("Lamp", Vec3::new(0.3, 0.36, 0.3), &glow, Transform::from_translation(lamp), false);
    b.light("Lounge lamp", lamp - Vec3::Y * 0.2, Color::srgb(1.0, 0.72, 0.42), 60_000.0, 10.0);
    // Office (back strip, left of the tower): desk facing the doorway, map table, bookshelves.
    let ox = (BACK_STRIP_X + f.x1 + f.jetty - UPPER_T) * 0.5;
    b.span("Office desk", Vec3::new(ox - 0.45, y + 0.72, -10.6), Vec3::new(ox + 0.45, y + 0.8, -9.0), &planks, true);
    b.span("Desk body", Vec3::new(ox - 0.4, y, -10.5), Vec3::new(ox + 0.4, y + 0.72, -9.1), &timber, true);
    b.span("Map table", Vec3::new(ox - 0.6, y + 0.85, -14.2), Vec3::new(ox + 0.6, y + 0.92, -12.6), &timber, true);
    b.span("Map table legs", Vec3::new(ox - 0.5, y, -14.0), Vec3::new(ox + 0.5, y + 0.85, -12.8), &timber, false);
    b.span("Regional map", Vec3::new(ox - 0.55, y + 0.92, -14.15), Vec3::new(ox + 0.55, y + 0.925, -12.65), &paper, false);
    for z in [-8.2f32, -7.0] {
        shelf(b, Vec3::new(f.x1 + f.jetty - UPPER_T - 0.22, y, z), 1.1, &timber, &paper, false);
    }
}

/// A tall shelf of ledgers against a wall: `at` is its foot centre, `len` along z; `facing_x`
/// true when it faces +x (on the −x wall), else −x.
fn shelf(b: &mut Build, at: Vec3, len: f32, timber: &Handle<StandardMaterial>, paper: &Handle<StandardMaterial>, facing_x: bool) {
    let d = 0.4;
    for dz in [-0.5f32, 0.5] {
        b.span("Shelf side", Vec3::new(at.x - d * 0.5, at.y, at.z + dz * len - 0.03), Vec3::new(at.x + d * 0.5, at.y + 2.2, at.z + dz * len + 0.03), timber, true);
    }
    for (k, h) in [0.1f32, 0.75, 1.4, 2.05].into_iter().enumerate() {
        b.span("Shelf board", Vec3::new(at.x - d * 0.5, at.y + h, at.z - len * 0.5), Vec3::new(at.x + d * 0.5, at.y + h + 0.04, at.z + len * 0.5), timber, false);
        if k < 3 {
            let n = (len / 0.12) as usize;
            for j in 0..n {
                if (j * 7 + k * 3) % 5 == 0 {
                    continue;
                }
                let z = at.z - len * 0.5 + 0.06 + j as f32 * 0.12;
                let tall = 0.42 + ((j * 13 + k * 5) % 4) as f32 * 0.04;
                let back = if facing_x { -0.05 } else { 0.05 };
                b.span("Ledger", Vec3::new(at.x - 0.13 + back, at.y + h + 0.04, z - 0.045), Vec3::new(at.x + 0.13 + back, at.y + h + 0.04 + tall, z + 0.045), paper, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Spec {
        Spec::load(&spec_path()).expect("guildhouse.json")
    }

    #[test]
    fn the_spec_loads_and_fits_the_hall_footprint() {
        let s = spec();
        assert!(s.check().is_ok());
        // Spot-check the readings the build leans on.
        assert_eq!(s.steps.count, 6);
        assert!(s.storeys[1].overhang > 0.0 && s.storeys[1].overhang < 1.0);
        assert!(s.tower.point_top_above_podium > s.tower.wall_top_above_podium);
    }

    #[test]
    fn heights_stack_up_and_the_tower_tops_the_ridge() {
        let s = spec();
        let f = Frame::new(&s);
        assert!(f.floor < f.upper && f.upper < f.eaves && f.eaves < f.ridge);
        assert!(FLOOR_Y + s.tower.wall_top_above_podium > f.ridge, "tower walls clear the ridge");
        assert!(FLOOR_Y + s.chimney.top_above_podium > f.eaves, "chimney clears the eaves");
    }

    #[test]
    fn the_stair_climbs_in_walkable_steps_and_misses_the_tower_and_yard_door() {
        let s = spec();
        let f = Frame::new(&s);
        let rise = (f.upper - f.floor) / STAIR_STEPS as f32;
        assert!(rise <= 0.26, "rise {rise}");
        let (tlo, _) = f.tower(&s);
        assert!(STAIR_X.1 < tlo.x, "stair ends before the tower");
        assert!(STAIR_X.0 > YARD_DOOR_X + YARD_DOOR_W * 0.5, "stair starts past the yard door");
        // The yard door sits between the front wall and the stair, clear of both corners.
        assert!(YARD_DOOR_X - YARD_DOOR_W * 0.5 > f.x0 + 0.3);
    }

    #[test]
    fn rooms_upstairs_hold_their_furniture() {
        let s = spec();
        let f = Frame::new(&s);
        let (tlo, thi) = f.tower(&s);
        // The tower is in the back-right corner and inside the back strip.
        assert!(tlo.x >= BACK_STRIP_X && (thi.y - (f.z1 + f.jetty)).abs() < 1e-4);
        // The stairwell (front-right) stays clear of the records | lounge partition.
        assert!(HALL_MAX.y - WALL_T * 0.5 - STAIR_W > ROOMS_Z + 1.0);
    }

    #[test]
    fn holes_split_a_wall_into_the_right_pieces() {
        let area = |p: &[(f32, f32, f32, f32)]| p.iter().map(|(a, b, c, d)| (b - a) * (d - c)).sum::<f32>();
        // 10 x 3 m with a 1 x 1 m window: two piers, under and over = 4 pieces, 29 m².
        let win = Hole { at: 5.0, w: 1.0, bottom: 1.0, top: 2.0 };
        let p = split(10.0, 0.0, 3.0, &[win]);
        assert_eq!(p.len(), 4);
        assert!((area(&p) - 29.0).abs() < 1e-4);
        // A door to the floor leaves no sill piece; a hole past the end is clipped.
        let door = Hole { at: 9.6, w: 1.2, bottom: 0.0, top: 2.0 };
        let p = split(10.0, 0.0, 3.0, &[door]);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!((area(&p) - (30.0 - 1.0 * 2.0)).abs() < 1e-4, "clipped at the end: 1 m of door left");
        // Unsorted holes come out in order and never overlap.
        let p = split(10.0, 0.0, 3.0, &[win, Hole { at: 2.0, ..win }]);
        assert!(p.windows(2).all(|w| w[0].0 <= w[1].0));
        assert!((area(&p) - 28.0).abs() < 1e-4);
    }
}
