Staged fragments (box only). The repo copies on daviy's PC may already contain these: the last PC command
(write ui_theme/body_controller/body_cmds/lib.rs edits, then cargo build) was interrupted mid-run, so its state is unverified.
- crates/game/src/ui_theme.rs.append.rs   : append to the existing ui_theme.rs (the API additions)
- crates/game/src/body_controller.highlight.rs : replaces fn highlight in body_controller.rs
- crates/game/src/body_cmds.torque.rs      : replaces placeholder torque_limit in body_cmds.rs (+ chain code in ragdoll_motors)
- crates/sim/src/lib.rs.lines              : module lines
- crates/game/src/sandbox.rs + sandbox_core.rs : NEW. Sandbox as a Studio start-up mode (no main.rs boot path). In studio.rs's plugin: app.add_plugins(crate::sandbox::plugin); Physics tab calls sandbox::physics_tab. sandbox_core.rs is pure and compiles standalone. Needs Physics' updated balance_torque(mass, h, ...). See TODO(verify)
- copy /workspace/phys/phys_balance.rs -> crates/sim/src/phys_balance.rs (compiles standalone with rustc, 0 warnings)

## F2 Studio restyle + panel kit + trails (00:45, built against /workspace/ui_src post-edit files)
- crates/game/src/ui_theme.rs       FULL REPLACEMENT (HUD Layer Lab). All old public names kept. Supersedes ui_theme.hud.rs and ui_theme.rs.append.rs (do NOT apply those).
- crates/game/src/studio.rs         = ui_src/studio.rs + kit hooks (CRLF kept). Diff: `diff /workspace/ui_src/studio.rs crates/game/src/studio.rs`
- crates/game/src/sandbox.rs        = ui_src/sandbox.rs + ramp restart on BalanceSet{on} + restyled physics_section (same toggle, not duplicated)
- crates/game/src/studio_kit.rs     NEW panel kit (+ studio_kit_core.rs pure, rustc-tested)
- crates/game/src/studio_trails.rs  NEW motion trails (gizmos only, never logged)
- main.rs.lines: mod studio_kit; mod studio_trails; + their plugins
- sim lib.rs needs Animation's: pub mod anim_rig; pub mod anim_trail_target; pub mod anim_flow_cut; (from /workspace/anim_staging)
