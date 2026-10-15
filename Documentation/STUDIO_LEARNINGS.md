# Studio learnings - boss notes -> numbers -> root cause -> fix scope

Each boss note from the Studio (collab/studio_notes.jsonl, gb.studio) is diagnosed here so later notes can
be matched against known causes before anything is hand-corrected. Scope says whether the fix was a single
correction (assets/corrections.json), a library-wide rule (promoted correction, clip "*"), or the pipeline.

| # | Note (model, clip @ t, bone) | Evidence | Root cause | Fix | Scope |
|---|------------------------------|----------|------------|-----|-------|
| 2 | pose_audit: source error peaks 60-100 deg in 50+ clips, mean 2.8 (2026-10-12) | Peaks only at fast moments (punch extension, flips, hit reactions); calm frames matched | The bake imported the 30 fps glTF source at Blender's default 24 fps and sampled between source keys, cutting fast peaks | `load_source` imports glTF at OUT_FPS. Mean 2.8 -> 0.6 deg, peaks gone, jump ground-sink and roll pops gone | Pipeline: every glTF-sourced clip |
| 3 | pose_audit: Pelvis->Spine 27 deg off in 40 clips | Every other spine/arm/leg segment matched to 0.0-0.4 deg; the source B_Spine has location keys (hip sway), B_Neck a constant offset | Retarget carried rotations only; authored joint translations were dropped | `retarget` carries each joint's offset from where its parent puts its rest head, keyed as location (zeroed before reading so last frame's key cannot feed back). 27 -> 11 deg in 2 clips | Pipeline |
| 4 | pose_audit: death-kneel toes 12 cm under the floor | Source on the floor; Ranger shin/thigh ratio differs | Pelvis height scaled by whole-leg ratio | `retarget` lifts the pelvis when a knee, ankle or ball goes under the root's floor | Pipeline |
| 5 | pose_audit: ankle under 40 deg in 215 clip-findings, neck/upper chest over range in 40 | Kevin raw mocap reaches 22-31 deg in every run push-off; Ranger rest pose bends neck 30 and upper chest 14 (RPG 4 and 5) | Joint table calibrated on the RPG rig's straight rest; real plantarflexion | joints.rs: ankle min 15, B_Spine2 max 50, B_Neck max 70 | Rule (runtime authority + audit) |
| 6 | pose_audit: nc_* loops 12-60 deg seams | Sampling at 24 fps missed last keys | Audit bug, not data (Kevin loops close exactly) | pose_audit imports at 30 fps and samples exact keys | Tool |
| 1 | "incorrect placement in context" - hero (Ranger), Unarmed-Attack-R1 @ 0.65 s, B_R_Clavicle (2026-10-12) | Same clip/time on the original RPG model: right upper-arm bend 42.3 deg vs 13.0 on the Ranger, left 38.6 vs 49.5; no joint out of range. The raised left arm is authored in the source clip. | Retarget copied world rotation deltas between different rest poses (RPG T-pose arms vs Ranger A-pose), so limbs aimed off. | `build_hero.retarget`: rotate each target bone's rest limb direction (joint to first mapped child joint) onto the source's before applying the delta. Quick build: right 35.7, left 37.7 deg. | Pipeline: every retargeted clip on every model |

## The audit loop (Claude, no agents)
`Tools/blender/pose_audit.py -- --glb <model> ... --source <rpg source glb> --kevin` checks every frame of every clip on
every model in about 30 s: joint ranges, knee hinge (flex and direction), floor, hands in torso, one-frame pops, loop
seams, and per-segment direction against the source clip. Each finding is compared with the same check on the source
(RPG glb, or the raw Kevin FBX for nc_*), so inherited issues are separated from ones our pipeline introduced; only
introduced patterns are listed. The digest goes to collab/pose_audit.txt/.json and shows in the Studio Patterns tab,
where clicking a pattern stages its worst case. Fix the cause, rebuild, re-run; a pattern across many clips is a
pipeline cause, a single clip is a correction.

## Patterns to check first on a new note
- A limb aims wrong but every joint is in range: compare against the source rig in the Studio (own clips) before correcting; suspect rest-pose mismatch.
- A pose is wrong only on one model: proportions/rest pose of that model (pipeline), not the clip.
- The same bone corrected in 2+ clips: promote to a library rule in the Patterns tab, then look for the pipeline cause.
