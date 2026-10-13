use bevy_egui::egui;
use guildbound_sim::rpg::{BAG_SIZE, BagAction, ITEMS, ItemId, ItemKind, Kit, SLOTS, Slot};
use crate::{sim_bridge::Sim, ui_theme as theme};

#[derive(Clone, Copy, PartialEq)]
enum Selection { Bag(usize, ItemId), Worn(Slot, ItemId) }

#[derive(Default)]
pub struct InventoryUi {
    pub tab: usize,
    selected: Option<Selection>,
    search: String,
    filter: usize,
}

impl InventoryUi {
    fn validate(&mut self, kit: &Kit) {
        let valid = match self.selected {
            Some(Selection::Bag(k, id)) => kit.bag.get(k) == Some(&id),
            Some(Selection::Worn(slot, id)) => kit.worn_in(slot) == Some(id),
            None => true,
        };
        if !valid { self.selected = None; }
    }
}

pub fn show(ctx: &egui::Context, sim: &Sim, state: &mut InventoryUi, icons: Option<&crate::hud::HudIcons>) -> (Option<BagAction>, bool) {
    let mut open = true;
    let mut action = None;
    let size = ctx.viewport_rect().size() - egui::vec2(40.0, 70.0);
    let width = size.x.min(1060.0).max(320.0);
    let height = size.y.min(680.0).max(240.0);
    ctx.layer_painter(egui::LayerId::new(egui::Order::Background, "inventory_shade".into()))
        .rect_filled(ctx.viewport_rect(), 0.0, egui::Color32::from_black_alpha(160));
    egui::Window::new("Character inventory").open(&mut open).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([width, height]).collapsible(false).resizable(false).show(ctx, |ui| {
            theme::tabs(ui, &["Inventory", "Combos & controls"], &mut state.tab);
            ui.separator();
            egui::ScrollArea::vertical().max_height(height - 90.0).show(ui, |ui| {
                if state.tab == 1 { combo_guide(ui); return; }
                let f = &sim.world.fighters[0];
                state.validate(&f.kit);
                let stats = f.kit.stats();
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Health {:.0}/{:.0}", f.hp, f.max_hp));
                    ui.separator(); ui.label(format!("Power +{:.0}%", stats.power));
                    ui.separator(); ui.label(format!("Armor {:.0}", stats.armor));
                    ui.separator(); ui.label(format!("Critical {:.0}%", stats.crit));
                });
                theme::hint(ui, "Click a slot to inspect. Double-click bag gear to equip. I / Esc closes. Combat continues while browsing.");
                ui.separator();
                let columns = if width >= 880.0 { 3 } else { 1 };
                ui.columns(columns, |uis| {
                    equipment(&mut uis[0], &f.kit, state);
                    let bag_column = if columns == 3 { 1 } else { 0 };
                    if let Some(a) = bag(&mut uis[bag_column], &f.kit, state, icons) { action = Some(a); }
                    let detail_column = if columns == 3 { 2 } else { 0 };
                    if let Some(a) = details(&mut uis[detail_column], &f.kit, state) { action = Some(a); }
                });
            });
        });
    (action, !open)
}

fn equipment(ui: &mut egui::Ui, kit: &Kit, state: &mut InventoryUi) {
    theme::section(ui, "Equipment");
    for slot in SLOTS {
        let item = kit.worn_in(slot);
        let selected = item.is_some_and(|id| state.selected == Some(Selection::Worn(slot, id)));
        let name = item.map_or("Empty", |id| ITEMS[id as usize].name);
        let response = ui.add_sized([ui.available_width(), 57.0], egui::Button::new(format!("{slot:?}\n{name}"))
            .fill(if selected { theme::TEAL_DIM } else { theme::CARD }));
        if response.clicked() { state.selected = item.map(|id| Selection::Worn(slot, id)); }
    }
}

fn bag(ui: &mut egui::Ui, kit: &Kit, state: &mut InventoryUi, icons: Option<&crate::hud::HudIcons>) -> Option<BagAction> {
    theme::section(ui, &format!("Bag  {} / {BAG_SIZE}", kit.bag.len()));
    ui.add(egui::TextEdit::singleline(&mut state.search).hint_text("Search items..."));
    theme::tabs(ui, &["All", "Gear", "Potions"], &mut state.filter);
    let query = state.search.to_lowercase();
    let visible: Vec<_> = kit.bag.iter().copied().enumerate().filter(|(_, id)| {
        let def = &ITEMS[*id as usize];
        def.name.contains(&query) && match state.filter { 1 => matches!(def.kind, ItemKind::Gear(_)), 2 => matches!(def.kind, ItemKind::Consumable { .. }), _ => true }
    }).collect();
    let columns = ((ui.available_width() / 78.0).floor() as usize).clamp(2, 5);
    let cell = (ui.available_width() - (columns - 1) as f32 * 6.0) / columns as f32;
    let mut action = None;
    egui::Grid::new("inventory_slots").spacing([6.0, 6.0]).show(ui, |ui| {
        for k in 0..BAG_SIZE {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(cell, 84.0), egui::Sense::click());
            let item = visible.get(k).copied();
            let selected = item.is_some_and(|(index, id)| state.selected == Some(Selection::Bag(index, id)));
            let p = ui.painter();
            p.rect(rect, 6.0, if selected { theme::TEAL_DIM } else { theme::CARD },
                egui::Stroke::new(1.0_f32, if response.hovered() || selected { theme::TEAL } else { theme::BORDER }), egui::StrokeKind::Inside);
            if let Some((index, id)) = item {
                let def = &ITEMS[id as usize];
                let (label, color) = match def.kind {
                    ItemKind::Consumable { .. } => ("POTION".to_owned(), theme::GOOD),
                    ItemKind::Gear(slot) => (format!("{slot:?}").to_uppercase(), theme::WARN),
                };
                if matches!(def.kind, ItemKind::Consumable { .. }) && let Some(icons) = icons {
                    let at = egui::Rect::from_center_size(rect.center_top() + egui::vec2(0.0, 20.0), egui::vec2(28.0, 28.0));
                    p.image(if id == 0 { icons.potion_red } else { icons.potion_blue }, at, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                } else {
                    p.text(rect.center_top() + egui::vec2(0.0, 19.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.0), color);
                }
                let text = p.layout(def.name.to_owned(), egui::FontId::proportional(12.0), theme::PAPER, cell - 10.0);
                p.galley(egui::pos2(rect.center().x - text.size().x * 0.5, rect.top() + 39.0), text, theme::PAPER);
                if response.clicked() { state.selected = Some(Selection::Bag(index, id)); }
                if response.double_clicked() && matches!(def.kind, ItemKind::Gear(_)) { action = Some(BagAction::Use(index as u8)); }
                response.on_hover_text(def.name);
            }
            if (k + 1) % columns == 0 { ui.end_row(); }
        }
    });
    if visible.is_empty() { theme::hint(ui, "No items here. Press E near a pickup or chest."); }
    action
}

fn details(ui: &mut egui::Ui, kit: &Kit, state: &InventoryUi) -> Option<BagAction> {
    theme::section(ui, "Item details");
    let Some(selection) = state.selected else { theme::hint(ui, "Select an item in your bag or an equipment slot."); return None; };
    let id = match selection { Selection::Bag(_, id) | Selection::Worn(_, id) => id };
    let def = &ITEMS[id as usize];
    ui.heading(def.name);
    ui.separator();
    let verb = match def.kind {
        ItemKind::Consumable { heal, ki } => {
            if heal > 0.0 { ui.label(format!("Restores {heal:.0} health")); }
            if ki > 0.0 { ui.label(format!("Restores {ki:.0} mana")); }
            theme::hint(ui, "Drink to restore. The effect applies when the drink completes.");
            "Use"
        }
        ItemKind::Gear(slot) => {
            ui.label(format!("Slot: {slot:?}"));
            let old = kit.worn_in(slot).map_or(Default::default(), |id| ITEMS[id as usize].mods);
            for (label, value, previous) in [("Power", def.mods.power, old.power), ("Armor", def.mods.armor, old.armor), ("Health", def.mods.vitality, old.vitality), ("Critical %", def.mods.crit, old.crit)] {
                ui.label(format!("{label}: +{value:.0}  ({:+.0} vs equipped)", value - previous));
            }
            if let Some(worn) = kit.worn_in(slot) { theme::hint(ui, &format!("Equipped: {}", ITEMS[worn as usize].name)); }
            "Equip"
        }
    };
    ui.separator();
    match selection {
        Selection::Bag(k, _) => {
            if ui.add_sized([ui.available_width(), 36.0], egui::Button::new(verb)).clicked() { return Some(BagAction::Use(k as u8)); }
            if ui.button("Drop on ground").clicked() { return Some(BagAction::Drop(k as u8)); }
        }
        Selection::Worn(slot, _) => {
            if ui.add_enabled(kit.bag.len() < BAG_SIZE, egui::Button::new("Unequip to bag")).clicked() { return Some(BagAction::Unequip(slot)); }
            if kit.bag.len() >= BAG_SIZE { theme::hint(ui, "Bag is full. Make space to unequip."); }
        }
    }
    None
}

pub fn combo_guide(ui: &mut egui::Ui) {
    theme::section(ui, "Melee combos");
    theme::hint(ui, "Tap LMB again during a landed strike to queue the next hit. Whiffs finish their recovery. Hold F / Mouse4 to charge a finisher. Directions are relative to your character's facing.");
    for (name, input, result) in guildbound_sim::combat::COMBO_GUIDE {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new(*name).strong().color(theme::TEAL));
            ui.label(*input);
            theme::hint(ui, result);
        });
    }
    theme::section(ui, "Movement & magic");
    ui.label("WASD walk | Shift run, then build momentum to sprint | Space jump | C roll");
    ui.label("Ctrl step | Shift + Ctrl homing dash | RMB hold guard, tap for perfect parry");
    ui.label("1 fireball (hold to charge) | 2 lightning | 3 frost nova | 4 blink | 5 healing");
    ui.label("E pickup / open chest | R healing draught | T mana tonic | F2 animation Studio");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, sim: &Sim, state: &mut InventoryUi, events: Vec<egui::Event>) -> (Option<BagAction>, egui::FullOutput) {
        let mut action = None;
        let output = ctx.run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))), events, ..Default::default() }, |ui| {
            action = show(ui.ctx(), sim, state, None).0;
        });
        (action, output)
    }

    fn text_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        fn find(shape: &egui::epaint::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() * 0.5),
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        output.shapes.iter().find_map(|s| find(&s.shape, label)).unwrap_or_else(|| panic!("missing {label}"))
    }

    fn click(ctx: &egui::Context, sim: &Sim, state: &mut InventoryUi, position: egui::Pos2) -> Option<BagAction> {
        frame(ctx, sim, state, vec![egui::Event::PointerMoved(position), egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        frame(ctx, sim, state, vec![egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]).0
    }

    #[test]
    fn clicking_a_bag_tile_selects_and_equips_the_correct_item() {
        let ctx = egui::Context::default();
        let mut sim = Sim::new();
        sim.world.fighters[0].kit.bag = vec![0, guildbound_sim::rpg::BRONZE_SWORD];
        let mut state = InventoryUi::default();
        frame(&ctx, &sim, &mut state, vec![]);
        let (_, output) = frame(&ctx, &sim, &mut state, vec![]);
        let at = text_position(&output, "bronze sword");
        assert_eq!(click(&ctx, &sim, &mut state, at), None);
        assert!(state.selected == Some(Selection::Bag(1, guildbound_sim::rpg::BRONZE_SWORD)));
        let (_, output) = frame(&ctx, &sim, &mut state, vec![]);
        let equip = text_position(&output, "Equip");
        let action = click(&ctx, &sim, &mut state, equip);
        assert_eq!(action, Some(BagAction::Use(1)));
        sim.world.step(&[guildbound_sim::Intent { bag: action, ..Default::default() }]);
        assert_eq!(sim.world.fighters[0].kit.weapon(), Some(guildbound_sim::rpg::Weapon::Sword));
        state.validate(&sim.world.fighters[0].kit);
        assert!(state.selected.is_none());
        let (_, output) = frame(&ctx, &sim, &mut state, vec![]);
        let worn = text_position(&output, "Weapon\nbronze sword");
        click(&ctx, &sim, &mut state, worn);
        let (_, output) = frame(&ctx, &sim, &mut state, vec![]);
        let unequip = text_position(&output, "Unequip to bag");
        let action = click(&ctx, &sim, &mut state, unequip);
        assert_eq!(action, Some(BagAction::Unequip(Slot::Weapon)));
        sim.world.step(&[guildbound_sim::Intent { bag: action, ..Default::default() }]);
        assert_eq!(sim.world.fighters[0].kit.weapon(), None);
        state.validate(&sim.world.fighters[0].kit);
        state.search = "bronze".into();
        state.filter = 1;
        let (_, output) = frame(&ctx, &sim, &mut state, vec![]);
        click(&ctx, &sim, &mut state, text_position(&output, "bronze sword"));
        assert!(state.selected == Some(Selection::Bag(1, guildbound_sim::rpg::BRONZE_SWORD)), "filtered cells retain the real bag index");
    }
}
