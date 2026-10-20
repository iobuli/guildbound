# Kev: how it is built, and how to extend it

Kev is a set of small, stdlib-plus-numpy tools that read the asset library and the game's own data, measure it,
and write JSON the game and other tools read back. Every tool is one node of a dependency graph. Every rule a
tool applies sits in a JSON file next to it, so a new rig family, tool or weight is a data change, not a code
change.

## Layers

| Layer | Tool | Reads | Writes |
|---|---|---|---|
| 0. Nodes | `nodes.py` + `roles.json` | a GLB's JSON header and inverse-bind matrices only | node table and body measurements per rig |
| 1. Files | `kev.py`, `survey.py` | GLB headers, clip keys | categories, usability verdicts, cluster stability |
| 2. Motion | `motion.py` | clips through `tools/anim/clip_fk.py`, bones found by layer 0 | per-clip features and labels |
| 3. Chains | `combos.py`, `h2h.py` | clip end poses, the game's strike table | chains, `unarmed_vs_*` movesets |
| 4. Bodies | `targets.py` | layer 0 on each enemy rig, fist heights from layer 2 | where each strike lands, per body |
| 5. Checks | `evaluate.py`, `context/build.py` | layers 1-3; the project docs | reports, contact sheets, retrieval |
| Graph | `graph.py` + `pipeline.json` | the stage list | runs stages in order, with a cache |

### Layer 0: the hierarchical node parse (`nodes.py`)
- **Read only what is needed.** Each joint's rest position is the inverse of its inverse-bind matrix:
  `inv(IBM)[:3, 3]`. glTF stores the matrices column-major, so each is transposed.
- **One row per joint:** `[index, name, parent, role, region, length / height]`. This compact table is
  `--llm` output for language models.
- **Parse time:** under 50 ms per rig (tested).
- **Role:**
  1. The name is lower-cased.
  2. Prefixes are stripped (`strip`): `mixamorig:`, `b-`, `def-`.
  3. Segment suffixes after a side are dropped (`suffix`): Rigify `upper_arm.L.001`, Auto-Rig Pro `.x`.
  4. The side is read (`side`).
  5. The first matching regex in `roles` gives the role.
- **Region:** comes from the role (`regions`). A twist bone takes its parent's region, so regions are
  resolved in hierarchy order.
- **Body algebra:**
  - up axis = the head minus the mean of the feet, normalised
  - height = the head's projection on that axis
  - limb lengths = sums of segment lengths divided by height
  - asymmetry = |L - R| / mean
  - arm pose angle = the angle of the upper arm against up
  - per region: PCA (SVD of the centred joints) gives its principal axis against up and its elongation
    (s1 / s2), plus its vertical span `span_h`
- **Tested on 11 rigs** (UAL x2, Sekiro, Rokoko, Kevin Iglesias, hero male / female, ActionForge, Rigify elf,
  knight, mage) with one body checklist. The dragon and the mech (Auto-Rig Pro) pass a limb and symmetry check.

### Layer 1: files (`kev.py`, `survey.py`)
- Features come from headers: size, nodes, meshes, clips and bones.
- Features are standardised, then clustered with k-means. k is chosen in 2..kmax by the best silhouette score.
- Stability is the median adjusted Rand index of clusterings on 25 / 50 / 75 % subsamples against the full run.

### Layer 2: motion (`motion.py`)
- Clips are sampled with forward kinematics at 30 fps. Canonical bones are found through layer 0, so any parsed
  rig works.
- Every feature is scale-free:

| Feature | Definition |
|---|---|
| hand_speed | 95th-percentile hand speed relative to the pelvis, in arm lengths / s |
| reach | largest shoulder-to-hand distance, in arm lengths |
| extend | range of the shoulder-to-hand distance over the clip, in arm lengths |
| travel | pelvis ground speed, in leg lengths / s |
| foot_lift | highest foot above the clip's lowest foot point, in leg lengths |
| level | signed pelvis height change, first to last frame, in leg lengths |
| gait | correlation of the left and right foot heights |
| burst | 95th-percentile over mean pelvis ground step |
| returns | 1 when the last pose matches the first |

- Labels come from fixed rules (`name_cluster`), with thresholds read off the measured clips and documented
  beside each constant:
  - strike = swing or thrust
  - fall-or-rise = |level| above 0.4
  - locomotion = gait below -0.4 in a loop, or steady travel
  - transition = travel without a steady cycle, or not a loop
  - jump-or-kick, idle and gesture cover the rest
- k-means clusters are kept alongside, for comparison only.

### Layer 3: chains (`combos.py`, `h2h.py`)
- **Transition A to B:**
  - the mean angle between A's last pose and B's first pose
  - the mean difference between A's end motion and B's start motion (axis-angle rotation vectors)
- **The hand-to-hand chain cost** for an enemy weapon (`h2h.chain_cost`) is the sum of:
  - `facing_weight` x the hip turn across each cut
  - the transition angle above `crossfade_hides_deg`
  - `same_hand_penalty`
  - `windup_weight` x the wind-up past the enemy's own wind-up
  - minus `impact_weight` x the mean poise
  - minus `target_weight` x the mean landing score on the enemy's body (layer 4)
- A strike whose window ends mid-swing may only close a chain.
- Chain length comes from the enemy's wind-up (`chain_length_by_windup`).
- The search is exhaustive over the pool, with no strike repeated back-to-back.

### Layer 4: bodies (`targets.py`)
1. The fist's height at the strike's peak is measured as a share of the attacker's height, then scaled to the
   hero body.
2. On each enemy rig at its own size, the fist lands in the first region whose `span_h` holds it: head, then
   torso, then leg. Between spans it takes the nearest one.
3. Above the skull (0.12 of height above the head joint) the strike is `over` and misses.
4. Score: head 2, torso 1, leg 0.25, over 0.

## How the game and Kev share one source of truth
- **Strike table.** The game's cutter (`crates/game/src/play/repertoire.rs`, `segment_explained`) exports
  `staging/kev/strikes.json` (`cargo test -p guildbound export_strike_table -- --ignored`). It holds every strike,
  and every rejected speed peak with its reason. `h2h.py` takes each strike's window from it, so Kev and the game
  never disagree.
- **Movesets.** `h2h.py --write` writes `assets/anim/movesets/unarmed_vs_<weapon>.json`. The game's
  `Movesets::versus` picks the set that matches the target's weapon, and the viewer lists them and plays each
  chain.

## Extending Kev without touching tool code
- **A rig family:** add its prefix to `strip`, a pattern to `roles`, or a suffix to `suffix` in `roles.json`, then
  add a naming test to `test_nodes.py`.
  - Rigify needed `suffix: (?<=\.[lr])\.\d{3}$`, `upper_arm`, and `spine.004`-`.006` for the neck and head.
  - Auto-Rig Pro needed `arm_stretch`, `leg_stretch`, `.x`, and `attachment` as a helper.
- **A tool:** add a stage to `pipeline.json` with `cmd` (or `exec`), `inputs`, `after` and `outputs`.
  - `graph.py` orders the stages with Kahn's algorithm. A cycle or an unknown dependency stops the run before
    anything starts.
  - A stage is skipped when the hash of its command, its input files (path, size, mtime) and its upstream keys
    matches the cache.
  - `--only` runs a stage and everything it comes after.
- **A planner weight or rule:** edit `h2h.json` (`rules`, `target_rig`, `other_rigs`). The pool's punches must
  come from the allowed libraries (`hand_strike_libs`), which the planner checks before planning.
- **A motion threshold:** edit the constant in `motion.py`, beside its measured range, and pin it with a row in
  `test_motion.py`.

## Running it
- `python3 -I tools/kev/graph.py --list` lists the stages.
- `python3 -I tools/kev/graph.py` runs what changed.
- `python3 -I -m unittest discover -s tools/kev -t tools/kev` runs the tests.
