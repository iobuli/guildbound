# Guildbound: make it the best game

Decision: stay on Rust/Bevy 0.19.1. Build thin, game-agnostic layers on top of Bevy (like `gbui`). Measure before optimising. Verification bar is `cargo build -p guildbound` (no test runs, no subagents unless the user says "ultracode").

## Corrections to the first analysis
- Grass and armour are already optimised (grass: chunk LOD, `VisibilityRange` cross-fade, no prepass/shadows, one wind buffer; armour: warm-started fit, built on the async pool). Do not re-optimise them before profiling.
- Camera has default MSAA and no bloom/SSAO/atmosphere/probes; render setup is only in `world.rs::spawn_sky_and_camera`.
- No frame-time data existed. Hot spots are unmeasured.
- `footstep_grass_*.ogg` are in `assets/audio/kenney` but nothing plays them.
- No `.cargo/config.toml`, no `[profile.release]`, no fast linker.

## Build order
1. **Dev loop + `gbperf`**: `.cargo/config.toml` (fast linker), release profile; `gbperf` crate: F3 panel (frame time, per-system time, entity counts, fixed-tick overruns), baseline to `staging/perf.log`.
2. **Footsteps / audio**: sounds from foot-contact events by surface, landing thuds, cloth, ambience.
3. **`gblook`**: hot-reloaded `assets/ui/look.json` (tonemapping, bloom, SSAO, atmosphere, fog, probe, MSAA vs resolution scale, Low/Med/High). Replaces inline setup in `world.rs`. Includes a **reflection probe** (captured environment map) so the plate armour reflects sky and courtyard, and a **planar reflection** for the pond (a mirrored low-res camera rendered to a texture; Bevy's SSR needs the deferred renderer, our grass is forward). **Playtest pause.**
3b. **`gbworld`** (procedural placement, video ch. 5): props carry tags (`flat`, `near_water`, `under_trees`, `off_path`, cluster sizes), areas carry tags (meadow, shore, courtyard, ridge), a seeded deterministic placer builds the world by rule; "kits" place sets together (camp = fire + log seats + pot). Moves repeated hand placement out of `world.rs` and scales to a bigger map.
4. **Fix what `gbperf` shows**, nothing else.
5. **`gbphys` + ragdoll**: pure stable-PD / capture-point maths ported from `crates/sim` (`phys_balance`, `phys_gait`, `phys_ragdoll_fix`); `play/ragdoll.rs` per its header (Avian `AngularMotor` on hinges only, own PD torque on spherical joints); reflexes (catch step, brace arms). Hit trigger exists at `combat.rs:2151`. **Playtest pause.**
   - Basis: Reil & Massey 2001, "Biologically inspired control of physically simulated bipeds" (Theory in Biosciences 120; Reil founded NaturalMotion/Euphoria): a central pattern generator + simple reflexes + suitable mechanics walk without a "brain". Take: reflexes as cheap sensor rules (foot contact lost, COM speed, torso tilt -> catch step / brace); a two-phase stumble oscillator alternating the legs for 2-3 steps on survivable hits (the Euphoria stagger); mechanics (limits, masses) right first.
   - Control laws from Geijtenbeek, van de Panne, van der Stappen 2013 ("Flexible muscle-based locomotion for bipedal creatures", TOG), without the muscles: (1) feature control by Jacobian transpose: PD virtual force/torque on a few featured bodies (trunk orientation, head position+orientation, swing leg), mapped down the chain to a stable root, per joint tau = (P_body - joint) x F + T; (2) SIMBICON hip target theta = theta0 + kp*d + kd*v for foot placement (same idea as `sim::phys_gait`); (3) per-leg FSM stance -> lift-off -> swing -> stance-prep on contact and stance-foot-to-COM distance (event-driven stumble steps, replaces a free-running oscillator); (4) 5-20 ms sensor delay (1-2 ticks ring buffer); (5) first-order lag on joint strength, da/dt = 100 (u - a).
   - Not taking: Hill muscles, muscle routing, CMA-ES at runtime.
   - Later: tune gains and reflex thresholds offline with a headless `tools/gb_lab` run over thousands of simulated hits (their GA did this), instead of by hand. Modern reference: Geijtenbeek et al. 2013, muscle-based bipeds.
6. **`gbfx`**: wetness mask on characters/armour, fake refraction, sprite campfire; fold `fx.rs` particles in.
7. Weather + day/night, save/load, settings menu (gbui), headless regression bot (F4 monitor invariants), impact-feel pass, cape/hair cards on `secondary.rs`, NPC reflexes + gibberish speech.

## Skip
Solari (NVIDIA-only, no skinned meshes/custom materials), strand hair, fluid-sim fire. Bevy 0.20 upgrade later, on its own (WGSL→WESL for `grass.wgsl`, `glass.wgsl`, `ring.wgsl`).

## Rust question
Not limiting. Limits are Bevy's (no editor, ~quarterly API breaks). Unreal would need the engine layer rewritten (about half the code); reconsider only if the look pass still leaves lighting/hair as the blocker. Keep core maths as pure functions.

## Rules
One crate per layer, game-agnostic; add each to `docs/HANDOFF.md` + `handoff.json`; pause at the playtests marked above.
