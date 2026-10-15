# Applying pass 6 (v5b) — for daviy

All paths are relative to the repo root (branch build2.0). Close the game first. Commit or stash your work before you start.

## 1. Copy files
| From the box (/workspace/pass6/...) | To the repo |
|---|---|
| game/sandbox.rs | crates/game/src/sandbox.rs (replace) |
| game/studio.rs | crates/game/src/studio.rs (replace; keep its CRLF line endings) |
| game/body_cmds.rs | crates/game/src/body_cmds.rs (replace) |
| sim/phys_balance.rs | crates/sim/src/phys_balance.rs (replace) |
| sim/phys_stance.rs | crates/sim/src/phys_stance.rs (replace) |
| sim/anim_acro_active.rs | crates/sim/src/anim_acro_active.rs (new) |

## 2. Delete
- crates/game/src/sandbox_core.rs. Nothing uses it. Also delete its `mod sandbox_core;` line if there is one.

## 3. Hook lines
1. crates/sim/src/lib.rs: add `pub mod anim_acro_active;`.
2. crates/game/src/main.rs: change `// mod sandbox; // disabled: ...` to `mod sandbox;`.
3. crates/game/src/studio.rs: if the line `// app.add_plugins(crate::sandbox::plugin);` is still commented out, uncomment it.
4. crates/game/Cargo.toml: avian3d needs the `debug-plugin` feature. It is on by default, so this only matters if you have
   `default-features = false`. In that case add `"debug-plugin"` to the features list.
5. Optional: in the code that plays an acro clip, call `world.resource_mut::<crate::sandbox::Sandbox>().start_acro(len_ticks)`.
   This fades the COM pull during the move. Without the call, acro_active stays 0.
6. Optional: in the gb.studio remote tab handler, replace the direct `studio.tab = i` with `studio.set_tab(i)`. This allows tab 9 (Pose).

## 4. Launch
- From the repo root, run `cargo run` the way you normally do. No environment variables are needed.
- The game opens in Studio sandbox mode with the stand test on. The skeleton overlay and the Avian debug gizmos are on. Built-in settings:
  - Kp 4000, Kd 126, kb 5, kbd 0.3, 12 substeps
  - foot damping 40 (absolute)
  - COM pull 0.11
- Logs go to target/live/metrics.jsonl and torque_sum.jsonl.
- Optional overrides: `GUILDBOUND_COM_PULL=off` (or a number), `GUILDBOUND_FOOT_DAMP=rel|off`, `GUILDBOUND_STAND=off`.

## 5. If it does not build
- The box only type-checked against stubs, so there are 3 things to check:
  - the real `Studio` has `show_skeleton`
  - `crate::props::physics_clock` exists
  - the ragdoll and possession APIs match
- To undo everything: `git checkout -- crates` and restore sandbox_core.rs.
