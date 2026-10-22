# Magic rework and new world (plan, 2026-10-22)

Two tracks, shared with Grok through the workroom board (`tools/kev/studio/board/`). Tasks T009 onward.

## 1. Skills, arts and spells: three different things

| | Skills | Arts | Spells |
|---|---|---|---|
| What it is | what you are trained in; no button | a body technique | a chanted working of the world |
| Fuel | experience (deeds) | Battlewill (aura of the body, built by fighting) | Magicules (regenerate after a pause) |
| Input | none: they change arts and spells | Q | E |
| Timing | passive | no chant; the move itself is the cast | chant, then release; can be interrupted |
| Where the damage comes from | modifiers (cost, speed, power, discovery) | the body: the hit lands on a foot, fist or blade bone at the moment of contact | the effect: projectile, beam, zone, summon |
| Animation | none | martial moves (the kicks, dive, backflip, palm thrust from the Kev sheets) | cast poses by element (cast, thrust, overhead, channel) |
| Look | level-up flash only | white-gold aura, smear streaks, shock rings, impact frames, afterimages | element colour, shader beams, sprite flipbooks, lights |
| Sound / HUD | quiet tick, SKILLS tab | dry crack and thud; Battlewill bar; ARTS tab | tonal rise and release; Magicule bar; MAGIC tab |

Rules that keep them apart:
- An art never has a chant and never has a travelling effect that outlives the move, except aura blades (they are the technique's edge).
- A spell never lands from a body bone: its damage is at the effect.
- A skill never has a button. It changes costs and numbers of arts and spells (`modifies` in skills.json) and decides which are discovered.

## 2. Effects (the three uploaded packs)

- `assets/vfx/pixel/`: 20 flipbook sheets, public domain (Free Pixel Effects Pack, 100x100 frames). Used as billboards for hits, casts, zones.
- Energy beams (MIT, Godot): ported to a Bevy material in `crates/game/src/play/energy.rs` + `assets/shaders/energy_beam.wgsl`: tapered tube, vertex bend (wave, arc, helix, coil, whip), nine fragment styles, flare and impact shock. The ten Godot variants become ten presets.
- Wallcoeur Fire VFX (Unity, licence not stated): kept local and out of git (`assets/vfx/fire/` is ignored). Flame and smoke textures only; prefabs are Unity-only.

## 3. New abilities (first set)

Arts (Q, body): Falling Star (axe kick, ground shock), Hurricane Kick (spinning back kick, crescent shock), Dragon Step (flying side kick, dash strike), Meteor Dive (the aerial dive as an art), Ghost Flip (backflip, afterimage then counter).
Spells (E): Solar Lance (gold beam, burns), Glacier Shard (ice beam, chills), Prism Ray (channelled beam, splits), Violet Storm (arc beam that chains), Flame Line (ground fire, damage over time), Vortex (pull), Protection Circle (ward zone), Nova (white burst).

## 4. Enemies and map (Grok leads)

- Village: the existing guild hall, market and foundry pads plus a proper village street of houses; peaceful NPCs, guards, pests (rats, crows) and occasional raiders.
- Wild: forest, ruins, a cave mouth, a marsh; element-themed monsters that each use one spell or art so players meet every effect: frost wolf (Glacier Shard), ember imp (Flame Line), storm hawk (Violet Storm), bog wisp (Vortex), stone bruiser (Falling Star), bandit mage (Solar Lance), a mini boss.
- Layout data in `assets/data/regions.json` (pads, items); spawn tables per region.

## 5. Order of work

1. Import packs (assets/vfx, credits). 2. Port the beam shader and prove one beam in game. 3. Flipbook FX. 4. `Shape::Beam`, `Shape::Zone`; three spells. 5. Arts bound to the new animations. 6. Separation in HUD and data. 7. Enemies and regions (Grok). 8. Recordings and review on the board.
