# Request to Higgsfield: everything agent-1 needs to finish animation, posture and spell aiming

From agent-1. This is one long brief, but each section stands alone: do them in order, deliver each as its own zip (repo paths mirrored, a preview, the script, the checker output), and say which model made any generated pixels.

Branch to read: `ev/E12-carry` (has everything: E14 attack speed, E7 ragdoll, E8 god mode, E12 carry, the HUD skins).

You can run Python on the repo, which I can only partly do here (my renderer is a slow CPU path). You can't run the game. Most of what follows is analysis of our own files plus reference images, with numbers I can paste straight into the code.

---

## 0. Ground rules

### Real data only
| What | Where |
|---|---|
| Animation clip libraries | `assets/anim/ual1.glb`, `ual2.glb` (UAL rig, 65 joints, our hero skeleton), `sekiro.glb` (66 joints), `rokoko.glb` (52, Mixamo-style), `cmu/` (BVH) |
| Movesets (strikes, clip windows, weapon defs, new `mass`) | `assets/anim/movesets/*.json` |
| Strike windows | `crates/game/src/play/moveset.rs` (`Attack.windup`, `active`, `cancel` as fractions of a clip window) |
| Abilities | `crates/game/src/play/magic.rs` `ABILITIES` (6 arts on Q, 4 spells on E: name, element, cost, shape, clip window `start`/`end`, `release`) |
| Reaction clips | `crates/game/src/play/combat.rs` `clips::` (flinch `ual1/Hit_Chest`, knockdown `ual2/Hit_Knockback`, roll `ual1/Roll_RM`, steps `sekiro/Jog Strafe Left` …) |
| Hero meshes | `assets/characters/hero/{male,female}/*.glb` (65 joints, every garment shares the rig, your validator confirmed this) |
| Model-space convention | +X = the body's LEFT, +Y up, +Z forward, metres from the feet |

Donor rigs are mapped onto ours by bone name in `crates/game/src/motion/retarget.rs`. Our bone handles (`motion/mod.rs` `Bones`):
- `root`, `pelvis`
- `spine[0..3]`, `neck`, `head`
- `clavicle[2]`, `upperarm[2]`, `lowerarm[2]`, `hand[2]`
- `thigh[2]`, `calf[2]`, `foot[2]`, `ball[2]`

Index 0 = left, 1 = right.

### Format rules
- **Numbers:** JSON, SI units, radians in data and degrees only in human-readable notes.
- **Images:** PNG for anything a tool reads; JPEG is fine for pure reference art.
- **Previews:** every image you draw from data (plots, skeleton views) is a matplotlib or PIL render of the real numbers, never an illustration of them.
- **Generated reference images:** free model only. Pass our in-game shots (`docs/model_shots/`, `docs/art/2026-10-18*/`) as references so proportions match.

---

## 1. Posture audit of the clip library (highest value)

I need to know what our animations already do before I add more lean on top (E12 carry posture adds a shoulder drop, side lean and forward pitch, `crates/game/src/motion/carry.rs`).

**For every clip in `ual1.glb` and `ual2.glb`** (and `sekiro.glb` where it maps), FK the clip on its own skeleton at 30 fps and compute per frame, in model space:
- **Side lean:** the angle of the line pelvis → `spine_03` (or the clip's chest bone) from vertical, in the body's lateral plane (signed, + = toward the body's right).
- **Forward pitch:** the same line in the sagittal plane (+ = forward).
- **Shoulder heights:** each clavicle's outer end and each `upperarm` head (m), and their difference left − right.
- **Centre-of-mass proxy:** the mean of pelvis, spine and head positions; its offset from the midpoint between the feet (m, lateral and forward).
- **Feet:** height of each foot and ball above the lowest point of the clip, plus foot contact flags (height < 2 cm and speed < 0.15 m/s).
- **Hands:** each hand's height and distance from the chest.
- **Root:** speed (m/s) and turn rate (rad/s).

**Deliver:**
- `docs/anim/posture_audit.json`: `{ clip: { "lean": [min, mean, max], "pitch": [...], "shoulder_diff": [...], "com_offset": [...], "contacts": [[l, r] per frame], "root_speed": [...], "duration": s } }`.
- `docs/anim/posture_audit.csv`, one row per clip with the summary numbers.
- `docs/anim/posture_audit_top.png`: the 20 clips with the largest natural lean or pitch, as small skeleton plots (front and side, 3 frames each).
- A short note listing:
  - clips whose rest lean is already more than 4° (the carry posture must stay under that);
  - idle and walk clips whose shoulder difference already exceeds 3 cm.

**Why:** the carry caps in `motion/carry.rs` (shoulder drop ≤ 0.14 rad, side lean ≤ 0.08, pitch ≤ 0.12) were set by eye. I'll tune them so carried weight reads clearly but never doubles a lean the clip already has.

## 2. What carrying weight looks like (reference + numbers)

**Images** (free model, our hero model as reference, front and side view, plain background), each figure standing and walking:
1. empty hands;
2. a dagger in the right hand, about 0.4 kg;
3. a katana in the right hand, about 1.1 kg;
4. a greatsword two-handed at rest, about 3 kg;
5. a greatsword on the right shoulder;
6. a battle axe in the right hand, about 2.2 kg;
7. a crossbow held at the hip, about 3 kg;
8. a woodcutting axe over the shoulder;
9. a sheathed greatsword on the back.

Plus one annotated sheet. On top of the generated images, draw with PIL:
- the plumb line from the head;
- the shoulder line, with its tilt in degrees;
- the torso line, with its lean in degrees.

**Numbers:** for each case, your best estimate (from biomechanics references you know, stated as estimates) of:
- shoulder drop on the holding side (°);
- side lean of the torso away from the load (°);
- forward pitch (°);
- stance width change (cm);
- arm swing reduction on the holding side (%).

**Deliver:**
- `docs/anim/carry_reference/*.jpg`;
- `docs/anim/carry_reference/carry_targets.json`: `{ case: { mass_kg, hand, drop_deg, lean_deg, pitch_deg, stance_cm, swing_pct } }`.

I'll fit the `DROP_PER_KG`, `LEAN_PER_KG` and `PITCH_*` constants to it.

## 3. Predicted result of E12, drawn from our code (verification without the game)

Port `crates/game/src/motion/carry.rs` `angles()` and `apply()` to Python: a short file, model-space rotations about each bone (clavicle about Z, the two upper spine bones about Z then X), the same as `select::straddle`. Apply it to the UAL idle and walk clips with `Load` set per weapon mass (from the movesets) in the hand named by `hand` ("r" = 1).

**Deliver:** `docs/anim/carry_predicted.png`, a grid of front and side skeleton plots, with the clip pose in grey and the pose with carry in colour, one row per weapon:
- unarmed;
- dagger;
- katana;
- axe;
- greatsword;
- crossbow;
- sheathed greatsword.

Sheathed is the back load at 30 %.

**Check and report:**
- the lean goes away from the heavy hand;
- no bone flips;
- the feet are unchanged, since the offsets only touch the spine and clavicles and foot IK runs after them.

**This is the verification step.** If a sign is wrong (lean toward the weapon, shoulder up), tell me the exact line.

## 4. Strike timing audit (for E14 attack speed and E15 transitions)

For every attack in every moveset (`assets/anim/movesets/*.json`), the clip window plus `windup`, `active`, `cancel`:
- **Hand speed:** the weapon hand's speed curve over the window, plus the tip speed if `length` > 0, with the tip at `length` × `scale` along the weapon's +Y.
- **Phase check:**
  - does the peak hand speed fall inside `[windup, active]`?
  - how many frames before or after?
- **Windup readability:** the duration of the windup in seconds at 1.0×, 1.4× and 1.7× attack speed. Use the per-phase warp from `combat.rs` `phase_speed`: windup at s, strike at s^0.3, recovery at s^0.8. Add the 2-frame anticipation hold when s > 1.15.
- **Flags:**
  - attacks whose windup drops under 0.12 s at 1.4×, which becomes unreadable for the defender's AI and for players;
  - attacks whose active window misses the speed peak.

**Deliver:**
- `docs/anim/strike_audit.json`;
- `docs/anim/strike_audit.png`: one small speed curve per attack, with the windup, active and cancel bands shaded, at 1.0× and 1.4×.

## 5. Transition data for E15 (the analysis half of baking; no game code)

For every pair of attacks within one moveset (A then B), and for each attack to and from the idle and walk loops:
- **Pose distance**, at every candidate cut frame (A's recovery band against B's first 30 % of windup): the sum over bones of the angle between local rotations, weighted towards the spine and arms. Also the velocity distance.
- **Best cut:** the frame pair with the lowest cost (pose + 0.3 × velocity), its cost, and the blend length it would need (cost-scaled, 0.08–0.25 s).
- **Foot contact:** do both clips have the same planted foot at the cut? Flag the pair if not.

**Deliver:**
- `docs/anim/transitions.json`: `{ moveset: { "A>B": { cut_a, cut_b, cost, blend_s, foot_ok } } }`;
- `docs/anim/transitions_heat.png`: a cost matrix per moveset.

This is the data E15 would bake from. Agent-2 owns the bake, so this is analysis only.

## 6. Spell aiming references for E13

What I'm building is a crosshair, a hold-to-aim preview at the target, release to cast, and lasting marks in the world.

**Images** (free model, our world screenshots as reference, third-person camera over the shoulder), each showing the aim state:

| Ability | Aim preview |
|---|---|
| Cinder Bolt | arc + impact ring |
| Frost Bloom | slow bolt + bloom radius |
| Storm Lance | target outline + chain links to the 2nd and 3rd targets |
| Orbit Motes | orbit ring around the caster |
| Crescent Edge | swept band in front |
| Flash Step | a translucent copy of the caster at the destination, the onion skin |
| Aura Bullet | straight line + ring |
| Earthbreaker | ground ring following the terrain |
| Aura Ward | shell outline on the caster |
| Veil | the caster's outline fading |

**Numbers per ability**, read from `ABILITIES` `shape`. Mark any number you choose, rather than read from code, as `proposed`:
- preview radius / length / arc height;
- the preview colour (element: aura gold #FFC857, fire #FF8A3D, frost #9EE7FF, lightning #B79CFF, arcane #C9A2FF);
- the alpha while aiming, and in range versus out of range.

**Deliver:**
- `docs/anim/aim_reference/*.jpg`;
- `docs/anim/aim_reference/aim_previews.json`: `{ id: { kind: "arc|ring|band|ghost|chain|shell", size_m, color, alpha_in, alpha_out, notes } }`.

The marks (H3: `crack`, `scorch`, `frost`, `char`) come from your H3 pack; add the lifetime in seconds you'd suggest for each (20–40 s), and the size in metres at full scale.

## 7. Cast pose references (P-series, improved)

Free model, our male and female models as reference, front and side views, plain background. One sheet per pose, with no mannequin and the correct weapon in hand:
- **P2:** overhead windup, katana;
- **P3:** strike follow-through, katana;
- **P4:** spell aim, open palm, off hand forward;
- **P5:** greatsword carried on the shoulder;
- **P6:** two-handed battle axe at rest;
- **P7:** monk palm counter;
- **P8:** knocked down → getting up, three frames;
- **P9:** Flash Step lunge;
- **P10:** Earthbreaker slam, both fists down.

**Deliver:** `docs/anim/poses/*.jpg`, plus `poses.json` with your estimated key joint angles per pose. These are approximate, marked as estimates, in the order spine pitch, spine lean, shoulder raise, elbow bend, knee bend.

## 8. Video check of our own recordings

If your tools can analyse video: the user can send you our recordings, such as the showcase mp4, or the clips agent-1 records on request. Give a motion critique:
- foot sliding;
- snaps between moves;
- poses that read wrong;
- moments where the body leans the wrong way.

Each item gets a timestamp and the body part. This is the visual check I can't do well here.

---

## Order and what I do with each

| # | Deliverable | agent-1 uses it for |
|---|---|---|
| 1 | posture audit | caps for E12; find clips that already lean |
| 3 | E12 predicted plots | verify the sign and size of the carry lean before you see it in-game |
| 2 | carry reference + targets | fit the per-kilogram constants |
| 4 | strike timing audit | E14 tuning, AI tell readability |
| 6 | aim references + numbers | E13 previews and marks |
| 5 | transitions | E15 (to agent-2) |
| 7 | pose sheets | hand-tuning and checks |
| 8 | video critique | ongoing |

**Ship sections 1 and 3 first, together if possible.** They answer the question "is the leaning right?" without the game.
