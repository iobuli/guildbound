//! Studio panel kit: the F2 layout daviy approved, built only from crate::ui_theme pieces.
//!   left rail   : rig + bone tree with search (selected bone = selection #ffbe61)
//!   centre      : 3D view (left empty for the camera) + slim top toolbar: pause/step, ragdoll, balance
//!   right       : inspector for the selection; 5 tabs Pose/Physics/Animation/Events/Controller,
//!                 common controls first, advanced in collapsed sections (ui_theme::advanced)
//!   bottom strip: ragdoll, balance, input lock, tick + fps, margin chip, peak_sat bar
//! Keys: F2 Studio, 1-5 tabs, R ragdoll, B balance, Space pause, '.' step, Ctrl+Z / Ctrl+Y undo/redo.
//! Every edit goes through `EditLog` (studio_kit_core). No UiHoverSet anywhere.
//! Extends: crates/game/src/studio.rs (called from its `ui` system). STAGED, UNCOMPILED (no egui on the box).
use bevy::prelude::*;
use bevy_egui::egui;
use crate::ui_theme as t;

#[path = "studio_kit_core.rs"]
pub mod core;
pub use self::core::*;

/// Kit state. Its own resource; drawn by status_strip_ui / toolbar_ui / bone_tree_ui (below) and, for the
/// tab bar and Events page, by studio.rs's `ui` through `KitParams`.
#[derive(Resource, Default)]
pub struct Kit {
    pub tab: Tab,
    pub bone_query: String,
    /// UI edit history (undo/redo/timeline). The sim-side truth for balance/ragdoll is sandbox.rs's
    /// guildbound_sim::event_log, which logs BalanceSet/RagdollOff itself when these requests apply.
    /// TODO(event_log): fold this into the shared EventLog once it supports undo (compensating events).
    pub log: EditLog,
    pub step_request: bool,
    /// Requests from widgets, forwarded by `keys` (which has free system-param slots).
    pub balance_req: Option<bool>,
    pub ragdoll_req: Option<bool>,
    /// FixedUpdate tick counter (Avian steps in FixedUpdate at 120 Hz); what the status strip shows as "tick".
    pub fixed_tick: u64,
    /// Bone tree rows for the stage rig, rebuilt by `cache_bones` when the rig changes.
    pub bones: Vec<BoneRow>,
    bones_for: Option<Entity>,
}

/// What the strip and toolbar show; studio.rs fills it each frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Status { pub ragdoll: bool, pub balance: bool, pub input_lock: bool, pub paused: bool, pub tick: u64, pub fps: f64, pub margin: f32, pub peak_sat: f32, pub peak_bone: Option<u16> }
impl Status {
    /// Build from the `SandboxShown` resource (same source the Physics section reads).
    pub fn from_shown(shown: &crate::sandbox::SandboxShown, paused: bool, input_lock: bool, fps: f64) -> Self {
        let s = shown.get();
        Status { ragdoll: s.active, balance: s.balance_on, input_lock, paused, tick: s.tick, fps,
                 margin: if s.margin.is_finite() { s.margin } else { -1.0 }, peak_sat: s.peak_sat, peak_bone: None }
    }
}

/// Single-step request from '.' / the Step button: steps to add to the per-frame fixed-time bank.
/// TODO(verify): props.rs time bank name (pass 4). The Rust Expert wires the consumer there: while paused,
/// add `n` fixed steps to the bank, then set this back to 0. Nothing consumes it yet.
#[derive(Resource, Default, Debug)]
pub struct StepRequest(pub u32);

pub fn plugin(app: &mut App) {
    // keys runs in Update; sandbox::apply_balance_toggle reads SetBalance in PostUpdate the same frame.
    // The kit's egui systems run before studio::ui so their panels claim space first; ui lays out in
    // ctx.available_rect() (see studio.rs). Order: bottom strip, top view bar, left bone rail.
    app.init_resource::<Kit>()
        .init_resource::<StepRequest>()
        .add_systems(Update, (keys, cache_bones))
        .add_systems(bevy_egui::EguiPrimaryContextPass, (status_strip_ui, toolbar_ui, bone_tree_ui).chain().before(crate::studio::ui))
        // TODO(verify): Time::<Fixed>::from_hz(120.0) set where Avian moves to FixedUpdate.
        .add_systems(FixedUpdate, |mut kit: ResMut<Kit>| kit.fixed_tick += 1);
}

/// Depth-first bone rows (name, depth) from the stage rig; deterministic: rig.bones order + ChildOf depth.
fn cache_bones(
    studio: Res<crate::studio::Studio>,
    mut kit: ResMut<Kit>,
    rigs: Query<&crate::studio::StudioRig>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
) {
    if kit.bones_for == studio.entity && !kit.bones.is_empty() { return; }
    kit.bones_for = studio.entity;
    kit.bones.clear();
    let Some(rig) = studio.entity.and_then(|e| rigs.get(e).ok()) else { return };
    let set: Vec<Entity> = rig.bones.clone();
    let depth_of = |mut e: Entity| { let mut d = 0u16; while let Ok(p) = parents.get(e) { e = p.parent(); if set.contains(&e) { d += 1; } else { break; } } d };
    for (i, &b) in rig.bones.iter().enumerate() {
        let Ok(n) = names.get(b) else { continue };
        // TODO(verify): driver colour per bone from body_controller::ControllerStore (animated/controller/IK/motor).
        kit.bones.push(BoneRow { bone: i as u16, name: n.as_str().to_owned(), depth: depth_of(b), driver: 0 });
    }

}

/// Studio keys (F2 stays in studio.rs::toggle) + forwarding of widget requests. Skipped while typing.
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut studio: ResMut<crate::studio::Studio>,
    mut kit: ResMut<Kit>,
    mut sim: ResMut<crate::sim_bridge::Sim>,
    mut body: ResMut<crate::possession::PhysicalBody>,
    mut bal: MessageWriter<crate::sandbox::SetBalance>,
    mut contexts: bevy_egui::EguiContexts,
    shown: Res<crate::sandbox::SandboxShown>,
    mut step: ResMut<StepRequest>,
) {
    if !studio.open { kit.balance_req = None; kit.ragdoll_req = None; return; }
    let typing = contexts.ctx_mut().map(|c| c.wants_keyboard_input()).unwrap_or(false);
    let tick = Tick(sim.world.tick);
    let shown = shown.get();
    if !typing {
        let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5];
        // studio::drag_bone holds Digit1-3 as an axis lock only while dragging with a bone selected and the
        // mouse down; tabs switch on just_pressed with no mouse button held.
        // TODO(verify): drag_bone's exact condition (mouse button) so the two never fight.
        for (i, k) in digits.iter().enumerate() {
            if keys.just_pressed(*k) && !ctrl { select_tab(&mut kit, &mut studio, TABS[i]); }
        }
        if keys.just_pressed(KeyCode::KeyR) && !ctrl { kit.ragdoll_req = Some(!body.on); }
        if keys.just_pressed(KeyCode::KeyB) && !ctrl { kit.balance_req = Some(!shown.balance_on); }
        if keys.just_pressed(KeyCode::Space) { sim.paused = !sim.paused; }
        if keys.just_pressed(KeyCode::Period) { kit.step_request = true; }
        if ctrl && (keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::KeyY)) {
            let before = kit.log.toggles();
            if keys.just_pressed(KeyCode::KeyZ) { kit.log.undo(); } else { kit.log.redo(); }
            let after = kit.log.toggles();
            if after.0 != before.0 { kit.ragdoll_req = Some(after.0); }
            if after.1 != before.1 { kit.balance_req = Some(after.1); }
            // undo/redo replays toggles only; reverting the request must not log a new UI edit
            if let Some(on) = kit.ragdoll_req.take() { body.on = on; }
            if let Some(on) = kit.balance_req.take() { bal.write(crate::sandbox::SetBalance(on)); }
        }
    }
    if let Some(on) = kit.ragdoll_req.take() { kit.log.push(tick, Edit::RagdollSet { on }); body.on = on; }
    if let Some(on) = kit.balance_req.take() { kit.log.push(tick, Edit::BalanceSet { on }); bal.write(crate::sandbox::SetBalance(on)); }
    if std::mem::take(&mut kit.step_request) {
        // One fixed step into the pass-4 time bank (props.rs); see StepRequest. No sim.step_once.
        step.0 = step.0.saturating_add(1);
    }
}

/// Maps a primary tab to the legacy Studio tab index so existing content (and CREATOR_TAB/POSE_TAB
/// users like body_controller's F10) keeps working. Events has no legacy page (kit draws it).
pub fn legacy_index(tab: Tab) -> Option<usize> {
    match tab { Tab::Pose => Some(9), Tab::Physics => Some(2), Tab::Animation => Some(0), Tab::Events => None, Tab::Controller => Some(3) }
}
pub fn select_tab(kit: &mut Kit, studio: &mut crate::studio::Studio, tab: Tab) {
    kit.tab = tab;
    if let Some(i) = legacy_index(tab) { studio.tab = i; }
}
/// Keep kit.tab in step when something else (F10, remote gb.studio) set studio.tab.
pub fn sync_from_legacy(kit: &mut Kit, studio_tab: usize) {
    if kit.tab == Tab::Events { return; }
    if let Some(t) = TABS.iter().copied().find(|t| legacy_index(*t) == Some(studio_tab)) { kit.tab = t; }
}

fn tip(label: &str, a: Action) -> String { format!("{label}  [{}]", shortcut(a)) }

/// Left tab rail: chamfered tabs in fixed order with their number key in the tooltip.
pub fn tab_bar(ui: &mut egui::Ui, kit: &mut Kit, studio: &mut crate::studio::Studio, legacy: &[&str]) {
    sync_from_legacy(kit, studio.tab);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for tb in TABS {
            if t::button(ui, tb.label(), kit.tab == tb).on_hover_text(tip(tb.label(), Action::Tab(tb))).clicked() { select_tab(kit, studio, tb); }
        }
        // Remaining legacy pages (Creator, Corrections, Patterns, Notes, HUD Layout, Engine) under "More".
        let more_on = legacy_index(kit.tab) != Some(studio.tab) && kit.tab != Tab::Events;
        egui::ComboBox::from_id_salt("kit_more").selected_text(if more_on { legacy[studio.tab.min(legacy.len() - 1)] } else { "More" }).show_ui(ui, |ui| {
            for (i, name) in legacy.iter().enumerate() {
                if TABS.iter().any(|t| legacy_index(*t) == Some(i)) { continue; }
                if ui.selectable_label(more_on && studio.tab == i, *name).clicked() { studio.tab = i; }
            }
        });
    });
}
// A "More" page is open when studio.tab is not the legacy page of kit.tab (sync_from_legacy leaves kit.tab alone).


/// Left rail: rig + searchable bone tree. Selection is `studio.selected` (bone name), shared with the
/// viewport pick and the Pose tab; a click also logs Edit::Select. Selected row = selection #ffbe61.
pub fn bone_tree(ui: &mut egui::Ui, kit: &mut Kit, studio: &mut crate::studio::Studio, tick: Tick) {
    t::section(ui, "Rig");
    t::stat(ui, "Model", studio.model.as_deref().unwrap_or("none").rsplit('/').next().unwrap_or(""), t::PAPER);
    ui.add(egui::TextEdit::singleline(&mut kit.bone_query).hint_text("search bones"));
    let rows = std::mem::take(&mut kit.bones);
    let mut clicked = None;
    egui::ScrollArea::vertical().id_salt("kit_bones").max_height(ui.available_height() * 0.45).show(ui, |ui| {
        for i in filter_tree(&rows, &kit.bone_query) {
            let r = &rows[i];
            ui.horizontal(|ui| {
                ui.add_space(r.depth as f32 * 10.0);
                ui.label(egui::RichText::new("●").size(9.0).color(t::DRIVER_COLORS[r.driver.min(3)]));
                let on = studio.selected.as_deref() == Some(r.name.as_str());
                let resp = ui.selectable_label(on, egui::RichText::new(&r.name).size(12.0).color(if on { t::SELECTION } else { t::PAPER }));
                if on { t::paint_glow(ui.painter(), resp.rect, 3.0, 0.4); }
                if resp.clicked() { clicked = Some((r.bone, r.name.clone())); }
            });
        }
    });
    if let Some((b, name)) = clicked { studio.selected = Some(name); kit.log.push(tick, Edit::Select { bone: Some(b) }); }
    if let Some(sel) = studio.selected.clone() && let Some(r) = rows.iter().find(|r| r.name == sel) { breadcrumb(ui, &rows, r.bone); }
    kit.bones = rows;
}

/// Breadcrumb for the selected bone: ancestors joined with " › ".
pub fn breadcrumb(ui: &mut egui::Ui, rows: &[BoneRow], bone: u16) {
    let Some(i) = rows.iter().position(|r| r.bone == bone) else { return };
    let mut chain = vec![i];
    let mut d = rows[i].depth;
    for j in (0..i).rev() { if rows[j].depth < d { chain.push(j); d = rows[j].depth; } }
    chain.reverse();
    ui.horizontal_wrapped(|ui| {
        for (n, &j) in chain.iter().enumerate() {
            if n > 0 { ui.label(egui::RichText::new("›").color(t::MUTED)); }
            let last = n + 1 == chain.len();
            ui.label(egui::RichText::new(&rows[j].name).size(11.5).color(if last { t::SELECTION } else { t::MUTED }));
        }
    });
}

/// Slim top toolbar over the 3D view.
pub fn toolbar(ui: &mut egui::Ui, kit: &mut Kit, s: &Status, sim_paused: &mut bool) {
    ui.horizontal(|ui| {
        if t::button(ui, if s.paused { "▶ Play" } else { "⏸ Pause" }, s.paused).on_hover_text(tip("Pause / play the sim", Action::Pause)).clicked() { *sim_paused = !*sim_paused; }
        if t::button(ui, "⏭ Step", false).on_hover_text(tip("Advance one frame", Action::Step)).clicked() { kit.step_request = true; }
        ui.separator();
        if t::button(ui, "Ragdoll", s.ragdoll).on_hover_text(tip("Toggle ragdoll", Action::ToggleRagdoll)).clicked() {
            kit.ragdoll_req = Some(!s.ragdoll);
        }
        if t::button(ui, "Balance", s.balance).on_hover_text(tip("Toggle balance (restarts the 0.5 s ramp)", Action::ToggleBalance)).clicked() {
            kit.balance_req = Some(!s.balance);
        }
        ui.separator();
        // Buttons undo/redo the UI log; toggles are re-applied through requests (no new edit row).
        // TODO(verify): add undo_req/redo_req if button-undo must also revert ragdoll/balance immediately.
        if t::button(ui, "↶", false).on_hover_text(tip("Undo", Action::Undo)).clicked() { kit.log.undo(); }
        if t::button(ui, "↷", false).on_hover_text(tip("Redo", Action::Redo)).clicked() { kit.log.redo(); }
    });
}

/// Bottom status strip.
pub fn status_strip(ui: &mut egui::Ui, s: &Status) {
    ui.horizontal(|ui| {
        let onoff = |b: bool| if b { "on" } else { "off" };
        t::chip(ui, &format!("Ragdoll {}", onoff(s.ragdoll)), if s.ragdoll { t::HP } else { t::MUTED });
        t::chip(ui, &format!("Balance {}", onoff(s.balance)), if s.balance { t::GUARD } else { t::MUTED });
        t::chip(ui, &format!("Input {}", if s.input_lock { "locked" } else { "free" }), if s.input_lock { t::DANGER } else { t::MUTED });
        ui.label(egui::RichText::new(format!("tick {}  {:.0} fps", s.tick, s.fps)).font(t::number_font(11.5)));
        let mc = match margin_class(s.margin) { MarginClass::Good => t::GOOD, MarginClass::Warn => t::SELECTION, MarginClass::Bad => t::DANGER };
        t::chip(ui, &format!("margin {:+.1} cm", s.margin * 100.0), mc).on_hover_text("Stability margin: green > 3 cm, amber 0-3 cm, red < 0");
        ui.label(egui::RichText::new("peak sat").size(11.0).color(t::MUTED));
        let col = if s.peak_sat > 0.9 { t::DANGER } else if s.peak_sat > 0.7 { t::SELECTION } else { t::CYAN };
        let bar = ui.add(egui::ProgressBar::new(s.peak_sat.clamp(0.0, 1.0)).desired_width(110.0).fill(col).text(egui::RichText::new(format!("{:.0}%", s.peak_sat * 100.0)).font(t::number_font(10.5))));
        if let Some(b) = s.peak_bone { bar.on_hover_text(format!("bone {b}")); }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| t::reduced_motion_toggle(ui));
    });
}

/// Events tab: draggable timeline over the edit log + scrolling list with click-to-jump.
pub fn events_tab(ui: &mut egui::Ui, kit: &mut Kit, self_check: Option<&mut dyn FnMut() -> bool>) {
    t::panel(ui, "Event timeline", |ui| {
        let n = kit.log.rows.len();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click_and_drag());
        t::paint_chamfer(ui.painter(), rect, 4.0, t::HUD_BG, t::BORDER);
        let x_of = |i: usize| rect.left() + 8.0 + (rect.width() - 16.0) * (i as f32 / n.max(1) as f32);
        for (i, (_, e)) in kit.log.rows.iter().enumerate() {
            let col = match e { Edit::RagdollSet { .. } => t::HP, Edit::BalanceSet { .. } => t::GUARD, Edit::Select { .. } => t::SELECTION, _ => t::CYAN };
            let a = if i < kit.log.cursor { 1.0 } else { 0.35 };
            ui.painter().line_segment([egui::pos2(x_of(i + 1), rect.top() + 8.0), egui::pos2(x_of(i + 1), rect.bottom() - 8.0)], egui::Stroke::new(2.0, col.gamma_multiply(a)));
        }
        let cx = x_of(kit.log.cursor);
        ui.painter().line_segment([egui::pos2(cx, rect.top() + 2.0), egui::pos2(cx, rect.bottom() - 2.0)], egui::Stroke::new(2.0, t::SELECTION));
        if (resp.dragged() || resp.clicked()) && let Some(p) = resp.interact_pointer_pos() {
            let f = ((p.x - rect.left() - 8.0) / (rect.width() - 16.0)).clamp(0.0, 1.0);
            kit.log.seek((f * n as f32).round() as usize);
        }
        resp.on_hover_text("Drag to scrub the edit history (Ctrl+Z / Ctrl+Y step)");
        ui.horizontal(|ui| {
            t::stat(ui, "Applied", format!("{} / {}", kit.log.cursor, n), t::PAPER);
        });
        if let Some(check) = self_check {
            if t::button(ui, "Self-check", false).on_hover_text("Replay the log twice and compare hashes").clicked() {
                let ok = check();
                t::badge(ui, if ok { "replay OK" } else { "replay MISMATCH" }, if ok { t::GOOD } else { t::DANGER });
            }
        }
    });
    t::panel(ui, "Events", |ui| {
        let mut jump = None;
        egui::ScrollArea::vertical().id_salt("kit_events").stick_to_bottom(true).show(ui, |ui| {
            for (i, (tk, e)) in kit.log.rows.iter().enumerate() {
                let on = i < kit.log.cursor;
                let txt = egui::RichText::new(format!("{:>6}  {}", tk.0, describe(e))).font(t::number_font(11.5)).color(if on { t::PAPER } else { t::MUTED });
                if ui.selectable_label(i + 1 == kit.log.cursor, txt).clicked() { jump = Some(i + 1); }
            }
        });
        if let Some(j) = jump { kit.log.seek(j); }
    });
}


/// Status shared by the strip and the view bar (one place builds it).
fn status_now(kit: &Kit, shown: &crate::sandbox::SandboxShown, sim: &crate::sim_bridge::Sim, agent: &crate::agent::Agent, body: &crate::possession::PhysicalBody, diag: &bevy::diagnostic::DiagnosticsStore) -> Status {
    let fps = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let mut s = Status::from_shown(shown, sim.paused, agent.lock_input, fps);
    s.tick = kit.fixed_tick; // Avian runs in FixedUpdate at 120 Hz: show the Fixed tick, fps separately
    s.ragdoll = body.on;
    s
}

/// Bottom status strip (full width). Runs before studio::ui.
/// TODO(verify): egui 0.40 ctx-level `Panel::show(ctx, ..)` name (studio.rs uses `show_inside` on a root Ui).
pub fn status_strip_ui(
    mut contexts: bevy_egui::EguiContexts,
    studio: Res<crate::studio::Studio>,
    kit: Res<Kit>,
    shown: Res<crate::sandbox::SandboxShown>,
    sim: Res<crate::sim_bridge::Sim>,
    agent: Res<crate::agent::Agent>,
    body: Res<crate::possession::PhysicalBody>,
    diag: Res<bevy::diagnostic::DiagnosticsStore>,
) -> Result {
    if !studio.open { return Ok(()); }
    let ctx = contexts.ctx_mut()?.clone();
    let s = status_now(&kit, &shown, &sim, &agent, &body, &diag);
    egui::Panel::bottom("studio_status").frame(t::dock().inner_margin(egui::Margin::symmetric(12, 4))).show(&ctx, |ui| status_strip(ui, &s));
    Ok(())
}

/// Slim view toolbar: pause/step, ragdoll, balance, undo/redo, trails toggle. Runs before studio::ui,
/// so the panel is added before the central content (fixes the late `studio_view_bar`).
#[allow(clippy::too_many_arguments)]
pub fn toolbar_ui(
    mut contexts: bevy_egui::EguiContexts,
    studio: Res<crate::studio::Studio>,
    mut kit: ResMut<Kit>,
    mut trails: ResMut<crate::studio_trails::Trails>,
    shown: Res<crate::sandbox::SandboxShown>,
    mut sim: ResMut<crate::sim_bridge::Sim>,
    agent: Res<crate::agent::Agent>,
    body: Res<crate::possession::PhysicalBody>,
    diag: Res<bevy::diagnostic::DiagnosticsStore>,
) -> Result {
    if !studio.open { return Ok(()); }
    let ctx = contexts.ctx_mut()?.clone();
    let s = status_now(&kit, &shown, &sim, &agent, &body, &diag);
    egui::Panel::top("studio_view_bar").frame(egui::Frame::new().fill(t::GLASS).inner_margin(egui::Margin::symmetric(8, 3))).show(&ctx, |ui| {
        ui.horizontal(|ui| {
            toolbar(ui, &mut kit, &s, &mut sim.paused);
            crate::studio_trails::toolbar_toggle(ui, &mut trails);
        });
    });
    Ok(())
}

/// Left rail: rig + bone tree (was inside studio.rs's asset panel). Runs before studio::ui.
pub fn bone_tree_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut studio: ResMut<crate::studio::Studio>,
    mut kit: ResMut<Kit>,
    sim: Res<crate::sim_bridge::Sim>,
) -> Result {
    if !studio.open { return Ok(()); }
    let ctx = contexts.ctx_mut()?.clone();
    let tick = Tick(sim.world.tick);
    egui::Panel::left("studio_bones").default_size(220.0).resizable(true).frame(t::dock()).show(&ctx, |ui| bone_tree(ui, &mut kit, &mut studio, tick));
    Ok(())
}
