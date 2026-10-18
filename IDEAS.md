# Ideas from the user (newest first)

Each entry is a direction to consider, not a verbatim order. Read at the start of every turn;
say on LOG.md which idea you picked up.

## 2026-10-18 16:12Z - agent-3 (Haiku helper, raw)

**Raw brainstorm from agent-3's Haiku helper.** It read the code but ran nothing. These are ideas to iterate on, not conclusions. Items marked (verify) are guesses from reading.

### E7 defeat ragdoll
**Feel**
1. Limp timing per clip. Measure when the pelvis first drops, instead of a fixed 0.3 s. Risk: a fixed time drifts if the clip speed changes.
2. Blend the last clip pose into physics over about 0.1 s. Risk: a long blend holds the body up over the terrain, which is the sinking bug again.
3. Slump along the killing blow's direction, not always backwards from the facing. Risk: falling toward a wall or a drop puts the body into geometry.
4. Drop the weapon on death (detach the held item). Needs the loadout owner to agree.
5. A final twitch at first settle: a tiny kick on the head or hand. Risk: it could lift a segment.
6. Scale the fling by blow strength on a kill, but not on the limp.

**Edge cases**
7. Death mid-air. Does gravity still apply to a dead body in Air mode? (verify)
8. Death in water. Segments have no buoyancy, and the ground ray may pass through the water volume.
9. Stairs. Check for segments hanging off a step edge, not just sunk ones.
10. Walls. Add a horizontal check for segments wedged in a wall.
11. Killed while already a ragdoll. Which owner does the death action get? (verify)
12. Mounted death. A rider's seated pose may spawn segments inside the horse collider.

**Robustness**
13. At `MAX_FLING` 18 m/s, a segment moves about 0.3 m per frame, more than a limb's radius, so check CCD or substeps.
14. The terrain trimesh is one-sided. Can a ray from below see the surface?
15. Segments can spawn inside geometry. Check the pelvis against the ground before `take_over`.
16. The check runs once. A segment sliding into a crack later is never rechecked.
17. A ray from 1 m up can start inside an overhang and mis-lift a segment. *(agent-3 fixed this: the ray now goes up from below the segment.)*
18. The lift kept the segment's velocity. *(agent-3 fixed this: velocity is zeroed on lift.)*
19. Sleep or freeze settled corpses to save CPU, tied to revive.
20. Cap the number of live corpses: bake the oldest into a static pose.

**Measuring**
21. A headless test sweeping slopes (0, 10, 25 and 40 degrees) and step heights, failing if any segment bottom is more than 0.03 m below the terrain.
22. Log how many segments were lifted and by how much. Big lifts mean the check is hiding a physics bug.
23. A histogram of time to settle, to catch bodies that never settle.
24. Frame cost of `drive` with 0, 10 and 30 corpses.
25. A screenshot diff of the slope scene.
26. A fuzz test: 200 random kills, with no segment ending under the surface.

### E8 god mode
1. **Key.** F11 looks free; F12 is the debug list.
2. **Placement.** Add it to the F12 list, or make a standalone panel.
3. **Tabs.** Self, target and world tabs. The target tab says so when nothing is locked on.
4. **Base.** Reuse the helper's stats panel and the `gbui` panel and label style.
5. **Tuning.** Keep it apart from the F1 tuning panel, which saves `feel.json`.
6. **Kill and revive.** Go through `kill()` and `Revive`, never write `hp` or `dead` directly.
7. **God mode.** Use an `Invulnerable` marker checked in `resolve`, so damage numbers still show.
8. **Infinite stamina.** Refuse `StaminaSpend` at the source.
9. **Noclip.** Use a flag the controller reads, rather than writing `MoveMode`.
10. **Time scale (hard conflict).** `apply_hitstop` overwrites `Time<Virtual>` speed every frame, so a debug time scale has to combine with it.
11. **Spawn.** Spawn enemies through `spawn_character` at the reticle point, on the ground.
12. **Rank and AI.** Set rank and AI difficulty through a config message, not mid-decision.
13. **Passive toggle.** Skips attack intents without changing the AI's state machine.
14. **Env vars.** `GB_GOD=1`, `GB_PANEL=1`.
15. **Scripted actions.** A JSON file of panel actions with frame numbers, for headless runs.
16. **Logging.** Every panel write logs a monitor line that tests can check.
17. **Release builds.** Gate god mode out of them.
18. **Monk counters.** God mode plus a Monk spawn plus time scale 0.25 gives a replayable counter scene. God mode must not disable the counter window.
19. **Rank AI.** Spawn each rank with fixed difficulty and watch `View::AiIntent`.
20. **Wardrobe.** Swap outfits live on self or the target. Needs agent-1's API.

### E6 look (from file names only)
1. **Grade.** One shared colour grade (LUT or post pass) for sky, terrain and characters.
2. **Fog.** One fog and atmosphere ramp, driven by the time of day in `sky.rs`.
3. **Palette.** Limit the generated textures to one palette at bake time.
4. **AO and shadows.** Bake AO for static props, and spend the RTX 3060 budget on character shadows.
5. **Cascades.** Cap shadow cascades and distance, A/B-tested with `View::ShaderMode`, measuring ms per setting.
6. **Quality tiers.** Put SSR and volumetrics behind a quality tier with a written ms budget.

### Open questions
**For agent-1:**
- Can a wardrobe swap change the bone count? `take_over` silently skips the ragdoll if it does.
- Is every cast member the same size, so the segment radii fit everyone?
- Do capes or hair have their own physics?
- Is there an API to change an outfit without respawning?
- Does every cast member use `ual1/Death01`?

**For agent-2:**
- Is `kill()` the only way to `dead = true`?
- Can a Monk counter a body during the 0.3 s limp?
- Who sends `Revive`, and does a revive during the death clip skip the ragdoll?
- Does `apply_hitstop` need to combine with a debug time scale?
- Does the horse also die, or only the rider?

## 2026-10-18 16:11Z - agent-3

**Reasoning, open for iteration (not decisions). @agent-1: please reason with me here; push back, add, branch off.**

### E7 death: what I think is happening, and the open questions
- An ordinary kill plays `ual1/Death01` on the still-standing capsule (`combat.rs die()`). The clip lies the body flat at the height of the capsule's feet. Any slope, step or rock under the head or legs is above that plane, so the body clips into it. Only blows above `ragdoll_threshold` ragdoll.
- My first cut, built and waiting for tests:
  - Go limp 0.3 s into the death clip, so the knees buckle, then physics lays the body on the real ground.
  - Check each segment against the ground when the body settles, and lift any that sank.
  - Add a 25° trimesh slope test.
- Questions I haven't settled:
  - **Limp timing.** Is 0.3 s right? It could depend on the blow: an instant limp for a heavy hit, a longer clip for a light one or a bleed-out.
  - **Fall direction.** Should the body fall toward or away from the killing blow? Right now it slumps backwards from its facing. The blow's direction may read better.
  - **The death clip.** Keep `Death01` at all, or start limp at once and add a short scripted "knees go" pose?
  - **Corpses and CPU.** On a CPU-heavy machine, should settled corpses freeze (static) after N seconds? Despawn after a fight? Cap the corpse count?
  - **The capsule.** It stays on the dead body while it's a ragdoll. Could it block other fighters? It's layer Character, and the ragdoll only collides with World.
- Idea from IDEAS (monks hum before a seize): if the hum is a sound plus an aura pulse, the same "presence" could make a corpse's area feel heavy, with crows and flies after a while (world-feel, E6). That's just a seed.

### E8 god mode: early shape
- The panel would reuse the helper's debug stats panel once `feat-bag-craft` lands.
- Each edit is sent as a message to its owning system (combat for hp, magic for pools, discipline for rank), never written straight in.
- Open question: should god mode also be scriptable with `GB_GOD=1 GB_SET_RANK=6 ...`, so agents can set up test scenes without clicking? That would speed up everyone's recordings.

### E6 look: a question for agent-1
- Your generated textures (skin, five cloth weaves) set the palette. Should the environment be tuned to match the characters, or the characters to a new environment palette? I lean toward the environment following your textures, since they exist now.
- What do the textures look like under the current sun, sky and fog? Any stills you've already taken would save me a render.

A Haiku helper is brainstorming E6/E7/E8 now. I'll add its raw ideas below this entry when it reports, unfiltered, for us both to pick at.

## 2026-10-18 16:02Z - user

ag_2 advise ag_3 on all pre required libs needed, wayland , plus whatever bevy needs next ,The first build failed near the end because this machine was missing system libraries that Bevy needs (Wayland, sound, input). Nothing was wrong with the game code. I’ve installed them, plus the software Vulkan driver for recording later. The build has restarted and reuses the roughly 600 parts already compiled, so it should be much quicker this time. Once it finishes, I’ll run the ragdoll tests

## 2026-10-18 15:57Z - user

that was a test

## 2026-10-18 15:57Z - user

monks hum before a seize enemies hear it and back off


