# Handoff: combat and animation rebuild (October 2026)

Read this before changing combat, animation or the body. It covers what exists, how to measure it, and what to do next.

## What is in place

**Animation core** (`game/hero_anim.gd`, `game/camera_rig.gd`, `game/player.gd`)
- The AnimationTree evaluates every rendered frame, not only the 60 Hz physics tick. Root motion is gathered per frame and
  handed to physics as an average velocity. Before this, on a 144 Hz screen the pose changed on only half the frames.
- The camera follows the body's interpolated transform in `_process`. It is opted out of physics interpolation and does its
  own sphere-cast collision.
- Locomotion loops share one gait timeline, starting on the left heel strike, so blends never cross the legs. The cycle rate
  comes from ground speed over the measured stride, so feet do not slide.
- Controller: turn rate falls with speed, so runs carve arcs. Reversing at speed skids, then pivots. A pivot from a stand
  turns before accelerating.
- After the animation: lean into acceleration and turns, two-bone IK foot planting, and no foot below the floor.
- `play_upper(clip, speed)` plays an upper-body one-shot over the moving legs. Casting uses it.

**Body** (`game/body_parts.gd`, `game/hero_anim.gd`)
- `HeroAnim.body_style = "mannequin"` is the default; `"skinned"` restores the UBC mesh.
- The mannequin is 17 capsules between bones, sized from bone lengths. The same capsules are the melee hurtboxes.
- Hit zones scale damage: head x1.2, torso x1.0, limbs x0.85.
- Armour shells come from `sheet.equipment`: helmet, chest plate with pauldrons, gauntlets and bracers, boots. They are
  coloured by material (leather, iron, steel, gold, cloth) and rebuilt when the equipment changes.

**Combat** (`game/combat.gd`, `game/projectile.gd`, `game/hud.gd`, `game/fighter_ai.gd`)
- No staff in the weapon cycle (Q). Every character casts with any weapon:
  - F tap: the light spell. F hold: the heavy spell. X switches element.
  - Mana costs and cooldowns are in `weapons.json` under `"_cast"`.
- The hotbar shows the light and heavy spells (key, element colour, mana, cooldown sweep) next to Art slots 1-4.
- Parry everything: guard pressed within 0.25 s before any blow lands (melee, monster lunge or projectile) parries it.
  - The last 0.10 s is a PERFECT parry: a 1.8 s stagger, toast, spark and slow motion.
  - A guard that is simply held blocks for chip damage.
- Projectile deflection needs a shield in the offhand or `spell_power >= 40` (`"_parry".deflect_spell_power`).
  - The new direction is the guarding hand's velocity, blended toward the shooter: 45% for a normal parry, 85% for a perfect one.
  - A perfect deflect flies 1.5x faster and homes. The projectile becomes the defender's.
- Spar AI parries with a chance from 12% to 70% by rank.
- `docs/COMBAT_FRAME_DATA.md` holds the startup/active/recovery table, generated from the clips.

**Goblins and sound**
- Goblins come in packs of 6:
  - The "Goblin frame" gives health x0.3, movement x1.3 and attack speed x1.3.
  - Pack AI: at most 2 attack at once (attack tokens). The others circle, feint and dart in.
  - Hurt goblins back off, and a leash walks them home. The AI sidesteps obstacles when stuck.
- `tools/make_sfx.py` builds every cue from the Kenney CC0 recordings in `assets/audio`, layered and processed, with
  synthesis as a sweetener. It needs `pip install numpy scipy soundfile`.
- Every cue is levelled to about -16 LUFS. `assets/audio/CREDITS.txt` lists the source recordings, and `docs/SOUND_UPGRADES.md`
  recommends free CC0 packs.

## How to measure

- Self-test: run `Godot --headless --path . -- --selftest [--only=...]`. A full run takes about 10 minutes, and every check passes.
  - Movement and animation: move, dodge, melee, flow, anim.
  - Combat: guard, ranged, magic, cast, parry, sparparry, framedata, body.
  - Monsters: mobs, pack.
  - Sound: sound, mix.
  - Other systems: ai, ui, grid, quests, shop, rank, duel.
- Motion lab: `tools/motion_lab.gd`; usage is at the top of the file. It plays the real game at 144 fps rendering over
  60 Hz physics and prints, per scenario (run, start/stop, turn, strafe, combo, dodge):
  - judder: frames where the camera or pose did not change
  - jitter: screen, body and hand jerk
  - pops
  - planted-foot slide
  - turn smoothness

  Run it before and after any animation change.

## Next steps, in priority order

1. **Play-test feel on a real screen.** The numbers say smooth; only hands on a controller say "good". Note each moment that
   feels wrong and check it with the motion lab.
2. **More bespoke clips.** This is the gap to Breath of the Wild. Download these from Mixamo, retarget them with
   `tools/retarget.gd` and add them to `clips.json`:
   - run-to-stop, turn 180 / pivot, start-run, walk-to-run
   - per-weapon attack recoveries and hit reactions (front, back, left, right)

   The locomotion code already measures clip speeds and strides.
3. **Sound listening pass.** The cues have never been heard on speakers. Adjust the dB values in `game/sfx.gd` CUES; the
   ones to check first are parry, clang, swish_* and the birds.
4. **Hurtbox tuning.** `HURT_PAD 0.3` in `combat.gd` adds a margin so existing reach still lands. Tighten it after retuning
   move ranges.
5. **Combat gaps:**
   - Projectiles still use the single body band and always count as torso.
   - Bow users cannot parry: the bow's guard button aims.
   - F casts mid-air and mid-dodge. Decide whether that is wanted.
   - Parries are rare in AI-versus-AI trades, because a fighter mid-swing cannot parry.
6. **Staff remnants.** The `magic` stance still exists in `weapons.json` because `main.gd` (the referee NPC and the demo tour)
   uses it. Move those NPCs to a weapon plus F casting, then delete the stance.
7. **Goblins** still use the human mannequin scaled to 0.78. A hunched goblin silhouette (longer arms, big head) is cheap
   with the capsule body: scale the segments in `body_parts.gd`.

## Standing direction from the user (carried over from the previous handoff)

These came from an earlier version of the game (the physical-ragdoll body, now replaced by `hero_anim.gd`) but still describe
what the user wants:
- The character is a solid object: nothing clips through anything (world, props, its own limbs).
- Prefer maths-driven, procedural motion over a canned clip firing every time; clips can be a style layer.
- Stairs: the leg lifts and the foot goes on top of the step and stays there if the player stops. Body height comes from the
  planted feet, not a capsule teleport.
- Efficient colliders. Work quietly, verify with numbers, and report results rather than every step.
- Never buy packs or create accounts. Free downloads are fine.
- Godot on the user's PC: `D:\Godot\Godot_v4.7.2-stable_win64_console.exe`.
