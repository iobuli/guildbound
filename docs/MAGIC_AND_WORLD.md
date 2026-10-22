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

## 6. Mechanics added (user, 2026-10-22)

What exists today: hold Q or E to aim, a preview by shape, release to cast (`spell_aim.rs`); `Crescent` is a thrown aura blade, a projectile. No spell charges; nothing absorbs; the only movement ability is `Step`.

1. **Hold and charge** (T019). Certain spells (and some arts) charge while the key is held: the cast pose holds at its wind-up (the same fixed-time hold the heavy attacks use), three stages with visible cues (aura and ground sigil brighten, a ring closes at each stage, a rising tone), release scales radius, power, range or beam length. Over-holding past the last stage bleeds Magicules; dodging cancels. Tap still casts the small version. Data: `charge: { stages, seconds, cost_growth }` on the ability.
2. **Better aiming** (T020). Ground reticle with the true radius for area spells (raycast to the ground under the crosshair, clamped to cast range, range ring shown); beam line preview to the first hit; lobbed arc with the landing ring; a soft lock cone that bends the cast toward the nearest enemy near the crosshair (off on gamepad option `aim assist`, on by default); target lead for moving foes (`aim::lead`); camera eases in while charging and shows the reticle's ground point.
3. **High-level area spells** (T021, "wow"): Meteor Rain (telegraphed circles on the ground, meteors fall one by one with fire flipbooks and a crater shock each), Absolute Zero (a ground sigil, then an expanding frost nova that freezes foes in blocks), Judgment Beam (a sky beam the width of the circle, ground ring, camera shake), Storm Call (strikes in the circle in sequence, chain arcs), Black Hole (pulls, then implodes). Gated by rank and mastery, long charge, large cost, a cooldown; every one gets a slow-motion beat, camera shake, a screen flash and scorch or frost marks that stay a while.
4. **Movement spells** (T022): Blink (short teleport, afterimage at both ends, the existing `Step` generalised), Gale Leap (launch with a glide), Phase Dash (dash through foes, brief untouchable), Rift Step (mark a spot and recall to it).
5. **Set-piece finishers** (T023): when a foe is below a threshold or broken, and the fighter has full Battlewill and enough Magicules, a finisher can be called (Q+E): a short scripted sequence with its own camera, a slow-motion beat and a signature animation from the Kev library (the aerial dive into a pillar of light, a spinning kick that opens a crater, a thousand-cut combination). One per fight; it costs everything; it always ends the target.
6. **Absorption** (T024): Siphon (a held guard whose release in the window of an incoming spell absorbs it: the element is stored as a charge, the next spell of that element is stronger, a share of its cost comes back), Devour (a touch that drains a foe's Magicules into yours), Absorb Art (turn a blocked blow into Battlewill). The stored element shows on the HUD and tints the aura.
7. **Sword spells cut, they do not fly** (T025): a new `Shape::Cut { reach, arc, delay }`. The sword's own swing is the cast: the cut is a thin, bright slash ribbon (smear-style shader) laid along the arc of the blade at the moment of the swing, and it damages everything it crosses at once (a swept segment test over the cast window, not a travelling body). A second after-cut line can appear after a delay (Sekiro/Bleach style). Variants: Wide Cut (an arc), Piercing Cut (a line), X Cut, Sky Splitter (vertical, long), Dimension Cut (finisher). The old Crescent projectile becomes the Wide Cut.

Division of work: Claude: the effects and core mechanics (T010 to T014, T019, T020, T021, T024, T025). Grok: world and enemies (T015 to T017), movement spells and finisher sequences (T022, T023, it knows the animation tools), review (T018). Each step goes through the stepped joint-work rule (reason, review, act, verify).
