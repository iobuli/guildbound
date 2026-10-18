# Higgsfield batch, 2026-10-18 (reference art, not shipped in the game)

Prompts: `docs/ART_PROMPTS.md` (U2, U3, U1, C1, C6, C5, P2, P3, P4). In-game reference: `docs/model_shots/ingame_wanderer.png`.

| file | use | notes |
|---|---|---|
| `U2_item_icons_stale_sim_items.jpg` | icon style | items are from `crates/sim/src/rpg.rs`, which the game does not use. Real bag items: Log, Stick, Stone, Apple (`play/items.rs`); weapons: katana, greatsword, dagger, knives, crossbow, bow, axe (`assets/anim/movesets/`) |
| `U3_spell_button_states.jpg` | **design for the Q/E slots**: idle, charging (arc), cooldown (sweep) | shows the old 5 sim spells on 1-5; the game has 6 arts (Q, Battlewill) + 4 spells (E, Magicules) in `play/magic.rs` `ABILITIES`, and 1-0 pick weapons |
| `U1_hud_layers_mock.jpg` | layering idea: vitals / grouped casts / weapon row, reticle above | same stale spells; background is not the game |
| `C1_wanderer.jpg` | next step for the player look | scarf, pouch belt, crossed straps |
| `C6_oni.jpg` | oni look | mask, rope belt knot, scorched cloth |
| `C5_monk.jpg` | order monk | drifted to green; the order is guild blue + gold |
| `P2_overhead_windup.jpg` | E14 anticipation pose | mannequin, not our character |
| `P3_strike_follow_through.jpg` | strike end | no weapon in hand |
| `P4_spell_aim_palm.jpg` | E13 aim pose | good |
