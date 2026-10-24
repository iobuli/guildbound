# Athorina Stage 1

Run `play.cmd` or `cargo run -p guildbound`. The executable is still named `guildbound` so existing launch scripts keep working; the window and prototype are Athorina.

## Scope

The running game contains one controllable character, a grey plane, and a solid wall. The player uses the retained kinematic capsule controller and the Kev motion bank. The old village, NPC, character creator, magic, combat systems, and F-key panels have been removed from this runtime. Their source is available in Git history; the animation and other assets stay in the repository.

The left panel is the new character and animation workspace. Select among four model presets, cycle their palette, page through the physical motion library, and click any clip to preview it on the current model. The library includes compatible UAL, Kev, Sekiro, Sparking rush, Trunks, and Tapion tracks. Magic, gun, bow, beam, blast, and cast clips are filtered from this prototype.

## Controls

| Input | Action |
| --- | --- |
| WASD | Move relative to camera |
| Shift / Alt / Ctrl | Sprint / walk / crouch |
| Space | Variable jump; hold toward wall to climb and mantle |
| Left mouse / K | Punch combo / kick |
| Right mouse drag / wheel | Orbit camera / zoom |
| M / N | Next model / next palette colour |
| P | Play or stop selected animation |
| Page Up / Page Down | Previous or next animation page |

The wall has a static collider. Strikes within reach add a dent to its rendered front surface and knock loose a few short-lived physical chips. The capsule controller, climb route, and camera collision keep the body and view outside the wall. The wall collider stays solid while the visual mesh dents; this stage is a controller and hit-feedback prototype, not destructible structural physics.

## Implementation

- `crates/game/src/play/athorina.rs`: stage, UI, attack timing, wall deformation, clip selection, climb and camera.
- `crates/game/src/play/controller.rs`: capsule movement, slope/step support, coyote jump, air steering, and collision slide.
- `crates/game/src/motion/`: retained rig binding, pose bank, retargeting and contact synthesis.

The local `profiles/feel.json` file is not loaded by Athorina. Its pre-existing worktree changes are intentionally left alone.
