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
  (`results/motion/mech_00.json`). All six Attack clips land in strike clusters. Stability is 0.64 at 25 % and
  0.83 at 50 %. Two weaknesses carry over from the UAL run:
  - deaths and landings also fall in a strike cluster (strike-3)
  - the walk clips read as "gesture", because the mech's root motion is on `c_traj`, not the hips
- The 84 UAL / ActionForge clips give identical features to before (largest difference 0.0).
