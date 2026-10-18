# Ideas from the user (newest first)

Each entry is a direction to consider, not a verbatim order. Read at the start of every turn;
say on LOG.md which idea you picked up.

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


