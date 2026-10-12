# Guildbound - handoff, 2026-10-11

## Latest (2026-10-12, Claude, absorb + legs pass) - read first
- **Hero:** RPG Character is the only model and the nucleus; 130 clips (RPG native, FightingMotions, Kevin mage, Mixamo set from Snaiel's Godot prototype, Quaternius UAL1+UAL2). Rebuilt from source with `Tools/blender/build_hero.py` (never `--extend-hero` round-trips: each one dropped a sample). Every clip is re-timed to 30 fps at bake (FBX imports switch the scene fps; the old hero had 8-9 samples per 0.8 s run cycle) and cyclic clips are loop-closed (seam 0.000 m). Audit: `check_hero_mapping.py` pass.
- **Locomotion:** phase-synced 8-direction walk/run blend space aligned on each clip's left-foot plant; character right is (-fz, fx) (the old sign mirrored every side clip). Root `Motion` is pinned to rest (it carries no motion in any clip; the old vertical keep leaked travel into an 0.43 m pelvis sag under lean = the 83 deg knee pop). Support IK takes its knee pole from the animated knee each frame.
- **Pose rules (boss):** `joints.rs` soft segment-angle limits on spine/neck/arms/wrists/ankles (roll-invariant; this rig flips bone roll so local limits are meaningless); `knee.rs` hinge: forward only, no hyperextension, max 135 deg flexion, foot kept. Ragdoll knees use the anatomical side axis with the same limit.
- **Measured (Tools/check_legs.py, live):** mean planted-foot slip 0.41 m/s over 16 walk/run directions (was 3.09), worst knee pop 28 deg (was 83). Controller 20/20, spells 3/3. **Open:** combat check fails on the ragdoll hinge-axis assert (0.151 rad vs 0.12; constant offset, likely hinge axis frames at creation; predates the legs pass).
- **World:** 60 m open world (sim bound `props::WORLD_HALF`), courtyard at the centre, deterministic Quaternius scatter (trees, rocks, plants, market clearing NE) with tree/rock obstacles in the sim. Sim also has posture/deathblow/backstab/notice/crits/timed drinks (`posture.rs`, `notice.rs`); audio `sfx.rs`.
- **Next (Beta V2):** Wenrexa UI rework, souls-like systems from Cat's template, Bestiary mobs, ragdoll hinge fix. Rules: commit every change, verify live before claiming, ask before push.

## Earlier Bevy state (before the absorb pass)
- **Engine:** Bevy 0.19.1 (Rust 1.99 stable, MSVC), code only, pinned per Decision 22. Boss dropped Unity on 2026-10-11. Branch `build/bevy`.
- **Workspace:** `crates/sim` = `guildbound_sim`, the engine-independent fixed-step motor, combat, magic and mob core (42 sim tests). `crates/game` = `guildbound`, the playable beta plus dev panel (9 controller/bridge/menu/motion tests). `.cargo/config.toml` caps builds at 6 jobs.
- **Run it:** double-click `play.cmd`, or `cargo run -p guildbound`. Menu: Spar fights a partner; Practice leaves the partner idle. Esc pauses; Tab opens the boss's dev panel (pause/step/reset/speed, live tuning, animation inspection, overlays and feed).
- **Hero:** RPG Character model/skeleton with 40 native, 42 FightingMotions and 7 Kevin Iglesias mage clips (89 total). Explicit bone/role/eight-direction mapping, measured stride phase and rule-clock action sampling. Read `Documentation/BETA_CONTROLLER.md` for conversion, audit and controller contracts. Foot IK/locking is not complete.
- **Verified 2026-10-12 (Codex):** mapping audit 89 clips / 801 pose samples plus 2405 attack/cast-landmark samples; workspace 51 tests. Live controller/combat/mage results and captures are in `Documentation/checks/live/`. Repeat with `python -I Tools/check_beta.py`, `python -I Tools/check_combat.py` and `python -I Tools/check_spells.py` while the beta is open.
- **Ownership now:** boss says no Aura; Claude ran out of usage, so Codex owns this fixing/polish pass without a handshake wait. Changes are uncommitted.
- **MB1 crash fix:** bevy_lunex 0.7.0's hover-event duplicator retains the parent target when forwarding to children, dispatching indefinitely. Our menu uses a bounded direct hover update. The earlier stack-reserve-only change was insufficient. `Tools/check_beta_liveness.py` exercises actual hover/out/MB1 click observers through `gb.menu_pointer` before polling liveness. Subsequent logs ending with 'No windows are open, exiting' record normal closes. Read `Documentation/BETA_CONTROLLER.md` for evidence and commands.
- **Expanded combat:** four-hit rush, buffered hit-confirm links, held directional smash, full-charge guard break, parry counter move, energy-cost homing dash into running punch. F/Mouse4 is smash; W/S select rising/sweep; Shift+Ctrl homing dash costs 8 ki. Ten clusters rotate 32 attacks with measured contact phase matching; held/release keeps the same clip. Aim/lean use frame-rate-independent springs.
- **Ragdoll:** RPG knockdowns use 11 dynamic capsules/10 limited joints, incoming momentum and eased get-up. Physics follows pause/speed/step. `gb.physics` reports stability. This is physical presentation; deterministic gameplay position and damage stay in `crates/sim`. Anatomical hinges, self-collision and terrain foot IK remain incomplete.
- **Mage casting:** fireball uses two-handed cast/load/release; lightning alternates mage hands. Fixed charge -> cast phase detection when sequence is reused, instant fireball release, source FBX animated-root unit/axis mismatch and bone-roll-dependent facing. Charge orbs follow final hand poses. Menu actions require primary click; Quit/OS-close causes are logged for diagnosis.
- **Pose mismatch:** roll geometry inspection showed the pelvis nearly one metre ahead of the motor. Fixed stale-parent root solving and rotating-root hip offset with current hierarchy transforms/world XZ anchoring; cached outgoing poses include the correction. The controller smoke now verifies the rendered roll pelvis against motor position, alongside clip/clock checks. `gb.presentation.geometry` exposes actual head/hips/feet/hands for review.
- **Latest live verification:** controller 20/20, combat/physics 9/9, mage 3/3. Liveness: 30 menu cycles, 300 actual pointer-observer events, responsive for 90.39 seconds. Beta remained open at menu after checks. Evidence: `Documentation/checks/live/beta-*-smoke.json`, `beta-liveness.json`, captures and `beta-release.err.log`.
- **Local verification API:** the running game answers JSON-RPC on `http://127.0.0.1:15702`. `Tools/gb.py` supports sim state, input, stepping, animation inspection and screenshots; `gb.presentation` reports active clips and pose hashes. It does not require Aura.
- **Unity is archived, not lost:** commit `0adfb47`, tag `unity-final` on `build/m0` (all code, scenes, tools, checks). Ignored packs, Aura package and caches: `D:/New folder (4)/guildbound-unity-archive/bevy-move-2026-10-11/`.
- **Read next:** `Documentation/ALPHA_LESSONS.md` (what to keep, verification rules, new Bevy risks, Unity -> Bevy translation table), then `Documentation/COMBAT_MOTION_PLAN.md` (phases still valid; its Unity type names translate through the lessons table).
- **Open tasks:** `collab/TASKS.md`.

## Unity era (historical; paths below refer to tag `unity-final`)
- **Design of record:** MASTER GDD v5.1 = the latest `BEGIN_MASTER_GDD` block in `TANDEM_REASONING.log` (repo root). Team log, roles and decisions live there too (Claude = rules code, verification, commits; Codex = art, animation, world, HUD presentation; boss = direction). `Documentation/MASTER.md` is older background.
- **Stack (M0 done):** Unity 6.3 LTS 6000.3.26f1, URP 17.3.0, Input System 1.20.1, Animation Rigging 1.4.1, Cinemachine 3.1.7, Test Framework 1.6.0, MCP for Unity v10.3.0, Aura for Unity plugin (free trial). Branch `build/m0`. Open the editor with `Tools/open_unity.ps1` (starts the MCP bridge).
- **Built:** layered HUD (`Assets/Game/UI/Kit`, `Guildbound/UI/Build HUD`), interfaces (`Assets/Game/Contracts/Interfaces`: IHudModel, ICombatPresentation, IInventoryModel), FxRepair (Magic Arsenal fire repaired with owned Hovl textures; no magenta), AvatarRepair, clip table (`Documentation/build/CLIP_TABLE.md`), Codex art tools/palette/panel kit.
- **In progress (M0.5):** owned collision motor + combat rules + fixtures in `Assets/Game/Combat` and `Assets/Game/Tests` (Claude; being created now); animation driver + support layer (Codex).
- **Legacy, do not run:** `Assets/Game/Scripts/Game.cs`, `Fighter.cs` and `Assets/Game/Editor/Build.cs` (`Guildbound/Build Scene`) are an earlier prototype of unknown ownership kept for reference only. Do not generate `Main.unity` from them; M0.5 play work will happen in `Assets/Game/Scenes/Sparring.unity` (not created yet).

## Older state notes (2026-10-11 morning, partly stale)
- **Project:** a fresh Unity 6000.3.26f1 (6.3 LTS) project with one blank scene, `Assets/Game/Scenes/Main.unity`. Our code lives under `Assets/Game` (namespace `Guildbound`).
- **Packages:** URP 17.5 (now 17.3.0 on 6.3 LTS), Input System 1.20.1, Animation Rigging 1.4.1, Cinemachine 3.1.7, AI Navigation 2.0.13, Test Framework, MCP for Unity v10.3.0.
- **Asset packs:** 16 Asset Store packs are imported. Their C# scripts were deliberately skipped: we write our own systems. Built-in materials were converted to URP; 4 Unlit/Color and Unlit/Texture materials remain.

## Rebuild the asset folders (they are git-ignored)
1. **List purchases:** open the editor normally, signed in through the Hub. `Unity.exe -projectPath <proj> -executeMethod Guildbound.Editor.Fetch.List -guildboundExit` writes `Logs/fetch-purchases.txt`.
2. **Download:** put product ids in `Logs/fetch-ids.txt`, then run `... -executeMethod Guildbound.Editor.Fetch.Download -guildboundExit`. Files land in `%APPDATA%/Unity/Asset Store-5.x/`. (The wait loop never sees completion; close the editor once the files exist.)
3. **Unpack:** `python -I Tools/unpack.py <proj> Tools/packs.txt` extracts every package, skipping code files.
4. **Convert materials:** `Unity.exe -batchmode -quit -nographics -projectPath <proj> -executeMethod Guildbound.Editor.AssetPrep.ConvertMaterials` (built-in -> URP).
5. **Catalog:** `... -executeMethod Guildbound.Editor.Catalog.Run` writes `Logs/catalog/*`.
6. **Contact sheets:** `... -executeMethod Guildbound.Editor.AssetPrep.Sheets` (batchmode without `-nographics`) writes `Logs/sheets/*.png`.

Product ids for this project: 276931 305932 45047 36286 76699 326043 28345 283656 247933 291284 282087 148410 394336 36405 42454 47320 65284. Inventory Framework (283656) is downloaded but deliberately not imported.

## Asset -> system map (decided)

| System | Source | Notes |
|---|---|---|
| Unarmed locomotion | ExplosiveLLC/RPG Character FREE | `Unarmed-Run-*` and `Unarmed-Strafe-*` in 8 directions with root motion (run ~3.6-4.2 m/s, strafe ~2.1 m/s) feed a 2D freeform tree. Plus Idle, Jump/Jump-Flip/Fall/Land, DiveRoll (dash), Stunned. |
| Unarmed strikes | FightingMotionsVolume1 | `hp_straight` (jab), `hp_hook`, `hp_upper` (punches); `bk_rh` (body roundhouse), `hk_rh` (high roundhouse), `hk_side` (side kick), `lk_rh` (low kicks). Explosive `Unarmed-Attack-L1..R3` as extra light strings. The `*_A2` clips are the in-place variants. |
| Reactions by attack height | FightingMotionsVolume1 | `hd`/`bd`/`ld`_front = head/body/low damage; `hb`/`bb`/`lb`_front = head/body/low block. `knockdown_A`, `knockdown_loop_A`, `getup_A`, `win_A`. Explosive `Unarmed-GetHit-F/B/L/R`, `Knockback-Back1/2`, `Knockdown1`, `Getup1` for direction. |
| Parry throw | Brawler | `Grab_Throw_A/B`, staged 0.70 m apart facing each other (from the sample scene). |
| Magic | Kevin Iglesias spellcasting (HumanM) | `MagicAttackDirect1H01_L`/`_R` = tap fire, alternating hands. `MagicAttackDirect2H01 - Load` (loop) then `- Cast` = charged fire. `MagicAttackOmni01` = overheld burst (D18). `CastingEnter`/`Idle`/`Exit`/`Damage` = stance and interrupt. |
| Weapons | Warrior Bundles 1-3 | Brute, Karate, Ninja, Sorceress, 2Handed, Archer, Knight, Mage, Crossbow, Hammer, Spearman, Swordsman. Each has Attack1, Idle, Walk, Run, Jump, Fall, Land. RPG pack `2Hand-Sword-*` has a full 11-attack sword set with locomotion and reactions. |
| Town NPCs | MoCapCentral, Idle MoCap, Explosive Crafting | Idles, talk, emotions, spellbook chant (guild clerk), drunk walk (tavern), vending, greet/nod (merchant), carry/pick-up (gathering). |
| Town and props | RPGPP_LT, Low Poly Tavern, Anime Trees | RPGPP: 5 houses, stalls, fences, gates, well, cart, banners, terrain tiles, rocks, trees. Tavern: modular walls/roofs/floor plus furniture for the guild hall interior. Anime Trees: painterly trees, bushes, grass. |
| VFX | Hovl Studio, Magic Arsenal | Hovl: lightning aura (awaken), electro hit, explosion, AoE, shields (guard/parry), portals, sparks, dust (footfalls/landing). Magic Arsenal: per element, Charge / Missile Tiny-Mega / Muzzle / Explosion Tiny-Mega / Dome / Beam. Fire charge stages: Fire (orange) -> Frost/Arcane (blue) -> Light (white). |

## Known issues
- **Magic Arsenal (fixed):** materials/scripts came from two unowned packs; `Guildbound/Assets/Repair Magic Arsenal Fire` (Assets/Game/Editor/Tools/FxRepair.cs) rebuilds them as URP Particles/Unlit on owned Hovl textures with fire tints. Re-run after a pack re-import.
- **Characters:** the Explosive characters are blocky demo mannequins. The smoothest rigs are MoCapCentral `rig_MCUnity` and the FightingMotions Blue/RedGuy. The boss decides the hero model; all clips are Humanoid, so any of them can play everything.
- **Hovl effects:** some sample tiny at 0.8 s. They play correctly in game, but check scale per effect.

## Next (current plan: GDD v5.1 milestones; the older list below the line is superseded)
1. **M0.5 combat spike (Claude):** deterministic fixed-step combat core in `Assets/Game/Combat` (asmdef `Guildbound.Combat`): owned motor (sole mover), action states, swept contact + one outcome per target/action, guard capacity/break, step, throw, plain/charged cast, recover; EditMode fixtures in `Assets/Game/Tests` run at 30/60/120 fps.
2. **M0.5 presentation (Codex):** animation driver on ICombatPresentation, fixed-length support/contact layer, HUD inventory page, fire visuals from owned packs.
3. **Sparring floor:** `Assets/Game/Scenes/Sparring.unity` (Aura may create it when the boss says go).
4. **M1 greybox gold path** after the M0.5 gates (GDD section 11).

Superseded (morning plan, do not follow): Build.cs scene generator, CharacterController + root-motion player, strike bake/warp, old D18 magic, shop/crafting.

Archive of the earlier attempts: `D:/New folder (4)/guildbound-unity-archive/` (code worth mining: the strike bake/warp, D18 fire, the HUD and the Fetch API notes).
