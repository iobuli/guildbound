# Guildbound (Unity) - handoff, 2026-10-11

## Current state (read first; supersedes the older notes below)
- **Design of record:** MASTER GDD v5.1 = the latest `BEGIN_MASTER_GDD` block in `TANDEM_REASONING.log` (repo root). Team log, roles and decisions live there too (Claude = rules code, verification, commits; Codex = art, animation, world, HUD presentation; boss = direction). `Documentation/MASTER.md` is older background.
- **Stack (M0 done):** Unity 6.3 LTS 6000.3.26f1, URP 17.3.0, Input System 1.20.1, Animation Rigging 1.4.1, Cinemachine 3.1.7, Test Framework 1.6.0, MCP for Unity v10.3.0, Aura for Unity plugin (free trial). Branch `build/m0`. Open the editor with `Tools/open_unity.ps1` (starts the MCP bridge).
- **Built:** layered HUD (`Assets/Game/UI/Kit`, `Guildbound/UI/Build HUD`), interfaces (`Assets/Game/Contracts/Interfaces`: IHudModel, ICombatPresentation, IInventoryModel), FxRepair (Magic Arsenal fire repaired with owned Hovl textures; no magenta), AvatarRepair, clip table (`Documentation/build/CLIP_TABLE.md`), Codex art tools/palette/panel kit.
- **In progress (M0.5):** owned collision motor + combat rules + fixtures in `Assets/Game/Motor`, `Assets/Game/Combat`, `Assets/Game/Tests` (Claude); animation driver + support layer (Codex).
- **Legacy, do not run:** `Assets/Game/Scripts/Game.cs`, `Fighter.cs` and `Assets/Game/Editor/Build.cs` (`Guildbound/Build Scene`) are an earlier prototype of unknown ownership kept for reference only. Do not generate `Main.unity` from them; M0.5 work happens in `Assets/Game/Scenes/Sparring.unity`.

## Older state notes (2026-10-11 morning, partly stale)
- **Project:** a fresh Unity 6000.5.1f1 project with one blank scene, `Assets/Game/Scenes/Main.unity`. Our code lives under `Assets/Game` (namespace `Guildbound`).
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

## Next (first playable, in order)
1. **`Build.cs` scene generator:** a town clearing from the RPGPP terrain tiles and buildings, Anime trees, the tavern-built guild hall, lighting and post.
2. **Player:** a rig with a CharacterController motor and a root-motion 8-way run/strafe tree; a Cinemachine orbit/lock-on camera.
3. **Combat:** the strike table from FightingMotions with baked reach and impact (port the strike-warp and bake ideas from the archive). Reactions by height and direction; guard/parry/throw.
4. **Magic:** Kevin Iglesias load/cast states plus Magic Arsenal effects implementing D18.
5. **Opponent and contracts:** sparring AI, guild board contracts, shop, crafting, save.

Archive of the earlier attempts: `D:/New folder (4)/guildbound-unity-archive/` (code worth mining: the strike bake/warp, D18 fire, the HUD and the Fetch API notes).
