# Physics debug overlay - hook (box only, not applied to the game)

Files (new): `debug_overlay.rs` (generic plugin, no game imports), `guildbound_source.rs` (game adapter).
Default ON. F3 all, F4 contact, F5 COM + capture point, F6 support polygon, F7 balance, F8 joint torque
(green -> red by |tau|/cap), F9 text, F10 voxel boxes + debris, F11 per-model labels.
Contact: on < 1 cm / off > 2.5 cm relative to rest_h (mean of first 60 planted ticks, then locked; pass6 v5b).
Draws in PostUpdate after TransformSystems::Propagate and PhysicsSystems::Last, in any game state.

## crates/game/src/main.rs
```rust
mod debug_overlay;
mod guildbound_source;
// with the other plugins (beside sandbox::plugin) - runs in gameplay too, not only F2:
app.add_plugins(debug_overlay::RagdollDebugOverlayPlugin::<guildbound_source::GameRigSource>::default())
   .init_resource::<guildbound_source::JointTorqueLog>()
   .init_resource::<guildbound_source::VoxelDebugFeed>();
```
Needs bevy features bevy_gizmos + bevy_ui + text (the game already has them).

## Rig names (hero, mannequin, any profile)
On each powered-ragdoll root spawn: `commands.entity(root).insert(guildbound_source::RigDebugInfo { name: "hero".into(), profile: rig_profile_name.into() });`
Rigs are found by the `ragdoll::Doll` component (+ the possessed `PhysicalBody` doll). If `Doll` is not a
Component in build2.0, add `#[derive(Component)]` to it or change the line marked ADJUST.

## Joint torque lines (optional, one line in the motor writer, sandbox.rs balance_tick)
`world.resource_mut::<guildbound_source::JointTorqueLog>().0.push((child_body, anchor_world, tau, cap));`
and clear it at the start of the tick (`.0.clear()`). Cap = 3*M*g safety cap or the per-joint cap.

## Voxel layer (optional)
Fill `VoxelDebugFeed.boxes` from `phys_voxel::merge_boxes(chunk)` (cell min/max * voxel size + chunk origin)
and `.debris` with each debris piece `(MassProps.com, MassProps.mass)` when chunks rebuild.

## Lightweight separate environment
`cd gb_lab && cargo run --release --features visual --bin visual` - stand scene, 12 substeps, kb 5, kbd 0.3,
Kp 4000 / Kd 126, foot damping 40. Space pauses physics, R resets the drift origin.
Box note: needs rustc >= 1.89 (toolchain ~/.rustup/toolchains/stable, 1.99).
