// ui_theme.hud.rs - HUD Layer Lab theme. STAGED, UNCOMPILED (egui not available on the box build).
// Replaces ui_theme.rs.append.rs: append THIS to crates/game/src/ui_theme.rs instead of that file.
// Extends: crates/game/src/ui_theme.rs (keeps every existing public name).
// Rule: no UiHoverSet anywhere (it overflows the stack). Hover uses ctx.animate_bool only.
//
// TODO(verify): existing ui_theme.rs already defines TEAL, PAPER, card(), button(), section(), stat().
// ACCENT/SPACE/FONT/DRIVER_COLORS below are defined only here (they came from the old append).
// If ui_theme.rs's own TEAL/PAPER/card() should take the HUD look too, change their bodies in place
// to use HUD_* below; their names and signatures stay the same.
// TODO(verify): egui version in Cargo.lock (bevy_egui for Bevy 0.19). Names used: Shape::convex_polygon,
// Shape::closed_line, FontData::from_owned, FontDefinitions, ctx.animate_bool_with_time.

use std::sync::atomic::{AtomicBool, Ordering};

const fn hex(rgb: u32) -> egui::Color32 {
    egui::Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

// ---- Palette
pub const HUD_BG: egui::Color32 = hex(0x081326);
pub const HUD_CARD: egui::Color32 = hex(0x102139);
pub const HUD_CARD_BORDER: egui::Color32 = hex(0x294863);
pub const HUD_TEXT: egui::Color32 = hex(0xeaf6ff);
pub const HUD_BUTTON: egui::Color32 = hex(0x102d4e);
pub const HUD_BUTTON_BORDER: egui::Color32 = hex(0x579fbb);
/// rgba(6,18,39,.70)
pub const HUD_PANEL: egui::Color32 = egui::Color32::from_rgba_premultiplied(4, 13, 27, 179);
pub const HP: egui::Color32 = hex(0xec6474);
pub const GUARD: egui::Color32 = hex(0x72d9ef);
pub const SELECTION: egui::Color32 = hex(0xffbe61);
pub const DANGER: egui::Color32 = hex(0xff385a);
pub const ACCENT: egui::Color32 = hex(0x5fd7ff);
pub const CHAMFER: f32 = 10.0;
pub const SPACE: [f32; 3] = [4.0, 8.0, 14.0];
pub const FONT: [f32; 3] = [11.5, 12.5, 14.0];
/// Minimum text opacity.
pub const TEXT_MIN_ALPHA: f32 = 0.85;

/// [animated, controller, IK, motor]. Distinct and tied to the meaning palette:
/// animated = muted text grey-blue, controller = selection amber (it is what you are editing),
/// IK = guard cyan, motor = HP rose (force/health). Danger red is kept only for warnings.
pub const DRIVER_COLORS: [egui::Color32; 4] = [hex(0x8fa6bd), SELECTION, GUARD, HP];

/// Text colour with the given opacity, clamped so it is never below 0.85.
pub fn text_alpha(alpha: f32) -> egui::Color32 {
    let a = alpha.clamp(TEXT_MIN_ALPHA, 1.0);
    egui::Color32::from_rgba_unmultiplied(HUD_TEXT.r(), HUD_TEXT.g(), HUD_TEXT.b(), (a * 255.0) as u8)
}

// ---- Reduced motion
static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);
pub fn reduced_motion() -> bool { REDUCED_MOTION.load(Ordering::Relaxed) }
pub fn set_reduced_motion(on: bool) { REDUCED_MOTION.store(on, Ordering::Relaxed) }
pub fn reduced_motion_toggle(ui: &mut egui::Ui) {
    let mut on = reduced_motion();
    if ui.checkbox(&mut on, egui::RichText::new("Reduced motion").color(HUD_TEXT)).changed() { set_reduced_motion(on); }
}
/// 0..1 hover amount; snaps instantly with reduced motion.
fn hover_t(ui: &egui::Ui, id: egui::Id, hovered: bool) -> f32 {
    if reduced_motion() { hovered as u8 as f32 } else { ui.ctx().animate_bool_with_time(id, hovered, 0.12) }
}

// ---- Fonts (call once, e.g. from the egui context setup system)
/// Loads Segoe UI (body), Georgia (titles, family "title"), Consolas (numbers, Monospace) from
/// C:/Windows/Fonts. Any font that fails to read is skipped, leaving egui's default in place.
pub fn install_hud_fonts(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let mut load = |key: &str, file: &str, family: egui::FontFamily| {
        if let Ok(bytes) = std::fs::read(format!("C:/Windows/Fonts/{file}")) {
            // TODO(verify): newer egui wants Arc<FontData>: .insert(key, std::sync::Arc::new(...))
            defs.font_data.insert(key.to_owned(), egui::FontData::from_owned(bytes).into());
            defs.families.entry(family).or_default().insert(0, key.to_owned());
            true
        } else { false }
    };
    load("segoe", "segoeui.ttf", egui::FontFamily::Proportional);
    load("consolas", "consola.ttf", egui::FontFamily::Monospace);
    let title = egui::FontFamily::Name("title".into());
    if !load("georgia", "georgia.ttf", title.clone()) {
        let fallback = defs.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
        defs.families.insert(title.clone(), fallback);
    } else {
        let rest = defs.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
        defs.families.get_mut(&title).unwrap().extend(rest);
    }
    ctx.set_fonts(defs);
    ctx.style_mut(|s| {
        s.visuals.panel_fill = HUD_BG;
        s.visuals.window_fill = HUD_PANEL;
        s.visuals.override_text_color = Some(HUD_TEXT);
        s.visuals.selection.bg_fill = SELECTION.linear_multiply(0.35);
        s.visuals.selection.stroke = egui::Stroke::new(1.0, SELECTION);
        s.visuals.widgets.inactive.bg_fill = HUD_BUTTON;
        s.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, HUD_BUTTON_BORDER);
        s.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    });
}
pub fn title_font(size: f32) -> egui::FontId { egui::FontId::new(size, egui::FontFamily::Name("title".into())) }
pub fn number_font(size: f32) -> egui::FontId { egui::FontId::monospace(size) }

// ---- Chamfer painter
/// Octagon with `c` px cut corners.
pub fn chamfer_points(r: egui::Rect, c: f32) -> Vec<egui::Pos2> {
    let c = c.min(r.width() * 0.5).min(r.height() * 0.5);
    let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
    [(l + c, t), (rr - c, t), (rr, t + c), (rr, b - c), (rr - c, b), (l + c, b), (l, b - c), (l, t + c)]
        .into_iter().map(|(x, y)| egui::pos2(x, y)).collect()
}
/// Fill + border + soft cyan glow (3 expanding low-alpha strokes). Glow pulses unless reduced motion.
pub fn paint_chamfer(p: &egui::Painter, r: egui::Rect, fill: egui::Color32, border: egui::Color32, glow: f32) {
    let pts = chamfer_points(r, CHAMFER);
    p.add(egui::Shape::convex_polygon(pts.clone(), fill, egui::Stroke::new(1.0, border)));
    if glow <= 0.0 { return; }
    let pulse = if reduced_motion() { 1.0 } else {
        let t = p.ctx().input(|i| i.time) as f32;
        p.ctx().request_repaint();
        0.8 + 0.2 * (t * 2.0).sin()
    };
    for (i, a) in [(1.0f32, 0.22f32), (2.5, 0.12), (4.0, 0.06)] {
        let col = egui::Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), (a * glow * pulse * 255.0) as u8);
        p.add(egui::Shape::closed_line(chamfer_points(r.expand(i), CHAMFER + i * 0.4), egui::Stroke::new(1.5, col)));
    }
}

// ---- Shared panel API (names unchanged)
pub fn panel<R>(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.add_space(SPACE[1]);
    let where_ = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::NONE.inner_margin(SPACE[2]).show(ui, |ui| { // TODO(verify): Frame::NONE vs Frame::none()
        ui.label(egui::RichText::new(title).font(title_font(FONT[2] + 1.0)).color(HUD_TEXT));
        ui.add_space(SPACE[0]);
        body(ui)
    });
    // Paint the background under the content after layout so the rect is known.
    let r = inner.response.rect;
    let mut shapes = Vec::new();
    let tmp = ui.painter().clone();
    let pts = chamfer_points(r, CHAMFER);
    shapes.push(egui::Shape::convex_polygon(pts, HUD_PANEL, egui::Stroke::new(1.0, HUD_CARD_BORDER)));
    ui.painter().set(where_, egui::Shape::Vec(shapes));
    // Glow drawn on top as thin strokes.
    for (i, a) in [(1.0f32, 0.18f32), (2.5, 0.09), (4.0, 0.04)] {
        let col = egui::Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), (a * 255.0) as u8);
        tmp.add(egui::Shape::closed_line(chamfer_points(r.expand(i), CHAMFER + i * 0.4), egui::Stroke::new(1.5, col)));
    }
    inner.inner
}

/// Chamfered HUD button; `selected` uses the selection amber border.
pub fn hud_button(ui: &mut egui::Ui, label: &str, selected: bool, accent: egui::Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_owned(), egui::FontId::proportional(FONT[1]), HUD_TEXT);
    let size = galley.size() + egui::vec2(SPACE[2] * 1.4, SPACE[1] * 1.2);
    let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let h = hover_t(ui, resp.id, resp.hovered());
    let border = if selected { SELECTION } else { lerp_col(HUD_BUTTON_BORDER, accent, h) };
    paint_chamfer(ui.painter(), r, HUD_BUTTON, border, if selected { 0.6 } else { h * 0.8 });
    ui.painter().galley(r.center() - galley.size() * 0.5, galley, HUD_TEXT);
    resp
}
pub fn icon_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    hud_button(ui, &format!("{icon}  {label}"), selected, ACCENT)
}
pub fn danger_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    hud_button(ui, &format!("{icon}  {label}"), false, DANGER)
}
pub fn driver_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (i, n) in ["animated", "controller", "IK", "motor"].iter().enumerate() {
            ui.label(egui::RichText::new(format!("● {n}")).size(FONT[0]).color(DRIVER_COLORS[i]));
        }
    });
}
/// Number readout in Consolas.
pub fn num(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(FONT[0]).color(text_alpha(0.85)));
        ui.label(egui::RichText::new(value).font(number_font(FONT[1])).color(HUD_TEXT));
    });
}
fn lerp_col(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    egui::Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

// ---- Driver colours for gizmos (names unchanged)
pub fn driver_index(state: &str) -> usize {
    match state { "controller" => 1, "IK" => 2, s if s.contains("motor") => 3, _ => 0 }
}
pub fn driver_gizmo(state: &str) -> bevy::prelude::Color {
    let c = DRIVER_COLORS[driver_index(state)];
    bevy::prelude::Color::srgb_u8(c.r(), c.g(), c.b())
}
pub fn hover_gizmo() -> bevy::prelude::Color { bevy::prelude::Color::srgb_u8(0x5f, 0xd7, 0xff) }
pub fn selection_gizmo() -> bevy::prelude::Color { bevy::prelude::Color::srgb_u8(0xff, 0xbe, 0x61) }

// ---- Cancel / snapshot
/// Snapshot of editable values taken when a panel opens. Cancel or Esc restores and closes.
#[derive(Default)]
pub struct Snapshot<T: Clone> { saved: Option<T> }
impl<T: Clone> Snapshot<T> {
    pub fn begin(&mut self, v: &T) { if self.saved.is_none() { self.saved = Some(v.clone()); } }
    pub fn commit(&mut self) { self.saved = None; }
    pub fn cancel(&mut self, v: &mut T) { if let Some(s) = self.saved.take() { *v = s; } }
    pub fn is_open(&self) -> bool { self.saved.is_some() }
}
/// Apply / Cancel row. Esc counts as Cancel and closes. Returns true when the panel should close.
pub fn apply_cancel<T: Clone>(ui: &mut egui::Ui, snap: &mut Snapshot<T>, value: &mut T) -> bool {
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let mut close = false;
    ui.horizontal(|ui| {
        if icon_button(ui, "✔", "Apply", false).clicked() { snap.commit(); close = true; }
        if hud_button(ui, "✖  Cancel", false, DANGER).clicked() || esc { snap.cancel(value); close = true; }
    });
    close
}

// ---- Studio tabs (chamfered, fixed order)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StudioTab { #[default] Pose, Physics, Animation, Events, Controller }
pub const STUDIO_TABS: [(StudioTab, &str, &str); 5] = [
    (StudioTab::Pose, "🦴", "Pose"),
    (StudioTab::Physics, "⚖", "Physics"),
    (StudioTab::Animation, "🎞", "Animation"),
    (StudioTab::Events, "📜", "Events"),
    (StudioTab::Controller, "🎮", "Controller"),
];
/// Left tab bar. Number keys 1-5 jump; Tab / arrow keys step. TODO(verify): merge with studio.rs's
/// existing tab enum if it has one (keep this order).
pub fn studio_tabs(ui: &mut egui::Ui, tab: &mut StudioTab) {
    let idx = STUDIO_TABS.iter().position(|t| t.0 == *tab).unwrap_or(0);
    let (jump, next, prev) = ui.input(|i| {
        let keys = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5];
        (keys.iter().position(|k| i.key_pressed(*k)),
         i.key_pressed(egui::Key::ArrowDown) || (i.key_pressed(egui::Key::Tab) && !i.modifiers.shift),
         i.key_pressed(egui::Key::ArrowUp) || (i.key_pressed(egui::Key::Tab) && i.modifiers.shift))
    });
    let n = STUDIO_TABS.len();
    let new = jump.unwrap_or(if next { (idx + 1) % n } else if prev { (idx + n - 1) % n } else { idx });
    *tab = STUDIO_TABS[new].0;
    ui.vertical(|ui| {
        for (t, icon, name) in STUDIO_TABS {
            if icon_button(ui, icon, name, *tab == t).clicked() { *tab = t; }
            ui.add_space(SPACE[0]);
        }
        ui.add_space(SPACE[2]);
        reduced_motion_toggle(ui);
    });
}
