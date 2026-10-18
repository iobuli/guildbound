# Guildbound art with Higgsfield: master prompts, one asset at a time

Rules:
- **Free generations only** (trial "unlim"); never spend credits. Check `unlim.available` first.
- **One asset per run.** Generate, review, bring it into the game, commit, then the next.
- **Our actual models are the reference.** Upload the in-game shot (`docs/model_shots/`) with every
  character, pose or concept prompt so the art moves from what we have, not from scratch.
- **AI output is reference or source, never shipped unprocessed.** Textures go through
  `tools/model/textures.py` (tiling, palette tint, `normal_from_height`); log every used asset in `LICENSES.md`
  (model, date, prompt id from this file).
- Keep the files small: 1k unless the asset needs more; delete downloads once processed (disk is tight).

## 0. Master style block (prepend to every prompt)
> Guildbound, a stylised third-person fantasy RPG. Hand-painted toon look with 4 soft light bands,
> a thin dark ink outline and gentle rim light; readable silhouettes, low-poly friendly shapes,
> no photoreal pores or film grain. Natural dyed fabrics in an earthy palette: madder red #8F2B24,
> moss #4D6638, woad #385275, linen #DBCCA8, ochre #B88538, umber #54422F, charcoal #333336,
> leather #734F30; guild blue #334D8C and guild gold #C79E42 for the order. Daylight, soft sky.

## 1. Textures and normal maps (one material per run)
The AI makes a **tileable albedo** and a **matching greyscale height map** (white = high).
`textures.py` turns the height into the normal map (`normal_from_height`) and tints albedo to the palette,
so the game keeps one consistent normal style.

Template:
> [master block] Seamless tileable texture, flat orthographic top view, even lighting, no shadows or
> highlights baked in, 1:1. Material: **{material}**. {detail}. Second image: the same pattern as a
> greyscale height map, white raised, black recessed, no colour.

| # | material | detail | replaces / feeds |
|---|---|---|---|
| T1 | plain linen weave | visible over-under threads, slight slub, worn soft | `cloth("linen")`, shirt, tunic |
| T2 | wool twill | diagonal twill ribs, felted fuzz | trousers, kilt, cloak |
| T3 | tanned leather | fine grain, creases, scuffed edges | boots, belt, baldric, gloves |
| T4 | quilted gambeson | diamond stitching, padded puffs | tabard, vest |
| T5 | silk for haori/hakama | soft sheen folds, faint woven pattern | haori, hakama, robe |
| T6 | hair strands | long clumped strands, neutral grey | `hair()` |
| T7 | skin detail tile | pores and fine wrinkles only, mid grey | detail layer over `skin()` (UV-baked skin stays ours) |
| T8 | worn flagstone | irregular cut stones, moss in joints | ruins walls, guild floor (E6, agent-3) |
| T9 | packed dirt path | pebbles, cart ruts | `ground.rs` (agent-3) |
| T10 | weathered wood planks | grain, nail holes | doors, crates, stairs |

## 2. Concept art from our models (one character per run)
Input: that look's in-game shot. Goal: see where the look can go next, then port the good ideas into
`appearance.json` and `tools/model/build.py`.

> [master block] Character concept sheet based on the attached in-game model. Keep the body
> proportions, face and outfit layering; push the design one step further: {direction}.
> Front, three-quarter and back views plus two detail call-outs, neutral grey background, 16:9.

| # | look | direction |
|---|---|---|
| C1 | wanderer (player) | travel-worn adventurer, layered straps, a pouch belt, a scarf |
| C2 | blade | disciplined swordsman, lacquered shoulder guard, sash for the sheath |
| C3 | arcanist | scholar-mage, rune-stitched robe edges, focus charm, ink-stained sleeves |
| C4 | hunter | ranger kit, quiver, fur collar, hand-wrapped bow grip |
| C5 | monk (the order) | guild blue and gold, prayer beads, wrapped forearms, hakama |
| C6 | oni | horned mask, heavy rope belt, scorched cloth, imposing silhouette |
| C7 | captain | guild officer, gold trim tabard, cape clasp |
| C8 | receptionist | guild clerk, apron with ledgers, ink pen, tidy hair |
| C9 | smith / farmer / villager | everyday village wear for the area styles |

## 3. Poses to recreate (one per run)
Input: our model in rest pose. Output: a clean full-body pose drawing to hand-tune in `motion/tweak.rs`
or match in mocap. Side and front view, same character, plain background.

> [master block] Full-body pose reference of the attached character, front and side view,
> plain light background, clear joint angles: **{pose}**.

P1 guard stance, blade forward · P2 heavy overhead windup (E14 anticipation) · P3 strike follow-through ·
P4 spell aim with open palm (E13) · P5 carrying a greatsword on the shoulder (E12) ·
P6 two-handed axe at rest (E12) · P7 monk palm counter · P8 knocked down, getting up.

## 4. Game UI art (one element per run)
The HUD stays our own Bevy UI (`docs/GUI_PLAN.md`): turquoise terminal glass, #2EE6C5 accent,
#7CFFB2 second accent, JetBrains Mono. AI makes **mock-ups and icon sheets**, not the UI itself.

> [master block, minus the fabric palette] Game UI {element}, transparent dark glass panel,
> 1 px turquoise #2EE6C5 border with corner brackets, faint scanlines, monospace text, flat icons
> in a single turquoise line style, 16:9 screen mock-up over a blurred grass field.

U1 full HUD layout · U2 icon sheet: items and tools (5×4 grid, transparent) · U3 icon sheet: spells and
skills · U4 spell reticle and aim states (E13) · U5 bag with tabs and crafting (E11) ·
U6 god-mode / debug panel (E8) · U7 class select screen · U8 dialogue box with gibberish decode.

## 5. Functional GUI layers (code, no AI needed)
Planned for the HUD, each its own toggleable layer:
- **Combat layer:** target frame (hp, poise, stance), attack-speed and windup tell, block/parry flash.
- **Aim layer (E13):** reticle, spell landing preview, range ring.
- **Carry layer (E12):** held-weight indicator per hand.
- **World layer:** floating names, interaction prompts, area banner on entering ruins/pond/guild.
- **Debug layer:** E8's F11 panel; F12 stats.

## Order
T1 → T3 → T2 (the most-seen surfaces first), then C1, C5, C6, then U1/U2, then poses as E12–E14 need them.
