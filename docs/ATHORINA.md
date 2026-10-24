# Athorina Stage 1

Run `play.cmd` or `cargo run -p guildbound`. The executable is still named `guildbound` so existing launch scripts keep working; the window and prototype are Athorina.

## Scope

The running game contains one controllable character, one sparring target, a grey plane, and a solid wall. The player uses the retained kinematic capsule controller and motion bank. The old village, character creator, magic systems, and F-key panels have been removed from this runtime. Their source is available in Git history; the animation and other assets stay in the repository.

The left panel is the new character and animation workspace. Select among four model presets, cycle their palette, page through the physical motion library, and click any clip to preview it on the current model. The library includes all 32 Kevin Iglesias melee clips, UAL, Sekiro, Sparking, Trunks, and Tapion tracks. The 42 male clips from the locally owned Kevin spellcasting pack are also available as **animation previews only**; the game still has no magic system. Generated brawler, karate, and shonen duel clips are excluded. When the machine-local Sparking cache is present, its converted clips and complete IN/HT/OT rushes load automatically at 60 Hz. Third-party extracts remain outside Git. Gun, bow, beam, blast, and unrelated cast clips are filtered from this prototype.

## Controls

| Input | Action |
| --- | --- |
| WASD | Move relative to camera |
| Shift / Alt / Ctrl | Sprint / walk / crouch |
| Space | Variable jump; hold toward wall to climb and mantle |
| Left mouse / K | Punch combo / kick |
| Tab or middle mouse | Toggle lock-on to the sparring target |
| Right mouse drag / wheel | Orbit camera / zoom |
| MB4 / MB5 | Previous / next motion and play it |
| M / N | Next model / next palette colour |
| P | Play or stop selected animation |
| Page Up / Page Down | Previous or next animation page |

The wall face is a sampled damage field. Strikes from the animated hand or foot bones carve craters and branching cracks, and rebuild a static triangle collider from the same vertices. A thin backing collider keeps the wall solid. The sparring target reacts to head, side, and body impacts, recoils within the stage bounds, and faces the player. Lock-on adds a floor ring, keeps the camera framing both characters, turns the player toward the target, and uses Kevin Iglesias directional strafe clips. Attack presses buffer into the next combo section; contact briefly holds the pose and pushes both bodies apart. The capsule controller, climb route, and camera collision keep the body and view outside the wall. The stage uses flat unlit materials without custom shading. This remains a controller and hit-feedback prototype, not full structural physics or a per-triangle animated character collider.

## Implementation

- `crates/game/src/play/athorina.rs`: stage, UI, attack timing, wall deformation, clip selection, climb and camera.
- `crates/game/src/play/controller.rs`: capsule movement, slope/step support, coyote jump, air steering, and collision slide.
- `crates/game/src/motion/`: retained rig binding, pose bank, retargeting and contact synthesis.
- `tools/model/kevin_spell_pack.py`: converts a local Kevin Iglesias `.unitypackage` through Blender into the ignored preview cache. Run it with the package path as its argument.

The local `profiles/feel.json` file is not loaded by Athorina. Its pre-existing worktree changes are intentionally left alone.
