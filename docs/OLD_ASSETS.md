# Old builds and their assets

Every earlier build of Guildbound is still in this repo's git history.

## 1. Godot build (branches `main`, `claude/loving-babbage-ne8up6`, Oct 7)
It's all committed and recoverable with `git show origin/main:<path>`, about 67 MB.

| What | Where | Count | Usable in Bevy? |
|---|---|---|---|
| Animations | `assets/anims/*.res` | 334 | Godot binary `.res` files, not glTF. They're Mixamo-style packs: FIST (jab, cross, hook, uppercut, front kick, roundhouse), GS greatsword, SS sword+shield, MAG magic, BOW, ADV, LOCO, FEM, DRUNK, FARM. To use them, get the source FBX/GLB from wherever the Godot import came from, or export them from Godot as glTF. |
| Weapons | `assets/weapons/*.gltf` | 15 | Yes, these are glTF: axes, dagger, swords, shields, staff, wand, spellbook, arrow. |
| Characters | `assets/characters/base` | 58 files | glTF base bodies. |
| Icons | `assets/icons/rpg` | 115 | PNG. |
| Audio | `assets/audio` | ~130 `.ogg` | Yes: punches, cloth, footsteps, metal, coins, books. |

## 2. Unity build (Oct 11, commits `910e531` … `0adfb47`, removed in `05422ea`)
Only our own code, scenes and catalogs were committed. The Asset Store packs themselves were never committed, so they're still on the PC that ran Unity, under
`C:/Users/64Bit 1TB NVME/AppData/Roaming/Unity/Asset Store-5.x/`:

| Pack | Clips/models | Best for |
|---|---|---|
| Fighting Motions Vol1 (Magicpot) | 42 clips | **Boxing**: hooks, jabs, uppercuts, combos |
| Brawler Animations - Free (SainDeveloper) | 24 clips | **Punches**, brawling |
| Fighter Pack Bundle FREE (Explosive) | ~4 fighters | Punch and kick sets (male, female, heavy, berserker) |
| Warrior Pack Bundle 1–3 FREE (Explosive) | ~220 clips, combined with the other Explosive packs | Weapon move sets: sword, spear, crossbow, etc. |
| RPG Character / Crafting Mecanim FREE (Explosive) | (in the 220 above) | Locomotion, crafting |
| MC Sample - Believable 3D Animations (MoCap Central) | 129 clips | Mocap: locomotion, actions |
| Human Spellcasting Animations FREE (Kevin Iglesias) | 90 clips | Spell casts |
| Idle MoCap (Morro Motion) | 17 clips | Idles |
| Magic Effects FREE (Hovl), Projectile Factory (Archanor) | 76 + 241 prefabs | VFX reference, textures |
| RPG Poly Pack Lite, Stylized Cozy Tavern, Anime Nature | 90 + 35 + 14 models | Props, tavern, trees |

The full clip list is `git show ca1054f:Documentation/catalog-clips.txt`.

### Getting them into the game
Bevy reads glTF/GLB, not FBX or `.unitypackage`. These packs are licensed for use in your game, but not for putting in a public repo.

1. Zip the `.unitypackage` files you want, starting with **Fighting Motions Vol1** and **Brawler Animations** for punches, and attach them in chat.
2. agent-1 unpacks them (a `.unitypackage` is a tar of GUID folders) and converts the FBX files to GLB in Blender (headless) on the same skeleton route as UAL.
3. Each pack becomes a library like `ual1.glb`. The strike cutter (`play/repertoire.rs`) then picks up its punches automatically.
