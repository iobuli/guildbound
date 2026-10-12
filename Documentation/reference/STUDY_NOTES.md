# Study notes - read, understand, reimagine

Rule (Decision 21, boss 2026-10-12): references are studied, never copied. No copy-paste, no line-by-line
translation, no assets or data files. Reference copies live outside the repo in `D:/New folder (4)/reference/`.
Everything below is written in our own words, then built in our own Rust. Each finding names the Guildbound
system it feeds and its status.

---

## Overgrowth (Wolfire, code Apache-2.0) - read 2026-10-12

Source: `reference/overgrowth` (sparse: `Source/Physics`, `Data/Scripts`).

### Physics folder (`Source/Physics`, a wrapper over Bullet)

| Finding | Guildbound use | Status |
|---|---|---|
| The character collides as a sphere: gather contact planes, sort deepest first, push out along each normal by its penetration depth. A swept sphere from start to end, then the same push-out at the end, handles fast motion. | Motor vs world geometry once the world has walls, rocks and buildings (the open world). Deterministic and cheap, no rigid body. | planned (terrain task) |
| Physics bodies are driven toward a target pose by setting velocity = distance / (frames x dt), not by teleporting, so contacts still resolve. Same for rotation via axis-angle / dt. | Active ragdoll (COMBAT_MOTION_PLAN phase 4): powered bones chase the animated pose with velocities. | planned |
| Joint strength is a scalar per joint; angular stops limit joints. | Ragdoll muscle strength and the human joint limits from alpha 1 (knee hinge, hip twist +-45). | planned |
| Contact effects (sound, particles) scale with contact impulse. | Hit feedback: louder, bigger effects for harder hits. | planned |

### Character script (`Data/Scripts/aschar.as`)

| Finding | Guildbound use | Status |
|---|---|---|
| Ground movement is a push plus speed-scaled drag: input pushes the body, drag bleeds momentum so top speed is the balance point. Coasting with no input decays exponentially. Starts are fast and ease into top speed; reversing pushes against your own momentum (natural plant-and-turn). | `MotorModel::Drag` in `guildbound_sim`, discretized so terminal speed is exact at 120 Hz. | done (59dd432) |
| Top speed shrinks uphill and grows downhill; if you are already faster than the cap (from a slope or a hit) the cap follows your speed down slowly instead of clamping. | Slopes on terrain; knockback that carries. | planned (terrain task) |
| Body tilt = a speed term (lean into the run) + a smoothed acceleration term (weight shift), eased with per-frame inertia (stiffer on the ground than in the air). | `anim::lean` (speed + acceleration, critically damped spring). | done |
| Active block: a press opens a short window (0.2 s, up to 0.4 s on easy for the player); a 0.2 s recharge after each press stops mashing; holding does not renew it. | Parry window (Decision 14 already says 0.13 s, no renew by holding); add the recharge. | combat port |
| A guard meter drains per blocked attack, more when the defender was mid-attack (trading) or much smaller than the attacker. | Guard capacity (GDD: max 100, light 15, heavy 45, regen after 0.8 s at 25/s) plus the trading penalty. | combat port |
| Blocking plays a flinch layer whose weight is a stiff spring kicked on each block, then relaxed. | Procedural block flinch on the upper body, spring-driven, no new clip needed. | combat port |
| Jumps: a very short crouch (about 0.04 s) before take-off; jump velocity scales with how fast you were already moving; the body tilts toward the jump. | Jump verb. | movement verbs task |
| Rolls and flips are their own state with their own speed and acceleration. | Dodge roll / step. | movement verbs task |

---

## Dragon Ball Z: Budokai Tenkaichi 3 - read 2026-10-12

- `z3xox/BT3-Recomp` (GPL-3.0) is a PS2-to-PC static recompiler and renderer. It contains no game code; its
  function maps are auto-named (`FUN_...`, `f_...`). Nothing there describes combat, so it is not used.
- The mechanics below are the game's publicly documented systems (manuals, wikis, guides) as we understand
  them. They are design inputs to verify by play, not measured frame data.

| Mechanic | Guildbound mapping | Status |
|---|---|---|
| Rush combos: a light string (up to about five hits); a heavy input partway through branches into a finisher that depends on the hit count and the held direction (up = launch, down = sweep/smash down). | Decisions 15-17 already say this. Combat clusters give each strike type rotating variants; the move table (phase 1) owns branches by count and direction. | clusters done; move table in combat port |
| Smash attack: hold heavy to charge; a full charge breaks guard and sends the target flying. | Charged heavy (GDD "hold F = charged smash"). | combat port |
| Steps and dashes: short evasive steps; a long dash that costs ki and can home in on the target. | Ctrl step/dash (GDD keymap); homing dash toward the lock target. | movement verbs task |
| Vanish / Z-counter: a perfectly timed guard teleports behind the attacker. | A parry variant: perfect window -> reposition behind the attacker plus a counter opportunity. | later |
| Sway / dodge out of guard. | Guard + direction = short dodge. | later |
| Ki: charge by holding a button; dashes, blasts and specials spend it; blast stock fills from combat. | Mana / element resource (GDD Fire costs, awaken). | later |
| Lock-on with target switching; the camera frames both fighters. | Lock-on exists in the sim (`lock_target`); camera framing of both fighters is a camera task. | partly done |
| Knockback with wall and ground bounces; a chase follow-up (Dragon Rush) on a knocked-away target. | Momentum knockback in the sim; bounce off arena walls; chase later. | combat port (knockback) |

---

## FATE (WildTangent) - read 2026-10-12

Source: `reference/fate/FATE/Source/DEV` (outside the repo). Studied for mechanics only; nothing copied.

| System | Finding (our words) | Guildbound use | Status |
|---|---|---|---|
| Monster brain | A state machine: wander, hunt, approach, attack, flee, get-range, cast, pick up item, return to town (pets), unique idle. Re-checks target validity every update (dead, out of active range, same faction or neutral -> drop target). | Mob AI for the open world: same states, our deterministic sim. | planned |
| Aggro and leash | Attention radius starts a hunt; a larger follow radius ends it (leash), unless the monster has a master. Paths refresh at most once per second. | Mob aggro/leash; matches the alpha 2 OpponentPost spec (watch 12 m, leash 20 m). | planned |
| Tactics | Melee monsters close in; mage-type monsters keep 25-60% of their attention radius and reposition to a random open spot near the target, rarely re-picking. | Caster mobs that kite; melee mobs that rush; pairs well with our elements. | planned |
| Spell roles | Monsters pick spells by role: attack, summon, banish, defend, heal self, heal minions. | Mob skill kits by role. | planned |
| Monster ranks | Higher-rank monsters give 1.1-1.2x experience and 5-10x item rarity; rarer to spawn (10-20x). | Champion/elite mobs; feeds the Hunter's Card rank ladder. | planned |
| Item quality | Grades Normal, Superior, Exceptional, Flawless prefix the name and scale stats; separate Magical / Unique / Artifact tiers; some items need identifying. | Item quality tiers on our ItemDef (Decision 6). | planned |
| Damage types | Piercing, slashing, crushing, magical, fire, ice, electric, undead; items add typed damage bonuses. | Typed damage; fire and electric already exist in our GDD. | planned |
| Sockets | Items carry socket counts; gems add effects. | Socketed gear as a crafting sink. | later |
| Effects | Each effect is passive (while equipped) or on use, with a target: user, character, area, item, user's pets. | Item and skill effect model (Decision 7 EffectDef). | planned |
| Inventory | 11 equipment slots (head, chest, both hands, arm, gloves, neck, feet, two rings, belt) plus 5 quick slots; bag items occupy a width x height grid. | Inventory page; our quick slots map to keys 1-4. | planned |
| Dungeons | Generator pipeline: carve a corridor maze, sparsify dead ends, place scored rooms (rectangular, oval, ring), pits, reconnect isolated rooms, doors, props, exits. | Procedural dungeons beneath the open world; same staged pipeline in our own code. | planned |

## Brackeys RPG-Tutorial and ML-Agents-Fighting-Animations (2026-10-12)
Clones (reference only): `D:/New folder (4)/reference/brackeys-rpg/repo`, `.../mlagents-fighting/repo`. Neither repo has a licence file, so all rights are reserved: nothing copied, ideas only.
- Brackeys (Unity C#): stat = base + sum of equipped modifiers, equip swaps with the bag, consumables, ground pickups, chest, overhead health bars. Ours: `crates/sim/src/rpg.rs` (power % / armor 100/(100+a) / vitality, deterministic via `Intent.interact` + `Intent.bag`), `crates/game/src/loot_view.rs`, inventory window in `panel.rs`, `hud::overhead_bars`.
- ML-Agents fighting (Unity, ML-Agents R12): an RL agent pushes a katana to meet the attacker's blade, arm IK follows. Ours is analytic and unarmed: `crates/game/src/guard_ik.rs` puts both forearms on the attacker's striking forearm while guarding/parrying. Presentation only; the sim decides the block. No training pipeline.

## Absorb pass 2026-10-12: Sekiro prototype, UE5 melee, RPG template, Quaternius, souls-like template
Clones (reference only, D:/New folder (4)/reference/): `Godot4ThirdPersonCombatPrototype` (Snaiel, MIT), `ue5-rpg-melee-combat-system` (Mustafa-Kum, no licence: ideas only), `RPG-template` (Lakshman-YT: store page only, the controller is a paid EULA product; feature list read, nothing downloaded), `cats-souls-like` (Cat Prisbrey, Unlicense + CC0 assets). `RPG-Tutorial` / `ML-Agents-Fighting-Animations` were re-cloned; already absorbed above.

### Snaiel Godot Sekiro-style prototype (MIT) -> built
- Instability (posture) fills from hits, blocks, parries; parrying a hit adds a little to the parrier, being parried adds a lot to the attacker; reduction pauses after each gain, then drains a share of max per second. Full = dizzy; dizzy from a parry is short and finisher-ready, dizzy from damage is long; a hit on a dizzy enemy is a one-shot finisher; leaving dizzy keeps 70% of the meter.
- Notice: idle -> getting suspicious -> suspicious (walk to last seen) -> getting aggro -> aggro, gauge rate from an inner/outer cone (angle and distance), instant aggro in the inner cone, aggro drops after 20 s out of sight. Off-screen triangles.
- Backstab: within 1.5 m, behind the victim (>125 deg from its facing), victim not aggro -> instant kill.
- Parry spam: window shrinks 0.2/0.1/0.05/0 s on repeated presses inside a 0.4 s cooldown. Health flask: charges, drink animation, heal at a mid-animation signal, interruptible.
- Ours (fused, improved): `crates/sim/src/posture.rs` fuses instability with our guard meter (guard still absorbs blocks; posture is the separate balance meter), recovery scales with hp and guarding, parry can never break the parrier, deathblow covers dizzy and backstab in one rule; `notice.rs` is one 0..2 gauge (no five state nodes), running is heard outside the cone, alert decays to suspicious (searching) instead of straight to idle. Our parry recharge already covers spam. Consumables became timed, interruptible drinks (item kept if cut short).
### UE5 melee (no licence, ideas only) -> partly built
- Ref-counted notify windows (combo, move-interrupt, ability-interrupt, hit), 0.2 s input buffer, separate light/heavy combo indices with a reset delay, crit roll + crit text styling (no rise, scale punch), per-weapon hit stop and camera shake, in-combat flag with a 5 s exit timer, cast bar with a commit point.
- Ours: move-cancel of late recovery (`posture.move_cancel`), deterministic crits (`Stats.crit`, duelist's gloves), damage numbers with crit punch (`hud::damage_numbers`). Planned: in-combat state, cast commit bar.
### RPG-template feature list (EULA, read only)
- Bow aim with time slow, crouch, directional hit reactions, idle head look, circular stamina, dynamic crosshair, target dummies. Ours: directional hit reaction (`mx_hit_back` when struck from behind), Quaternius training dummies. Planned: bow/aim, stamina ring.
### Assets absorbed onto the RPG hero (nucleus)
- Mixamo set from the Godot prototype (19 clips: dizzy, deaths, finisher, hits, kneel, block, magic, turns) and Quaternius UAL1+UAL2 (22 clips: consume, guard break, knockback, chest/interact/pickup, punches, roll, sprint, slide, throw, get-up). Weapon, zombie, farming and locomotion sets omitted (RPG walk/run stays the nucleus). Retargeting fixed a 30-vs-24 fps bug for .blend sources. Hero: 130 clips, audit pass.
- Audio: Kenney CC0 recordings and the alpha's synthesized cues (assets/audio, credits inside) drive `crates/game/src/sfx.rs`.
- Quaternius MegaKits (CC0, boss's kits): courtyard dressing table in `props.rs`. Wenrexa UI and icon packs: extracted to assets/packs/_sources/ui for the Beta V2 HUD.

## Beta V2 master pass (2026-10-12): Cat's souls-like template, new kits, search finds
- **Cat Prisbrey's Godot souls-like template** (Unlicense code, CC0 assets; reference copy `D:/New folder (4)/reference/cats-souls-like`): one key does both, a tap dodges and a hold (0.3 s) sprints; hard landing past a fall height; i-frames during roughly the first 70% of a dodge; weapon/gadget slots with a change animation; consumable quick slot cycled and used by key; ladders, levers, gates and doors as interactables; spawn sites (rest/respawn points); patrol points for enemies; root-motion driven controller. Ours: quick slots (R healing draught, T mana tonic) in `sim_bridge` + HUD; combat/exploring modes with jog/sprint (`Tuning.relaxed`, `in_combat`); planned: tap-dodge/hold-sprint on one key, rest sites, ladders/levers/gates, patrol routes for mobs.
- **Quaternius Universal Base Characters + Modular Character Outfits (CC0)**: universal UAL skeleton shared with UAL1/UAL2 and the Bestiary. Built: `Tools/blender/build_master.py` makes the master hero (Male Ranger, base head only, hair) and NPC models; bones renamed to the RPG names the game reads; the RPG hero library (130 clips), Kevin Human Basic Motions (non-combat walk/run/sprint/jump) and UAL townsfolk clips retargeted onto it. Bestiary Imp/Puglin share the skeleton but ship no clips: planned as mobs once fighters can carry their own clip library.
- **Kevin Iglesias Human Basic Motions FREE** (boss's asset): non-combat 8-direction walk/run, forward sprints, jump/fall; idles cut (boss: RPG idles only).
- **Modular terrain collection** (OBJ, 305 pieces): extracted to `assets/packs/_sources/modular_terrain`; planned for the world blockout.
- **Search finds** (cloned for reference): `vleue/vleue_navigator` (MIT/Apache, Polyanya navmesh from glTF or obstacle components; Bevy 0.18 at clone time) for mob/NPC pathfinding around the scatter; `janhohenheim/bevy_ahoy` (Bevy 0.19 kinematic controller) for ledge/step ideas; Mischief in Miniature (MIT/Apache, CC0 assets) noted. Rejected for now: bevior_tree (targets Bevy 0.15).

## Overgrowth (Wolfire Games) - procedural life and physics, for Possession (2026-10-12)
- Source: github.com/WolfireGames/overgrowth, sparse clone at `D:/New folder (4)/reference/overgrowth` (Data/Scripts).
- Licence: Apache-2.0 for the code (README; assets under Proprietary/ are not ours to use). We adapted ideas and
  numbers in our own Rust, no code copied, so no LICENSES.md row is needed.
- Read: `aschar.as` UpdateEyeLookTarget, UpdateHeadLook, DoHeadIK, DoChestIK, UpdateTilt, HandleAccelTilt,
  Breathe, UpdateItemFistGrip, SetActiveRagdollFallPose.

| Mechanic (our words) | Numbers | Feeds | Status |
|---|---|---|---|
| Look target priority: forced/lock target's head, else the player's camera facing (far point), else an NPC's chosen character or an idle gaze re-picked every 0.8-2 s in the front half, mixed half with facing, +-0.3 vertical. A running player glances between run direction and camera when sin(3t)+sin(2.3t) > 0. | 0.8-2 s gaze slots | `procedural::fighters`, Possession look "wander" | built (deterministic hash instead of RNG) |
| Head look strength 0 in flips, rolls and non-movement/attack states, reduced while flinching | - | `procedural::fighters` (busy states) | built |
| Head turn: yaw/pitch from the head frame, yaw limit 1.3 x 0.7 rad for humans, pitch +-0.8, keep the old side when the target swings behind (hysteresis), springy with acceleration inertia, applied half at the neck and half at the head | 0.91 / 0.8 rad | `procedural::look_angles/turn_head` | built (critically damped spring, 11 rad/s) |
| Chest follows the look partly (torso control 0.35-1 by stance, 0 in air, reduced while attacking) | head_range 1.0 rad | our chest aim spring in anim.rs already turns the chest to strike targets; the head subtracts it | fused |
| Tilt into motion: target = velocity x 0.5 deg/(m/s) + smoothed acceleration x 1 deg/(m/s^2), ground only, eased (inertia 0.9 per 1/120 s) | 0.5, 1.0 | Possession lean; fighters already lean in anim.rs (0.02 rad/(m/s), 0.018 rad/(m/s^2)) | built |
| Breathing: rate eases from 0.9 to 4.9 rad/s with speed (combat counts as half), amplitude grows with rate | 0.9-4.9 | `procedural::breathe` (chest pitch 1.6 deg, shoulders 2.5 deg) | built |
| Fist grip when a hand holds something (morph target there) | 0/1 | finger curl by grip from the sim action: fists in strikes/guard, open for magic, relaxed out of combat | built (bone curl, eased) |
| Active ragdoll on a fall: strength from speed (0.1 x speed, max 0.8), curl to a fetal pose near hazards, arms out front when the torso faces the hazard | 0.8 max | ragdoll.rs (perfect-parry knock): protective arms | planned |
| Small random shake on head/torso targets (0.001-0.005 rad) for life | - | - | rejected for now (breathing gives the life; RNG not wanted) |

Fusion: Overgrowth drives a few key poses with springs; we keep full clips but add the same spring layers on top,
plus the authority rules (joints.rs, knee.rs) that Overgrowth does not have, and Possession gives the boss and Claude
direct control over every bone with the physics readout (centre of mass, momentum, balance) Overgrowth never shows.
