//! Main menu and pause menu on bevy_lunex.
//! Layout and hover behaviour follow Bevypunk by IDEDARY (MIT OR Apache-2.0,
//! https://github.com/IDEDARY/Bevypunk). Bevypunk's art is not licensed for use, so the look here is ours:
//! the flat zinc theme every screen shares (ui_theme) and the Rajdhani font (SIL OFL, assets/fonts/rajdhani/OFL.txt).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::FontSize;
use bevy_lunex::*;

use crate::sim_bridge::Sim;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    Playing,
}

pub const INK: Color = Color::srgba(0.035, 0.035, 0.043, 0.93);
// ui_theme colours as Bevy colours
const PAPER: Color = Color::srgb(0.957, 0.957, 0.961);
const MUTED: Color = Color::srgb(0.631, 0.631, 0.667);
const CARD: Color = Color::srgba(0.094, 0.094, 0.106, 0.92);
const BORDER: Color = Color::srgb(0.153, 0.153, 0.165);
const ACCENT: Color = Color::srgb(0.176, 0.831, 0.749);

#[derive(Component)]
pub struct MenuScene;

#[derive(Component)]
pub struct MenuButton;

/// Set this finite subtree directly. Lunex 0.7's UiHoverSet duplicator copies
/// the parent's event target into each child dispatch, recursively dispatching
/// the same parent forever. Do not emit UiHoverSet from our pointer observers.
fn menu_hover<E: EntityEvent, const ENABLED: bool>(event: On<E>, children: Query<&Children>, mut hover: Query<&mut UiHover>) {
    let root = event.event_target();
    for entity in std::iter::once(root).chain(children.iter_descendants(root)) {
        if let Ok(mut state) = hover.get_mut(entity) {
            state.enable = ENABLED;
        }
    }
}

/// The 2D camera lunex draws with, layered over the 3D view.
pub fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        // Both cameras must share the HDR intermediate target; mixing HDR and LDR
        // lets the overlay replace the world with black even with clear_color=None.
        bevy::camera::Hdr,
        Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
    ));
}

fn label(assets: &AssetServer, text: &str, size: f32, bold: bool) -> (Text2d, TextFont) {
    let font = if bold { "fonts/rajdhani/Rajdhani-Bold.ttf" } else { "fonts/rajdhani/Rajdhani-Medium.ttf" };
    (
        Text2d::new(text.to_ascii_uppercase()),
        TextFont { font: assets.load(font).into(), font_size: FontSize::Px(size), ..default() },
    )
}

pub fn spawn_menu(mut commands: Commands, assets: Res<AssetServer>, mut sim: ResMut<Sim>) {
    sim.paused = true;
    sim.clear_player_input();
    sim.pending_steps = 0;
    let started = sim.world.tick > 0;
    commands
        .spawn((UiLayoutRoot::new_2d(), UiFetchFromCamera::<0>, MenuScene))
        .with_children(|ui| {
            // left-side panel over the live arena
            ui.spawn((
                Name::new("Panel"),
                UiLayout::window().pos(Rl((4.0, 0.0))).size(Rl((30.0, 100.0))).pack(),
                Sprite::from_color(INK, Vec2::ONE),
                UiColor::from(INK),
                Pickable::IGNORE,
            ));
            // hairline on the panel's right edge
            ui.spawn((
                Name::new("Panel edge"),
                UiLayout::window().pos(Rl((34.0, 0.0))).size((Ab(1.0), Rl(100.0))).pack(),
                Sprite::from_color(BORDER, Vec2::ONE),
                UiColor::from(BORDER),
                Pickable::IGNORE,
            ));
            ui.spawn((
                Name::new("Title"),
                UiLayout::window().pos(Rl((7.0, 14.0))).anchor(Anchor::CENTER_LEFT).pack(),
                UiColor::from(PAPER),
                UiTextSize::from(Rh(9.0)),
                label(&assets, "Guildbound", 128.0, true),
                Pickable::IGNORE,
            ));
            ui.spawn((
                Name::new("Subtitle"),
                UiLayout::window().pos(Rl((7.3, 21.0))).anchor(Anchor::CENTER_LEFT).pack(),
                UiColor::from(ACCENT),
                UiTextSize::from(Rh(2.6)),
                label(&assets, if started { "sparring paused" } else { "the practice courtyard" }, 48.0, false),
                Pickable::IGNORE,
            ));

            ui.spawn(UiLayout::window().pos(Rl((6.0, 32.0))).size(Rl((26.0, 44.0))).pack()).with_children(|ui| {
                let buttons: &[&str] = if started { &["Resume", "New spar", "Practice", "Creator", "Studio", "Quit"] } else { &["Spar", "Practice", "Creator", "Studio", "Quit"] };
                let (size, gap) = (12.5, 3.0);
                for (k, name) in buttons.iter().enumerate() {
                    let mut button = ui.spawn((
                        MenuButton,
                        Name::new(*name),
                        UiLayout::window().y(Rl(k as f32 * (size + gap))).size(Rl((100.0, size))).pack(),
                        OnHoverSetCursor::new(bevy::window::SystemCursorIcon::Pointer),
                    ));
                    button
                        .with_children(|ui| {
                            ui.spawn((
                                UiLayout::new(vec![
                                    (UiBase::id(), UiLayout::window().full()),
                                    (UiHover::id(), UiLayout::window().x(Rl(2.0)).full()),
                                ]),
                                UiHover::new().forward_speed(20.0).backward_speed(6.0),
                                UiColor::new(vec![(UiBase::id(), CARD), (UiHover::id(), PAPER)]),
                                Sprite::default(),
                                Pickable::IGNORE,
                            ))
                            .with_children(|ui| {
                                ui.spawn((
                                    UiLayout::window().pos((Rh(30.0), Rl(50.0))).anchor(Anchor::CENTER_LEFT).pack(),
                                    UiColor::new(vec![(UiBase::id(), PAPER), (UiHover::id(), INK.with_alpha(1.0))]),
                                    UiHover::new().forward_speed(20.0).backward_speed(6.0),
                                    UiTextSize::from(Rh(54.0)),
                                    label(&assets, name, 64.0, false),
                                    Pickable::IGNORE,
                                ));
                                ui.spawn((
                                    UiLayout::window().pos(Rl((92.0, 50.0))).anchor(Anchor::CENTER_RIGHT).pack(),
                                    UiColor::new(vec![(UiBase::id(), MUTED.with_alpha(0.5)), (UiHover::id(), INK.with_alpha(1.0))]),
                                    UiHover::new().forward_speed(20.0).backward_speed(6.0),
                                    UiTextSize::from(Rh(58.0)),
                                    Text2d::new(">"),
                                    TextFont { font: assets.load("fonts/rajdhani/Rajdhani-Bold.ttf").into(), font_size: FontSize::Px(64.0), ..default() },
                                    Pickable::IGNORE,
                                ));
                            });
                        })
                        .observe(menu_hover::<Pointer<Over>, true>)
                        .observe(menu_hover::<Pointer<Out>, false>);
                    match *name {
                        "Resume" => {
                            button.observe(|event: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
                                if event.button == bevy::picking::pointer::PointerButton::Primary { next.set(AppState::Playing); }
                            });
                        }
                        "Spar" | "New spar" => {
                            button.observe(|event: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>, mut sim: ResMut<Sim>| {
                                if event.button != bevy::picking::pointer::PointerButton::Primary { return; }
                                sim.reset();
                                sim.opponent = crate::sim_bridge::OpponentMode::Spar;
                                sim.world.fighters[0].lock_target = Some(1);
                                next.set(AppState::Playing);
                            });
                        }
                        "Practice" => {
                            button.observe(|event: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>, mut sim: ResMut<Sim>| {
                                if event.button != bevy::picking::pointer::PointerButton::Primary { return; }
                                sim.reset();
                                sim.opponent = crate::sim_bridge::OpponentMode::Idle;
                                sim.world.fighters[0].lock_target = None;
                                next.set(AppState::Playing);
                            });
                        }
                        "Creator" => {
                            // the Studio scene opened on its Creator tab: build a character from modular parts
                            button.observe(|event: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>, mut sim: ResMut<Sim>, mut studio: ResMut<crate::studio::Studio>| {
                                if event.button != bevy::picking::pointer::PointerButton::Primary { return; }
                                sim.reset();
                                sim.opponent = crate::sim_bridge::OpponentMode::Idle;
                                sim.world.fighters[0].lock_target = None;
                                studio.scene = true;
                                studio.tab = crate::studio::CREATOR_TAB;
                                if !studio.open {
                                    studio.toggle_request = true;
                                }
                                next.set(AppState::Playing);
                            });
                        }
                        "Studio" => {
                            // the Studio as its own scene: only the stage model, cycling the uniform Quaternius set
                            button.observe(|event: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>, mut sim: ResMut<Sim>, mut studio: ResMut<crate::studio::Studio>| {
                                if event.button != bevy::picking::pointer::PointerButton::Primary { return; }
                                sim.reset();
                                sim.opponent = crate::sim_bridge::OpponentMode::Idle;
                                sim.world.fighters[0].lock_target = None;
                                studio.scene = true;
                                studio.hero_clips = true;
                                if studio.model.is_none() {
                                    studio.request_spawn("packs/hero/mannequin.glb".into());
                                }
                                if !studio.open {
                                    studio.toggle_request = true;
                                }
                                next.set(AppState::Playing);
                            });
                        }
                        "Quit" => {
                            button.observe(|event: On<Pointer<Click>>, mut exit: MessageWriter<AppExit>| {
                                if event.button != bevy::picking::pointer::PointerButton::Primary { return; }
                                info!("Menu Quit requested by primary pointer click at {:?}", event.pointer_location.position);
                                exit.write(AppExit::Success);
                            });
                        }
                        _ => unreachable!("menu only contains implemented actions"),
                    }
                }
            });

            ui.spawn((
                UiLayout::window().pos(Rl((7.0, 94.0))).anchor(Anchor::CENTER_LEFT).pack(),
                UiColor::from(MUTED),
                UiTextSize::from(Rh(2.0)),
                label(&assets, "esc menu  -  tab dev panel  -  f1 inspector", 40.0, false),
                Pickable::IGNORE,
            ));
            for (i, text) in ["Spar - a partner who fights back", "Creator - build a character from parts", "WASD move   SHIFT sprint   CTRL step", "SPACE jump   C roll   RMB guard"].iter().enumerate() {
                ui.spawn((
                    UiLayout::window().pos(Rl((7.0, 78.0 + i as f32 * 3.5))).anchor(Anchor::CENTER_LEFT).pack(),
                    UiColor::from(MUTED),
                    UiTextSize::from(Rh(2.1)),
                    label(&assets, text, 40.0, false),
                    Pickable::IGNORE,
                ));
            }
        });
}

pub fn despawn_menu(mut commands: Commands, scene: Query<Entity, With<MenuScene>>, mut sim: ResMut<Sim>) {
    for e in &scene {
        commands.entity(e).despawn();
    }
    sim.paused = false;
}

/// Esc: play <-> menu (the menu doubles as the pause menu).
pub fn toggle_menu(keys: Res<ButtonInput<KeyCode>>, state: Res<State<AppState>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(match state.get() {
            AppState::Playing => AppState::MainMenu,
            AppState::MainMenu => AppState::Playing,
        });
    }
}

pub fn trace_close_requests(mut close: MessageReader<bevy::window::WindowCloseRequested>) {
    for request in close.read() { info!("OS window close requested for {:?}", request.window); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::picking::{backend::HitData, pointer::{Location, PointerId}};

    #[test]
    fn pointer_hover_updates_a_finite_subtree_and_leaves_other_buttons_alone() {
        let mut app = App::new();
        app.add_plugins(bevy_lunex::UiLunexStatePlugin);
        let world = app.world_mut();
        let window = world.spawn_empty().id();
        let root = world.spawn_empty().observe(menu_hover::<Pointer<Over>, true>).observe(menu_hover::<Pointer<Out>, false>).id();
        let child = world.spawn(UiHover::new()).id();
        let grandchild = world.spawn(UiHover::new()).id();
        let other = world.spawn(UiHover::new()).id();
        world.entity_mut(root).add_child(child);
        world.entity_mut(child).add_child(grandchild);
        // Include the library's observer: our path must never trigger its
        // recursive UiHoverSet duplicator, even with a populated subtree.
        let location = Location { target: bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)).normalize(Some(window)).unwrap(), position: Vec2::ZERO };
        let hit = HitData::new(window, 0.0, None, None);
        for _ in 0..100 {
            world.trigger(Pointer::new(PointerId::Mouse, location.clone(), Over { hit: hit.clone() }, root));
            world.flush();
            assert!(world.get::<UiHover>(child).unwrap().enable);
            assert!(world.get::<UiHover>(grandchild).unwrap().enable);
            assert!(!world.get::<UiHover>(other).unwrap().enable);
            world.trigger(Pointer::new(PointerId::Mouse, location.clone(), Out { hit: hit.clone() }, root));
            world.flush();
            assert!(!world.get::<UiHover>(child).unwrap().enable);
            assert!(!world.get::<UiHover>(grandchild).unwrap().enable);
        }
    }
}
