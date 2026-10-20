# Uploaded packs: what Kev found

The packs live under `assets/packs/<pack>/` (gitignored: `src/` holds the original files, `glb/` the converted GLBs).
They are not pushed. The Mech_00 licence forbids redistributing the asset, and this repo is public. FoodPack and
PropsLite came with no licence file, so treat them as local-only until a licence is found.

| Pack | GLBs | Rigged | Notes |
|---|---|---|---|
| mech_00 (Jimmys7777) | 31 | `Mech_Char_Mesh` (87 joints, Auto-Rig Pro) | `Mech_00_Anim.glb` holds 31 clips: Attack1-3 L/R, Shoot, Walk/Run (plus `_Root` versions), Jump, Die, GetHit. Includes attachments, Core_Sword/Hammer, and floor, wall and ledge tiles. Licence: personal and commercial use, no resale or redistribution. |
| food_newlua (NewLuaStudios) | 1 (28 meshes) | no | metres, about 0.75 m across |
| props_lite | 48 | no | **centimetres**: Barrel_01 is 99.5 tall, so scale by 0.01 when placing |
| company_characters | 4 | all (Rigify `DEF-`) | elf 216 j, knight 176 j, mage 191 j, dragon 143 j; no clips |

## Same tests on the new rigs

- `nodes.py`: roles.json gained Rigify rules (segment suffix after the side, `upper_arm`, `spine.004`-`.006` for
  the neck and head) and Auto-Rig Pro rules (`arm_stretch`, `leg_stretch`, `.x` centre marker, `*_Attachment`
  sockets as helpers). The elf, knight and mage pass the human body checklist with the other 8 rigs. The dragon
  and the mech pass the limb and symmetry check. Summary: `results/nodes/rigs.txt`.
  - elf / knight / mage: about 1.8 m tall, arms 0.33-0.36 of height, legs 0.53-0.59, A-pose (arm about 142 deg from up)
  - mech: 3.72 m (cm units), arms 0.63 of height (a gorilla build), legs 0.61
- `motion.py` now finds bones by role, not by UE name, so it runs on the mech's own clips
  (`results/motion/mech_00.json`, next to `results/motion/actionforge.json`).
- Categories are now rules on each clip's own features, not k-means centroid names, so a label does not move
  when the set of clips changes. Two features were added:
  - `extend`: how far the arm opens over the clip
  - `level`: how far the hips rise or fall
- The rules, with the measured values behind them:
  - a **swing** is fast hands, at least 14 arm lengths / s, without a level change or running feet
  - a **thrust** is the arm opening at least 0.5 of its length (jab 0.53, cross 0.57)
  - a **fall-or-rise** is the hips moving more than 0.3 leg lengths vertically
- Strikes found:
  - ActionForge: the 14 real hits (jab, cross, hook, overhand throw, shield bash, zombie scratch, sword attacks,
    tree chopping). No deaths, landings or sits.
  - Mech: exactly its six Attack clips. Die is fall-or-rise, Run is jump-or-kick, Shoot is gesture.
- Still imperfect:
  - in-place walk loops read as gesture, since in-place clips have no travel; their `_Root` versions are
    locomotion
  - Mech_GetHit reads as locomotion, because it is pushed back 2.3 leg lengths / s
  - the k-means clusters alongside are less stable now (0.25 at 25 % on ActionForge) because k drops to 3.
    They are kept only for comparison; the categories themselves are deterministic.
- The 84 UAL / ActionForge clips' original five features are unchanged.
