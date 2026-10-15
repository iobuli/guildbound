//! One UI system for every screen (HUD, Studio, dev panel): the HUD Layer Lab look daviy approved.
//! Navy #081326 surfaces, #102139 cards with a #294863 border, chamfered translucent panels
//! (rgba(6,18,39,.70), 10 px cuts) with a soft #5fd7ff glow, #102d4e buttons with a #579fbb border.
//! Fonts: Segoe UI body, Georgia titles, Consolas numbers (C:/Windows/Fonts, egui defaults as fallback).
//! Meaning colours: HP #ec6474, guard #72d9ef, selection #ffbe61, danger #ff385a.
//! Rules: text opacity never below 0.85; reduced-motion toggle; never use UiHoverSet (it overflows the stack).
//!
//! FULL REPLACEMENT of crates/game/src/ui_theme.rs. Every previous public name is kept with the same
//! signature (TEAL, TEAL_DIM, GLASS, CARD, BORDER, PAPER, MUTED, WARN, BAD, GOOD, style, dock, card,
//! section, hint, stat, button, tabs, badge, ACCENT, SPACE, FONT, DRIVER_COLORS, driver_index,
//! driver_gizmo, hover_gizmo, panel, icon_button, danger_button, driver_legend); the colour names now
//! hold HUD values so every screen restyles without edits.
//! Written against the box copy /workspace/repo/ui_theme.rs (egui with Panel::top, Frame::new,
//! CornerRadius, StrokeKind). TODO(verify): diff against the post-edit ui_theme.rs once it arrives.
use bevy_egui::egui;
use std::sync::atomic::{AtomicBool, Ordering};

const fn hex(rgb: u32) -> egui::Color32 {
    egui::Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

// ---- HUD palette
pub const HUD_BG: egui::Color32 = hex(0x081326);
pub const HUD_CARD: egui::Color32 = hex(0x102139);
pub const HUD_CARD_BORDER: egui::Color32 = hex(0x294863);
pub const HUD_TEXT: egui::Color32 = hex(0xeaf6ff);
pub const HUD_BUTTON: egui::Color32 = hex(0x102d4e);
pub const HUD_BUTTON_BORDER: egui::Color32 = hex(0x579fbb);
/// rgba(6,18,39,.70), premultiplied.
pub const HUD_PANEL: egui::Color32 = egui::Color32::from_rgba_premultiplied(4, 13, 27, 179);
pub const HP: egui::Color32 = hex(0xec6474);
pub const GUARD: egui::Color32 = hex(0x72d9ef);
pub const SELECTION: egui::Color32 = hex(0xffbe61);
pub const DANGER: egui::Color32 = hex(0xff385a);
pub const CYAN: egui::Color32 = hex(0x5fd7ff);
pub const CHAMFER: f32 = 10.0;
pub const TEXT_MIN_ALPHA: f32 = 0.85;

// ---- Legacy names, HUD values
pub const TEAL: egui::Color32 = CYAN;
pub const TEAL_DIM: egui::Color32 = hex(0x123a55);
pub const GLASS: egui::Color32 = HUD_PANEL;
pub const CARD: egui::Color32 = HUD_CARD;
pub const BORDER: egui::Color32 = HUD_CARD_BORDER;
pub const PAPER: egui::Color32 = HUD_TEXT;
/// Secondary text: opaque light slate, well above the 0.85 opacity rule (it is a colour, not alpha).
pub const MUTED: egui::Color32 = hex(0xb4cadb);
pub const WARN: egui::Color32 = SELECTION;
pub const BAD: egui::Color32 = DANGER;
pub const GOOD: egui::Color32 = hex(0x6fe3a0);

pub const ACCENT: egui::Color32 = CYAN;
pub const SPACE: [f32; 3] = [4.0, 8.0, 14.0];
pub const FONT: [f32; 3] = [11.5, 12.5, 14.0];
/// [animated, controller, IK, motor]: slate, selection amber, guard cyan, HP rose. Danger red stays for warnings.
pub const DRIVER_COLORS: [egui::Color32; 4] = [hex(0x8fa6bd), SELECTION, GUARD, HP];

/// Text with opacity clamped to >= 0.85.
pub fn text_alpha(alpha: f32) -> egui::Color32 {
    let a = alpha.clamp(TEXT_MIN_ALPHA, 1.0);
    egui::Color32::from_rgba_unmultiplied(HUD_TEXT.r(), HUD_TEXT.g(), HUD_TEXT.b(), (a * 255.0).round() as u8)
}

// ---- Reduced motion
static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);
pub fn reduced_motion() -> bool { REDUCED_MOTION.load(Ordering::Relaxed) }
pub fn set_reduced_motion(on: bool) { REDUCED_MOTION.store(on, Ordering::Relaxed) }
pub fn reduced_motion_toggle(ui: &mut egui::Ui) {
    let mut on = reduced_motion();
    if checkbox(ui, &mut on, "Reduced motion").on_hover_text("No hover animation or glow pulse").changed() { set_reduced_motion(on); }
}
fn hover_t(ui: &egui::Ui, id: egui::Id, hovered: bool) -> f32 {
    if reduced_motion() { if hovered { 1.0 } else { 0.0 } } else { ui.ctx().animate_bool_with_time(id, hovered, 0.12) }
}

// ---- Fonts
static FONTS_DONE: AtomicBool = AtomicBool::new(false);
/// Segoe UI (Proportional), Consolas (Monospace), Georgia (family "title") from C:/Windows/Fonts.
/// Missing files are skipped; egui defaults stay underneath as fallback. Runs once.
pub fn install_fonts(ctx: &egui::Context) {
    if FONTS_DONE.swap(true, Ordering::Relaxed) { return; }
    let mut defs = egui::FontDefinitions::default();
    let mut add = |key: &str, file: &str, family: egui::FontFamily| -> bool {
        let Ok(bytes) = std::fs::read(format!("C:/Windows/Fonts/{file}")) else { return false };
        // TODO(verify): font_data value type (Arc<FontData> in recent egui).
        defs.font_data.insert(key.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        defs.families.entry(family).or_default().insert(0, key.to_owned());
        true
    };
    add("segoe", "segoeui.ttf", egui::FontFamily::Proportional);
    add("consolas", "consola.ttf", egui::FontFamily::Monospace);
    let title = egui::FontFamily::Name("title".into());
    let body = defs.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    add("georgia", "georgia.ttf", title.clone());
    defs.families.entry(title).or_default().extend(body);
    ctx.set_fonts(defs);
}
pub fn title_font(size: f32) -> egui::FontId { egui::FontId::new(size, egui::FontFamily::Name("title".into())) }
pub fn number_font(size: f32) -> egui::FontId { egui::FontId::monospace(size) }
pub fn body_font(size: f32) -> egui::FontId { egui::FontId::proportional(size) }

// ---- Global style (same name as before)
pub fn style(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut v = egui::Visuals::dark();
    let r = egui::CornerRadius::same(2);
    v.override_text_color = Some(HUD_TEXT);
    v.panel_fill = GLASS;
    v.window_fill = GLASS;
    v.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
    v.window_corner_radius = egui::CornerRadius::same(2);
    v.window_shadow = egui::Shadow { offset: [0, 0], blur: 16, spread: 0, color: egui::Color32::from_rgba_unmultiplied(95, 215, 255, 28) };
    v.popup_shadow = v.window_shadow;
    v.menu_corner_radius = r;
    v.selection.bg_fill = SELECTION.gamma_multiply(0.30);
    v.selection.stroke = egui::Stroke::new(1.0_f32, SELECTION);
    v.hyperlink_color = CYAN;
    v.faint_bg_color = CARD;
    v.extreme_bg_color = HUD_BG; // text edits, slider rails
    v.code_bg_color = CARD;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
        w.corner_radius = r;
        w.expansion = 0.0;
        w.fg_stroke = egui::Stroke::new(1.0_f32, HUD_TEXT);
    }
    v.widgets.noninteractive.bg_fill = CARD;
    v.widgets.noninteractive.weak_bg_fill = CARD;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    v.widgets.inactive.bg_fill = HUD_BUTTON; // slider handle, checkbox box
    v.widgets.inactive.weak_bg_fill = HUD_BUTTON;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, HUD_BUTTON_BORDER);
    v.widgets.hovered.bg_fill = hex(0x15406a);
    v.widgets.hovered.weak_bg_fill = hex(0x15406a);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, CYAN);
    v.widgets.active.bg_fill = hex(0x1a4f80);
    v.widgets.active.weak_bg_fill = hex(0x1a4f80);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.5_f32, CYAN);
    v.widgets.open = v.widgets.active;
    v.slider_trailing_fill = true; // trailing fill uses selection.bg_fill
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.slider_rail_height = 4.0;
        s.text_styles.insert(egui::TextStyle::Body, body_font(12.5));
        s.text_styles.insert(egui::TextStyle::Button, body_font(12.5));
        s.text_styles.insert(egui::TextStyle::Small, body_font(11.0));
        s.text_styles.insert(egui::TextStyle::Monospace, number_font(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, title_font(16.0));
        if reduced_motion() { s.animation_time = 0.0; }
    });
}

// ---- Chamfer painter
pub fn chamfer_points(r: egui::Rect, c: f32) -> Vec<egui::Pos2> {
    let c = c.min(r.width() * 0.5).min(r.height() * 0.5);
    let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
    [(l + c, t), (rr - c, t), (rr, t + c), (rr, b - c), (rr - c, b), (l + c, b), (l, b - c), (l, t + c)]
        .into_iter().map(|(x, y)| egui::pos2(x, y)).collect()
}
/// Soft cyan glow: three expanding low-alpha strokes; pulses unless reduced motion. `strength` 0..1.
pub fn paint_glow(p: &egui::Painter, r: egui::Rect, c: f32, strength: f32) {
    if strength <= 0.0 { return; }
    let pulse = if reduced_motion() { 1.0 } else { 0.85 + 0.15 * ((p.ctx().input(|i| i.time) as f32) * 2.0).sin() };
    for (grow, a) in [(1.0f32, 0.20f32), (2.5, 0.10), (4.0, 0.05)] {
        let col = egui::Color32::from_rgba_unmultiplied(CYAN.r(), CYAN.g(), CYAN.b(), (a * strength * pulse * 255.0) as u8);
        p.add(egui::Shape::closed_line(chamfer_points(r.expand(grow), c + grow * 0.4), egui::Stroke::new(1.5, col)));
    }
}
pub fn paint_chamfer(p: &egui::Painter, r: egui::Rect, c: f32, fill: egui::Color32, border: egui::Color32) {
    p.add(egui::Shape::convex_polygon(chamfer_points(r, c), fill, egui::Stroke::new(1.0, border)));
}
/// Chamfered translucent frame around `body`: background is painted underneath via a reserved shape slot.
pub fn chamfer_frame<R>(ui: &mut egui::Ui, fill: egui::Color32, border: egui::Color32, glow: f32, margin: f32, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let slot = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::new().inner_margin(margin).show(ui, body);
    let r = inner.response.rect;
    ui.painter().set(slot, egui::Shape::convex_polygon(chamfer_points(r, CHAMFER), fill, egui::Stroke::new(1.0, border)));
    paint_glow(ui.painter(), r, CHAMFER, glow);
    inner.inner
}

// ---- Frames (same names)
/// Edge dock (side panels, top/bottom strips). Plain rect fill so egui panels tile; glow lives on cards.
pub fn dock() -> egui::Frame {
    egui::Frame::new().fill(GLASS).stroke(egui::Stroke::new(1.0_f32, BORDER)).inner_margin(12.0)
}
/// Raised card. egui::Frame cannot cut corners, so chamfer_frame/panel are preferred for new code.
pub fn card() -> egui::Frame {
    egui::Frame::new().fill(CARD).stroke(egui::Stroke::new(1.0_f32, BORDER)).inner_margin(10.0).corner_radius(3.0)
}

// ---- Text pieces (same names)
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(SPACE[1]);
    ui.label(egui::RichText::new(title).font(title_font(13.5)).color(PAPER));
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.left()..=r.right(), r.top(), egui::Stroke::new(1.0_f32, BORDER));
    ui.painter().hline(r.left()..=(r.left() + 28.0).min(r.right()), r.top(), egui::Stroke::new(2.0_f32, CYAN));
    ui.add_space(SPACE[0] + 2.0);
}
pub fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(11.5).color(MUTED));
}
/// Label left, number right in Consolas.
pub fn stat(ui: &mut egui::Ui, label: &str, value: impl Into<String>, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value.into()).font(number_font(12.0)).color(color));
        });
    });
}
pub fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    chip(ui, text, color);
}
/// Chamfered pill with a tinted fill and full-opacity text.
pub fn chip(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.to_owned(), number_font(11.0), color);
    let (r, resp) = ui.allocate_exact_size(g.size() + egui::vec2(14.0, 6.0), egui::Sense::hover());
    paint_chamfer(ui.painter(), r, 4.0, color.gamma_multiply(0.18), color.gamma_multiply(0.75));
    ui.painter().galley(r.center() - g.size() * 0.5, g, color);
    resp
}

// ---- Controls
/// Chamfered HUD button. `selected` = selection amber border + glow. `accent` tints hover.
pub fn hud_button(ui: &mut egui::Ui, text: &str, selected: bool, accent: egui::Color32) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.to_owned(), body_font(FONT[1]), HUD_TEXT);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(g.size().x + 24.0, 28.0), egui::Sense::click());
    let h = hover_t(ui, resp.id, resp.hovered());
    let border = if selected { SELECTION } else { lerp(HUD_BUTTON_BORDER, accent, h) };
    let fill = if selected { hex(0x1a3a5c) } else { lerp(HUD_BUTTON, hex(0x15406a), h) };
    paint_chamfer(ui.painter(), rect, 6.0, fill, border);
    paint_glow(ui.painter(), rect, 6.0, if selected { 0.5 } else { h * 0.6 });
    ui.painter().galley(rect.center() - g.size() * 0.5, g, HUD_TEXT);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn button(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response { hud_button(ui, text, selected, CYAN) }
pub fn icon_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    hud_button(ui, &format!("{icon}  {label}"), selected, CYAN)
}
pub fn danger_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    hud_button(ui, &format!("{icon}  {label}"), false, DANGER)
}
/// Button with "label (key)" tooltip. Every Studio button should go through this or `.on_hover_text`.
pub fn key_button(ui: &mut egui::Ui, text: &str, selected: bool, tip: &str, key: &str) -> egui::Response {
    button(ui, text, selected).on_hover_text(format!("{tip}  [{key}]"))
}
/// Themed checkbox: chamfered box, cyan tick, selection border when on.
pub fn checkbox(ui: &mut egui::Ui, on: &mut bool, label: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(label.to_owned(), body_font(FONT[1]), HUD_TEXT);
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(22.0 + g.size().x, 22.0), egui::Sense::click());
    if resp.clicked() { *on = !*on; resp.mark_changed(); }
    let b = egui::Rect::from_min_size(rect.left_center() - egui::vec2(0.0, 8.0), egui::vec2(16.0, 16.0));
    let h = hover_t(ui, resp.id, resp.hovered());
    paint_chamfer(ui.painter(), b, 3.0, HUD_BUTTON, if *on { SELECTION } else { lerp(HUD_BUTTON_BORDER, CYAN, h) });
    if *on {
        let p = |x: f32, y: f32| egui::pos2(b.left() + x, b.top() + y);
        ui.painter().add(egui::Shape::line(vec![p(3.5, 8.5), p(7.0, 12.0), p(12.5, 4.5)], egui::Stroke::new(2.0, CYAN)));
    }
    ui.painter().galley(egui::pos2(b.right() + 6.0, rect.center().y - g.size().y * 0.5), g, HUD_TEXT);
    resp
}
/// Themed slider: style() already colours rails/handles; this adds a label and a Consolas value.
pub fn slider(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(MUTED));
        ui.add(egui::Slider::new(v, range).show_value(false));
        ui.label(egui::RichText::new(format!("{v:.3}")).font(number_font(12.0)));
    }).inner
}
/// Collapsed-by-default "advanced" section with a Georgia header.
pub fn advanced<R>(ui: &mut egui::Ui, id: &str, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    egui::CollapsingHeader::new(egui::RichText::new(title).font(title_font(12.5)).color(MUTED))
        .id_salt(id).default_open(false).show(ui, body).body_returned
}

/// Tab strip (same name/signature). Chamfered tabs; selected tab is selection amber.
pub fn tabs(ui: &mut egui::Ui, labels: &[&str], at: &mut usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, t) in labels.iter().enumerate() {
            if hud_button(ui, t, *at == i, CYAN).clicked() { *at = i; }
        }
    });
}

// ---- Shared panel API
pub fn driver_index(state: &str) -> usize {
    match state { "controller" => 1, "IK" => 2, s if s.contains("motor") => 3, _ => 0 }
}
pub fn driver_gizmo(state: &str) -> bevy::prelude::Color {
    let c = DRIVER_COLORS[driver_index(state)];
    bevy::prelude::Color::srgb_u8(c.r(), c.g(), c.b())
}
pub fn hover_gizmo() -> bevy::prelude::Color { bevy::prelude::Color::srgb_u8(0x5f, 0xd7, 0xff) }
pub fn selection_gizmo() -> bevy::prelude::Color { bevy::prelude::Color::srgb_u8(0xff, 0xbe, 0x61) }
/// Titled chamfered translucent panel with a cyan glow.
pub fn panel<R>(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.add_space(SPACE[1]);
    chamfer_frame(ui, HUD_PANEL, BORDER, 0.7, 12.0, |ui| {
        ui.label(egui::RichText::new(title).font(title_font(FONT[2] + 1.0)).color(PAPER));
        ui.add_space(SPACE[0]);
        body(ui)
    })
}
pub fn driver_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (i, n) in ["animated", "controller", "IK", "motor"].iter().enumerate() {
            ui.label(egui::RichText::new(format!("● {n}")).size(FONT[0]).color(DRIVER_COLORS[i]));
        }
    });
}

// ---- Cancel / snapshot
#[derive(Default)]
pub struct Snapshot<T: Clone> { saved: Option<T> }
impl<T: Clone> Snapshot<T> {
    pub fn begin(&mut self, v: &T) { if self.saved.is_none() { self.saved = Some(v.clone()); } }
    pub fn commit(&mut self) { self.saved = None; }
    pub fn cancel(&mut self, v: &mut T) { if let Some(s) = self.saved.take() { *v = s; } }
    pub fn is_open(&self) -> bool { self.saved.is_some() }
}
/// Apply / Cancel row; Esc = Cancel. Returns true when the panel should close.
pub fn apply_cancel<T: Clone>(ui: &mut egui::Ui, snap: &mut Snapshot<T>, value: &mut T) -> bool {
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let mut close = false;
    ui.horizontal(|ui| {
        if key_button(ui, "Apply", false, "Keep these values", "Enter").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) { snap.commit(); close = true; }
        if hud_button(ui, "Cancel", false, DANGER).on_hover_text("Restore the values from when this opened  [Esc]").clicked() || esc { snap.cancel(value); close = true; }
    });
    close
}

fn lerp(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    egui::Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}
