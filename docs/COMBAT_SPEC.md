# Combat spec: hand-to-hand melee + lock-on (build spec)

Controls follow `docs/CONTROLS_AND_PHYSICS.md` §2.2 and §4. Everything is built on the physical body: strikes are real
contacts between rigid parts, reactions go through the target's BodyRig, knockdowns use the ragdoll + GetUpPose get-up.
Unarmed now. Moves are data, so weapons can be added later.

## 1. Foundation (must land first, keeps the game running)

1. **Intent source.** `game/intent.gd` (`class_name Intent`, RefCounted). Fields:
   - `move: Vector2` (x right, y forward, -1..1)
   - `view_yaw: float` (the yaw that movement is relative to)
   - held flags: `sprint, crouch, guard, attack`
   - one-frame `just_*` flags: `jump, dodge, interact, attack, attack_released, guard, lock, lock_next, lock_prev, ragdoll, cast_1..4`
   - `Intent.from_input(cam)` builds it from `Input` (the local player).
   - AI brains fill one directly.
   - `Player` reads only `intent` (set every physics frame before the tick). No `Input.*` calls remain in player.gd.
   - The self-test keeps working because the local source still reads `Input`.
2. **Player as a reusable fighter.**
   - `Player` works with `cam == null` and no HUD, and `is_ai` skips mouse handling.
   - `main.gd` / `arena.gd` can spawn extra `Player` instances (same CharacterVisual / BodyRig / MotionIK / GetUpPose / RagdollPose) with their own `CharModel` (race, level).
   - An `ai` Node child provides the intent.
3. **Characters collide with each other, never with themselves.**
   - **Capsules:** player capsules mask `L_PLAYER` too, so fighters can't walk through each other.
   - **Body parts:** every BodyRig part has `L_BODY` in its mask at all times. Each rig adds collision exceptions between ALL pairs of its own parts (105 pairs), except during ragdoll and get-up, when self-collision is wanted. `start_ragdoll` / `recover(true)` remove the non-adjacent exceptions; `solo()` / `reset()` restore them. Only parent/grandparent exceptions stay permanent.
   - **Result:** one fighter's fists and forearms physically stop on another fighter's body, and the drive's contact force pushes the target's parts.
   - **Registry:** `Player.fighters` (static Array) lists live fighters for lock-on and AI queries.

## 2. Combat component: `game/combat.gd` (`class_name Combat`, Node child of Player)

States: `IDLE, WINDUP, ACTIVE, RECOVER, CHARGING, GUARD, STAGGER, GUARD_BREAK`. Only usable when the player state is
GROUND (not carrying, climbing, swimming, ragdolled or getting up). Carrying keeps LMB = throw.

**Input rules**
- Attack inputs are buffered for 120 ms.
- LMB tap gives the light combo. The next press counts if it arrives in the RECOVER combo window (the first 60% of recovery).
- LMB held for 0.35 s or more starts CHARGING (max 1.0 s). Releasing it gives the charged strike.
- RMB held is GUARD. An RMB press within 150 ms before an incoming hit lands is a PARRY.
- Alt is the existing dodge. Its i-frames count. A **perfect dodge** happens when an enemy strike's active window would have hit within 0.2 s after the dodge started. It grants **Flurry**: attack_speed `more` x1.3 for 1.5 s, plus the attacker's combat clock runs at 0.5x for 1.5 s.

**Moves**: `data/moves.json`. Times are in seconds at the reference attack speed 1.5:

| id | hand | aim | startup | active | recovery | dmg mult | impact | stamina | reach m |
|---|---|---|---|---|---|---|---|---|---|
| jab | l | head | 0.10 | 0.08 | 0.22 | 0.8 | 1.0 | 6 | 0.75 |
| cross | r | head | 0.14 | 0.08 | 0.28 | 1.0 | 1.4 | 8 | 0.80 |
| hook | l | head (from the side) | 0.18 | 0.10 | 0.36 | 1.25 | 2.2 | 10 | 0.70 |
| charged | r | chest | 0.12 | 0.10 | 0.45 | 1.8 → 2.6 by charge | 4.5 | 20 | 0.85 + 0.5 m step |

- Time scale = `clamp(1.5 / attack_speed, 0.5, 1.5)`, applied to startup and recovery (not active).
- The combo is jab → cross → hook, then back to jab.

**Damage**
- `raw = attack_damage * mult`.
- Crit: if `randf() < crit_chance`, multiply by 1.75. The randomness is seeded per fighter so it is testable.
- `dealt = target.sheet.take_physical(raw)`.
- Heal the attacker by `dealt * lifesteal`.
- Guarded: the target takes 20% of the damage and loses stamina equal to `0.5 * raw`. If stamina runs out → GUARD_BREAK (stagger for 0.8 s).
- Parried: the attacker goes into STAGGER for 0.9 s.

**Poise and reactions**
- `eff = impact * (1 - tenacity)`.
- `eff < 1.2` → flinch: a MotionIK hit-recoil spring in the hit direction, plus a BodyRig strength dip to 0.6 for 0.15 s on the struck part's chain, plus an impulse on the struck part.
- `eff ≥ 1.2` → STAGGER for 0.45 s: the body is pushed back about 0.4 m by stepping (the foot planner takes steps; no sliding), and the target can't act.
- `eff ≥ 3.5`, or the target is already staggered and `eff ≥ 2`, or HP hits 0 → **knockdown**: `start_ragdoll(impulse)`, followed by the normal get-up.

**Hit-stop**: 70 ms (heavy: 90 ms). Implemented as `Engine.time_scale = 0.05`, restored from a real-time timer (`Time.get_ticks_msec`). Single-player only for now.

## 3. Strike motion: maths first, clips as style

- The upper body plays the pack clip (Punch_Jab, Punch_Cross, Melee_Hook) on the `upper` OneShot, timed to the move's phases.
- On top of that, MotionIK `strike` IK drives the striking hand along a computed path:
  - guard pose (fists by the chin)
  - wind-up (slight retraction)
  - an extension that meets the aim point on the target (head or chest), clamped to arm reach
  - a retract back to the guard pose
- **Body mechanics:**
  - The torso twists around spine_01 toward the striking side.
  - The pelvis shifts weight forward.
  - The hook arcs in from the side (aim point offset ±0.35 m laterally, then swept in).
- While in GUARD / combat idle, both hands hold the guard pose (forearms in front of the face) via the same IK.
- The fist rigid body follows the target. When it meets the target's body, physics stops it at the surface: no clipping, and the contact pushes the target.

## 4. Hit detection

- During ACTIVE, the striking hand's motion is swept each physics step (`PhysicsShapeQueryParameters3D` with a 0.06 m sphere from its last position to its current one) against `L_BODY` parts of OTHER fighters.
- The first hit per strike registers. It records the struck part (head / chest / pelvis / limb) and the contact point and normal.
- Hit-zone multipliers: head ×1.15, chest ×1.0, limbs ×0.85.
- Guard check: the target is in GUARD and the attack comes from within ±70° of the target's facing.

## 5. Lock-on: `game/lock_on.gd` (`class_name LockOn`, Node child of Player)

**Acquiring a target**
- MMB toggles lock-on.
- The candidate is the nearest fighter within 15 m and inside a 60° cone around the camera's forward direction. Score: `angle_deg + dist * 2`, lowest wins.
- When not locked, a **soft target** (the same pick within a 35° cone) aims strikes but does not move the camera.

**While locked**
- The mouse wheel cycles to the next or previous target by screen-space angle (it zooms the camera when not locked).
- The lock drops when the target is over 20 m away, or line of sight is blocked for more than 1 s, or the target is knocked out or dead. On a lost target, the lock moves to the next candidate if there is one.

**Camera** (camera_rig `lock_target`)
- Yaw eases (exponential, rate 8) so the target sits about 20% off-centre with the player in front.
- Pitch eases to -0.2. Zoom becomes `4 + 0.3 * dist`.
- Mouse look nudges yaw by ±30°, and it springs back.

**Movement while locked**
- `facing` stays toward the target.
- WASD is relative to the player→target direction, giving strafe-circling at 0.75× move speed.
- Sprint breaks facing temporarily.
- Space = side-hop: 1.2 m in the input direction, 0.25 s, 8 stamina, no i-frames.
- Alt = directional dodge relative to the target.

## 6. Opponents in the arena

- **Training dummy** at about (0, 0, 22):
  - a Player with `ai = "dummy"`
  - never attacks; stands in a guard-less stance
  - regenerates to full HP 3 s after its last hit
  - Human, level 10.
- **Sparring partner** at about (6, 0, 22), with `ai = "spar"`, a Human at level 8. Behaviour:
  - Approaches to 1.6 m and circle-strafes.
  - Attacks with a light combo at an interval of `rand(0.8, 1.6) / (attack_speed / 1.5)` seconds.
  - Guards with 35% probability when the player is in WINDUP within 2 m.
  - Dodges back with 15% probability on a charged wind-up.
  - Backs off below 30% HP. Uses the same Combat / LockOn components through its Intent.
  - Is reset by F2 restore.

## 7. HUD

- When locked: a reticle on the target's chest (screen-projected), plus the target's name and HP bar at the top centre.
- When soft-targeting: a small reticle.
- Toasts for **Parry!**, **Guard broken**, **Flurry**, **Knockdown**.
- The charge level shows as a ring around the stamina ring.

## 8. Self-test (`t_combat`, plus additions to the pop/penetration checks)

1. MMB locks the dummy in front of you. Nothing locks beyond 15 m or behind you. The lock drops beyond 20 m.
2. The wheel cycles between the dummy and the sparring partner.
3. A jab hits the dummy: damage = `attack_damage * 0.8 * zone * 100/(100+armor)` ± 1, with crit disabled in the test.
4. LMB ×3 lands jab, cross, hook within the expected total time ± 10%. With Shadowrush (attack-speed buff), the total time scales by the expected ratio ± 10%.
5. Guard: damage is 20% ± 2%, and stamina drops. Holding guard with no stamina ends in GUARD_BREAK.
6. A parry pressed within 150 ms of the hit staggers the attacker. Pressed at 300 ms, it does not.
7. Dodge i-frames: a strike during the i-frames deals 0 damage. A perfect dodge grants the Flurry modifier, which is visible in the attack_speed breakdown.
8. A fully charged strike staggers the dummy. A charged strike on a staggered target knocks it down: it ragdolls, then gets up and stands.
9. No penetration: during strikes, the attacker's hand and forearm bodies are never more than 2 cm inside any of the target's part shapes. Fighter capsules never overlap.
10. Hit-stop: `Engine.time_scale` returns to 1.0 within 120 ms real time.
11. The sparring partner, left alone with an idle player for 6 s, lands at least 2 hits. When the player throws 10 jabs at it, it guards at least once.
12. A locked strafe keeps the distance to the target within ±0.3 m over a 3 s circle at constant input.
