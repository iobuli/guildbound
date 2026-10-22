//! Pixel-art flipbook effects: camera-facing quads that play one of the 100 px sheets in `assets/vfx/pixel` once (hits,
//! casts, bursts) or looping for a set time (zones, auras). Each can be additive (glow) or alpha-blended (smoke, scorch),
//! so a spell stacks both: a glowing beam over dark smoke. Other systems ask with a `Flip` message.
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// Sheets: file stem, grid size (cells per side), frames used.
pub const SHEETS: [(&str, usize, usize); 20] = [
    ("1_magicspell", 9, 75),
    ("2_magic8", 8, 61),
    ("3_bluefire", 8, 61),
    ("4_casting", 9, 73),
    ("5_magickahit", 7, 41),
    ("6_flamelash", 7, 46),
    ("7_firespin", 8, 61),
    ("8_protectioncircle", 8, 61),
    ("9_brightfire", 8, 61),
    ("10_weaponhit", 6, 31),
    ("11_fire", 8, 61),
    ("12_nebula", 8, 61),
    ("13_vortex", 8, 61),
    ("14_phantom", 8, 61),
    ("15_loading", 11, 121),
    ("16_sunburn", 8, 61),
    ("17_felspell", 10, 91),
    ("18_midnight", 8, 61),
    ("19_freezing", 10, 86),
    ("20_magicbubbles", 8, 61),
];

/// Named sheets the game asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    MagicSpell = 0,
    Magic8,
    BlueFire,
    Casting,
    MagicHit,
    FlameLash,
    FireSpin,
    Ward,
    BrightFire,
    WeaponHit,
    Fire,
    Nebula,
    Vortex,
    Phantom,
    Loading,
    Sunburn,
    Fel,
    Midnight,
    Freezing,
    Bubbles,
}

impl Sheet {
    pub const ALL: [Sheet; 20] = [
        Sheet::MagicSpell, Sheet::Magic8, Sheet::BlueFire, Sheet::Casting, Sheet::MagicHit, Sheet::FlameLash, Sheet::FireSpin,
        Sheet::Ward, Sheet::BrightFire, Sheet::WeaponHit, Sheet::Fire, Sheet::Nebula, Sheet::Vortex, Sheet::Phantom,
        Sheet::Loading, Sheet::Sunburn, Sheet::Fel, Sheet::Midnight, Sheet::Freezing, Sheet::Bubbles,
    ];
}

#[derive(Message, Clone, Copy, Debug)]
pub struct Flip {
    pub sheet: Sheet,
    pub at: Vec3,
    /// World size of the quad edge in metres.
    pub size: f32,
    /// Seconds for one pass through the sheet.
    pub life: f32,
    /// Plays this many passes (1 = once, then fades out).
    pub loops: u32,
    pub tint: Color,
    /// 0 keeps the sheet's own colours, 1 replaces them with `tint`.
    pub recolor: f32,
    pub additive: bool,
    pub intensity: f32,
}

impl Flip {
    pub fn new(sheet: Sheet, at: Vec3, size: f32, life: f32) -> Self {
        Self { sheet, at, size, life, loops: 1, tint: Color::WHITE, recolor: 0.0, additive: true, intensity: 1.0 }
    }
    pub fn tinted(mut self, c: Color, recolor: f32) -> Self {
        self.tint = c;
        self.recolor = recolor;
        self
    }
    pub fn smoke(mut self) -> Self {
        self.additive = false;
        self
    }
    pub fn looped(mut self, n: u32) -> Self {
        self.loops = n.max(1);
        self
    }
}

#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub struct FlipParams {
    pub tint: Vec4,
    pub frame: Vec4,
    pub mode: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FlipMaterial {
    #[uniform(0)]
    pub params: FlipParams,
    #[texture(1)]
    #[sampler(2)]
    pub sheet: Handle<Image>,
    pub additive: bool,
}

impl Material for FlipMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/flipbook.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/flipbook.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        if self.additive {
            AlphaMode::Add
        } else {
            AlphaMode::Blend
        }
    }
}

#[derive(Component)]
struct Playing {
    sheet: usize,
    age: f32,
    life: f32,
    loops: u32,
    material: Handle<FlipMaterial>,
    opacity: f32,
}

#[derive(Resource)]
struct Sheets {
    images: Vec<Handle<Image>>,
    quad: Handle<Mesh>,
}

fn load(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    let images = SHEETS
        .iter()
        .map(|(n, ..)| {
            assets.load_with_settings(format!("vfx/pixel/{n}_spritesheet.png"), |s: &mut ImageLoaderSettings| {
                s.sampler = ImageSampler::nearest();
            })
        })
        .collect();
    commands.insert_resource(Sheets { images, quad: meshes.add(Rectangle::new(1.0, 1.0)) });
}

fn spawn(mut commands: Commands, mut flips: MessageReader<Flip>, sheets: Option<Res<Sheets>>, mut mats: ResMut<Assets<FlipMaterial>>) {
    let Some(sheets) = sheets else { return };
    for f in flips.read() {
        let i = f.sheet as usize;
        let (_, n, _) = SHEETS[i];
        let c = f.tint.to_linear();
        let material = mats.add(FlipMaterial {
            params: FlipParams {
                tint: Vec4::new(c.red, c.green, c.blue, 1.0),
                frame: Vec4::new(n as f32, 0.0, 1.0, 0.0),
                mode: Vec4::new(if f.additive { 1.0 } else { 0.0 }, f.intensity, f.recolor, 0.0),
            },
            sheet: sheets.images[i].clone(),
            additive: f.additive,
        });
        commands.spawn((
            Mesh3d(sheets.quad.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(f.at).with_scale(Vec3::splat(f.size)),
            Playing { sheet: i, age: 0.0, life: f.life.max(0.05), loops: f.loops, material, opacity: 1.0 },
            bevy::light::NotShadowCaster,
            Name::new("Flip"),
        ));
    }
}

fn play(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Playing)>, mut mats: ResMut<Assets<FlipMaterial>>) {
    for (e, mut p) in &mut q {
        p.age += time.delta_secs();
        let total = p.life * p.loops as f32;
        if p.age >= total {
            commands.entity(e).despawn();
            continue;
        }
        let (_, n, frames) = SHEETS[p.sheet];
        let _ = n;
        let t = (p.age % p.life) / p.life * frames as f32;
        let a = t.floor().min(frames as f32 - 1.0);
        let b = if a + 1.0 >= frames as f32 { a } else { a + 1.0 };
        let fade_out = ((total - p.age) / (p.life * 0.18)).clamp(0.0, 1.0);
        p.opacity = fade_out;
        if let Some(mut m) = mats.get_mut(&p.material) {
            m.params.frame.y = a;
            m.params.frame.z = b;
            m.params.frame.w = t - a;
            m.params.tint.w = fade_out;
        }
    }
}

/// `GB_FLIP=<index|all>[:alpha]` shows that sheet beside the player every 2.4 s (dev smoke test); `all` walks every sheet.
fn flip_smoke(mut commands: Commands, time: Res<Time>, mut out: MessageWriter<Flip>, player: Query<&Transform, With<super::Player>>, mut state: Local<(Option<f32>, usize, Option<(f32, String)>)>) {
    let Ok(v) = std::env::var("GB_FLIP") else { return };
    let (which, mode) = v.split_once(':').unwrap_or((v.as_str(), ""));
    let all = which == "all";
    let now = time.elapsed_secs();
    if let Some((at, path)) = state.2.clone()
        && now >= at
    {
        state.2 = None;
        shoot(&mut commands, path);
    }
    let Ok(pt) = player.single() else { return };
    if now < 2.0 || state.0.is_some_and(|l| now - l < REEL_STEP) || (all && state.1 >= Sheet::ALL.len()) {
        return;
    }
    state.0 = Some(now);
    let idx = if all { state.1 } else { which.parse::<usize>().unwrap_or(0).min(19) };
    state.1 += 1;
    let mut f = Flip::new(Sheet::ALL[idx], pt.translation + Vec3::Y * 1.3 + *pt.forward() * 1.6, 3.0, 1.6);
    f.additive = mode != "alpha";
    out.write(f);
    if let Some(dir) = reel_dir() {
        state.2 = Some((now + REEL_SHOT, format!("{dir}/{}_{idx:02}.png", if f.additive { "add" } else { "alpha" })));
    }
}

/// `GB_REEL=<dir>`: one game run stands in for many screenshots. Effect smoke tests (`GB_BEAM=all`, `GB_FLIP=all[:alpha]`)
/// fire one item every `REEL_STEP` seconds and save a still `REEL_SHOT` seconds after each into the directory.
pub const REEL_STEP: f32 = 2.4;
pub const REEL_SHOT: f32 = 1.2;

pub fn reel_dir() -> Option<String> {
    std::env::var("GB_REEL").ok().filter(|s| !s.trim().is_empty())
}

pub fn shoot(commands: &mut Commands, path: String) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FlipMaterial>::default())
        .add_message::<Flip>()
        .add_systems(Startup, load)
        .add_systems(Update, (spawn, play, flip_smoke).chain());
}
