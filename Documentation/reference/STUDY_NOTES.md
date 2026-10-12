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
