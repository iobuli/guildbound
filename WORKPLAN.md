# Guildbound work plan — for agent-1 to orchestrate

Written by agent-2 from the user's direction in its chat (their words were a stream of thought;
this is the plan made from it, not a transcript). Read `CONTEXT.md` first for rules, branches and
what exists. Every package below is an **event**: a self-contained piece of work with the idea,
how to build it, how to know it's done, and where it grows next. Hand them out as even work.

## Ground rules for every agent

- **Base:** `overhaul-2026-10-16` (now `fc15f67`). Branch per event: `ev/<id>-<short-name>`.
  Pull before starting, rebase before handing over. One agent owns a file at a time; claim it on
  the board ("taking combat.rs strike()") before large edits.
- **Rework is allowed.** Any agent may rework earlier code (combat states, discipline, looks,
  shaders, ragdoll, AI) if it makes the whole hang together better. Say so on the board first,
  keep behaviour that other systems depend on (or update those systems in the same branch), and
  keep the tests passing or replace them with better ones.
- **Cohesion over features:** one combat language (tells, windows, reactions) for every class; one
  look (shaders, textures, palette) for every surface; one debug surface for every system.
- **Checks:** build light (`CARGO_INCREMENTAL=0 nice cargo build -p guildbound -j 4`), test once
  per batch, verify in game headless (`GB_RECORD`, `GB_SCREENSHOT`) and look at the frames.
  Prefer measuring tools (fit checker, floor report, joint monitor) over eyeballing.
- **Hand-over:** push (or bundle → agent-2 pushes), then post with `post.sh`. Agents that can't
  push end their turn with a ready-to-post log line.

## Who

| Agent | Role | Events |
|---|---|---|
| agent-1 | orchestrator; wardrobe / cast / appearance | E9, E10, hands out the rest |
| agent-2 | combat, motion, AI, merging + pushing | E1, E2, E3, E4, E5 |
| agent-3 (new) | rendering, environment, death ragdoll, debug god mode | E6, E7, E8 |
| helper | bag, crafting, debug stats (`feat-bag-craft`) | E11 |

---

## E1 · Monk finger deflect: guard, perfect deflect, disarm, seize, finisher (agent-2)

**Idea.** The monk meets a blade with two fingers, placed on the blade's real path. Holding
guard is a *deflect*: the fingers ride the flat of the blade aside, damage is reduced (today's
`guard_bonus`). Timed *perfectly* (inside the parry window), the deflect turns into one of two
outcomes depending on the gap between monk and attacker:
- **Close in level/rank** → **Disarm**: the weapon is knocked out of the attacker's hand. The
  attacker keeps fighting bare-handed (they can still punch and kick).
- **Far lower** (gap ≥ 2 ranks, or the attacker's power below a % of the monk's, tunable) →
  **Seize**: a different deflect animation; the monk grips the blade, strikes the attacker, and
  the attacker **staggers weaponless** (no ragdoll) into a long *Broken* state. During it the
  monk can perform a **Finisher**.
This replaces the current counter → `Stunned` → ragdoll-on-next-hit rule for the Monk; rework it.

**How.**
1. *Blade path prediction:* combat already sweeps each blade as a capsule between frames
   (`combat::hit_test`, `sweep`). Add `predict_contact(attacker, defender) -> (time, point,
   blade_dir, edge_dir)`: sample the attacker's attack clip ahead through its `active` window
   (pose bank `sample`, weapon grip transform) and find where the blade crosses the defender's
   guard sphere. Do it when the attack's `Tell` starts.
2. *Finger placement:* while the monk guards against a predicted contact, drive the lead hand
   with an IK target at the contact point, palm turned so the two straight fingers (`Form::
   TwoFinger`) lie against the *flat* of the blade, pushing along the perpendicular of the edge.
   Use `HandTargets` + a `Grip` with `Shape::Flat` on the blade plane (hand rig already supports
   flat grips), time it so the fingers arrive at `time`. Head looks at the blade (`look_at`).
3. *Verdicts:* extend `combat::judge` with `Deflected` (held guard), `PerfectDeflect`. Resolve:
   `Deflected` = block with monk bonuses; `PerfectDeflect` → `discipline::deflect_outcome(monk,
   attacker) -> Disarm | Seize`.
4. *Disarm:* move the attacker's weapon entity off the hand into a physics pickup (rigid body,
   thrown along the deflect direction), set `Loadout` to unarmed (path recomputes already),
   short flinch. Weapon can be picked up again (F) by anyone.
5. *Seize:* paired animation: monk clip (grab + strike, from CMU blocks/punches: `144_07/08
   left_blocks`, `144_26/27 right_blocks`, `135_09 oiduki`, trimmed) and attacker reaction
   (stagger). Align both roots for the duration (a `Paired { other, offset }` component that
   pins relative transforms). The seized weapon ends in the monk's hand briefly, then is
   dropped or tossed. Attacker enters `Broken { left }` (3–4 s): no actions, guard down.
6. *Finisher:* while an enemy near you is `Broken`, a prompt; LMB/F plays a finisher (monk:
   chained CMU strikes ending in a knockdown; damage to 0 or heavy). Victim plays matching
   reactions; camera tightens; hitstop on the last blow.
7. Hands: `HandFlash` forms per phase (TwoFinger on contact, Fist on the strike, Cupped on the
   seize). Check with the fit checker (`GB_CHECK=1`) that fingers don't pass through the blade.

**Done when.** Tests: prediction finds the contact for each katana/greatsword attack within a
few cm; verdict tiers by timing and rank gap; disarm drops the weapon and the path changes;
seize leaves the attacker `Broken`, not ragdolled; finisher only on `Broken`. In game: a
recording of a monk vs the Opponent showing deflect, disarm, seize and finisher, with `[discipline]`
log lines for each.

**Grows into.** Every class gets its own perfect-defence tier (E2). Weapons on the ground become
real loot. Enemies learn disarms (repertoire). Finishers per weapon and per enemy kind (demons
resist seize, need a different finisher).

## E2 · Perfect defence for every class (agent-2, after E1)

Same rule, each class shown differently: **Blade** perfect parry → riposte cut or disarm by
binding the blade; **Hunter** perfect dodge → backstep shot / snare that roots; **Arcanist**
perfect ward → spell reflected or frozen attacker. Max rank (S) gets the strongest version
(already: flares per class in `discipline::Flare`). Share one tier engine
(`discipline::perfect_outcome(defender, attacker, kind)`) so tuning stays in one place.

## E3 · Mocap glue and combat frames (agent-2)

**Idea.** The CMU takes have the moves (monks especially); they need fitting, not more clips.
- **Per-attack metadata file** (`assets/anim/strikes.json`): for every library strike, its trimmed
  range, `windup / active / recover`, hit limb, reach, and which classes use it. Generate it with
  the existing limb-speed peak finder (`repertoire.rs`, `clip_probe`) and keep hand edits there.
- **Glue:** blend-out of one strike into the next by matching the end pose of A to start of B
  (pick chains whose poses are close; add short procedural transitions where not).
- **Head and hands fitting:** CMU has no fingers and a stiff head: forms (fist/cupped/two-finger)
  by strike type, head looking at the target, neck stabilised.
- **Fidelity by rank** (already started: `discipline::variety`): low ranks repeat a few moves with
  slower recovery; high ranks chain library moves smoothly; monks use the widest set.
- **Checks:** floor report (done), plus a foot-slide report and a wall/self-penetration report in
  the fit checker.

## E4 · AI around rank (agent-2)

Unique *hesitation* animations when an enemy faces a higher rank (feints, a stutter step, a
guard check, a backward glance), and flee runs that look like panic (look back, stumble). Demons
"try their luck" with taunts. Uses E3's library; data in the same strike/feint table.

## E5 · Climbing and parkour (agent-2)

Distinct climbing states (hang, shimmy, climb up/down, corner wrap, hang-to-hang jump, drop to
hang, back eject to a wall behind), camera-relative stick, speed-gated chains (vault at running
speed). Body stays off the wall (`body::body_out` done); add knees/feet to the wall checker and a
recorded climb test that reports maximum wall penetration.

## E6 · Shaders and environment, one look (agent-3)

**Idea.** Everything has looked the same for a while. Make the world beautiful and make the
shaders fit the textures we generate, so the whole thing reads as one product.
- **Audit** `assets/shaders/` (skin.wgsl ink/toon/presence, ground.wgsl, weather.wgsl,
  wind_aura.wgsl, grass) and the generated textures (`tools/model/textures.py`: skin, eyes,
  hair, five cloth weaves + normals) with their UV layouts. Make one lighting model and one
  palette: ink bands, rim, AO, normal maps all read the same way on skin, cloth, stone, wood, grass.
- **Shaders tuned to the textures:** sample the generated normal maps and weave patterns
  properly (UV scale per material, texel density), cloth sheen per weave, skin subsurface
  approximation for the ink look, triplanar for terrain/rock with the same band ramp.
- **Environment pass:** terrain variety (paths, slopes, rocks, water edge), foliage density and
  variety, better sky/fog/sun colour over the day, props with weathering everywhere, the ruins and
  guild areas dressed to match their outfits' palettes (agent-1's area styles).
- **Budget:** RTX 3060 at 1080p: keep frame time; add LOD/culling where needed.
**Done when:** before/after stills of 6 fixed spots (guild, village, pond, ruins, road, close-up
of a character) in a contact sheet; no shader validation errors.

## E7 · Defeat ragdoll: bodies sink into the ground (agent-3)

**Bug.** When a character dies, the body doesn't fall onto the ground cleanly: it ends up partly
**under the terrain**. Find out why and restore a proper *defeat ragdoll*.
**Where to look:** `play/ragdoll.rs` (segment spawn positions vs the capsule's `HALF_HEIGHT`
offset, collision layers of the segments vs `Phys::World`, the terrain collider type (heightfield
/ trimesh) and thin-surface tunnelling — try CCD/substeps), `combat::kill` (death pose vs
ragdoll handoff, whether the kinematic capsule is removed or still pushes), and the get-up / stay-
down logic. Note: `GB_FLING=1` and F6 trigger ragdolls for testing.
**Done when:** a death (and a heavy knockdown) settles with every segment on or above the ground
(log the lowest segment height vs `ground_height` after rest; add a test), recorded from the side.

## E8 · Debug god mode (agent-3, builds on the helper's debug stats panel)

A panel (e.g. F11) to **play with the systems**:
- **Self:** god mode (no damage, no death), infinite stamina / Battlewill / mana, noclip, set
  level, attributes, class, path, rank (`GB_RANK` live), learn any art/spell/skill.
- **Enemy (lock-on target or nearest):** edit hp/poise/stamina, stats, rank/path/species, AI
  difficulty (reaction, aggression), make passive, kill, revive.
- **World:** spawn an enemy of a chosen kind/weapon/rank in front of you, time scale, peaceful,
  time of day.
Writes go through the owning systems (messages or the component's owner), not around them.

## E9 · Cast checks and showcase (agent-1)

Close-ups of every named character and class outfit, fixes for clashes (pieces vs garments,
colours too close, clipping), plus a showcase video. Include the monks (order style), the Oni and
the Monk class now that they are on the base.

## E10 · Looks follow the fight (agent-1, with the helper's bag style)

The player's outfit and bag look change with how they fight (`BagStyle`): colours and wear by
dominant skill, rank marks (a sash colour per rank), monk prayer beads by rank. One appearance
source of truth.

## E11 · Bag, crafting, debug stats (helper → merge by agent-2)

`feat-bag-craft`: satchel tabs (gear, items, quests, header with gold/level/rank/class), crafting
(table, furnace, blueprints; skills lift each limit; an evolved skill lifts all and lets rare
materials add properties), debug stats panel. Merge after build + tests; E8 builds on it.

## E12 · Posture follows what's held (agent-1)
`WeaponDef.mass` → new `play/carry.rs` computes `Load` (each hand, back, hips) → `select.rs` `Posture` → a carry block in `compose.rs` before IK: holding shoulder drops, free arm balances, chest forward under a back load, wider stance and bent knees with mass. Tests: lean direction, stance grows with mass, empty hands unchanged.

## E13 · Spell aiming: crosshair, onion-skin previews, world marks (agent-1)
Hold E/Q to aim (shared HUD reticle), release to cast; a tap casts as today. A ghost preview at the target per shape: Bolt arc + impact ring, Quake ground ring, Step = translucent caster at the destination. Lasting marks (capped, fade over 20–40 s): cracks, scorch, frost, char. Files: `magic.rs`, new `spell_aim.rs`.

## E14 · Attack speed that keeps the strike (agent-1) · DONE in `int/E7-E8-E14` (281/0)
`Derived.attack_speed` × rank; the swing is time-warped per phase (windup ×s, strike ×s^0.3, recovery ×s^0.8), with a 2-frame anticipation hold above ×1.15 and afterimages (`afterimage.rs`). Hooks for E8: `Melee::shield`, `Melee::refresh`.

## E15 · Baked combo transitions (proposed: agent-2 owns, agent-1 tools) · needs agent-2's yes
Everything heavy offline; the game only plays baked data.
1. **Tag** every clip (UAL, Rokoko, CMU BVH, Sekiro) from numbers our tools extract: root speed, foot contacts, hand height, weapon arc, energy → windup / strike / recovery / feint / stance / weight. A helper model (kev-4b if it fits; still unchecked, huggingface.co is blocked here) may label and propose; numbers stay ours.
2. **Combo graph** → `assets/anim/strikes.json` + repertoire: which move follows which, timing windows, variants (feint, delayed hit, finisher), style per enemy. Reviewed, then committed.
3. **Bake** (Python, `tools/anim/`): per graph edge, best cut frame by pose + velocity match; short blended transition with foot lock, ground contact, `Grip` targets, and the E14 warp per speed tier.
4. **Check**: score foot slide, ground clip, joint pops, blade through body, balance; reject/retry; `GB_RECORD` contact sheet of the worst only.
5. **Game**: a seeded picker walks the graph; `motion/bank.rs` plays clips + transitions.
Storage: quantised rotations, transition frames only (disk is tight). First slice: one weapon's combos, before/after recordings.

## E16 · Art polish through Higgsfield (agent-1) · `docs/ART_PROMPTS.md`
One asset per run, free generations only. Textures via `tools/model/ai_texture.py` (lattice, straighten, phase lock, seam fade, height from albedo). T1 linen done: branch `art/T1-linen`. Next T3 leather, T2 wool twill; then concepts C1/C5/C6 from in-game shots, UI mock-ups, poses for E12–E14.
