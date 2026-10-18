# Character model: parts, numbers, hero kit

State 2026-10-18. Everything here is measured by `tools/model/analyse.py`, built by
`tools/model/build.py` and checked by `tools/model/verify.py` (all run with `python -I`, need
`numpy scipy pygltflib pillow`; no Blender). Reports: `assets/characters/hero/build_report.json`,
`assets/characters/hero/verify_report.json`.

## 1. The mannequin: `assets/anim/ual2.glb` (unchanged)

Quaternius Universal Animation Library 2 mannequin, CC0. It is also the canonical skeleton for
every clip (`motion/mod.rs`), so it is never edited.

| | |
|---|---|
| Skin | 1 skin, 65 joints (`root pelvis spine_01..03 neck_01 Head`, clavicle/upperarm/lowerarm/hand + 5 fingers × 4 per side, thigh/calf/foot/ball/ball_leaf per side) |
| Hierarchy | `Armature` → `root` (rotation −90° about X, Blender Z-up) → joints; mesh node `Mannequin` |
| Bind pose | equals rest FK to 6.4e-7 (max abs matrix error) |
| Mesh | one mesh, 2 primitives: `M_Main` 3389 verts / 5732 tris (body shell, sRGB-ish 0.80 0.40 0.04 = the yellow), `M_Joints` 5157 verts / 8012 tris (purple joint rings). Total 8546 verts (7330 welded), 13744 tris |
| Influences | M_Main: 1636 verts 1 joint, 860 two, 388 three, 505 four; M_Joints: 2098 / 2886 / 78 / 95. Weight sums 1 ± 1.2e-7. 52 joints carry weight |
| Height | 1.829 m, soles at y = 0 |
| Morph targets, textures | none |

Body split by dominant joint (both primitives together; left side, right is mirrored):

| Part (joints) | Verts | Tris | Radius about bone: median / p90 (m) |
|---|---|---|---|
| head (`Head`) | 202 | 370 | 0.143 from the head joint (max 0.26) |
| neck (`neck_01`) | 66 | 86 | 0.057 / 0.126 |
| torso (`spine_01..03`, clavicles) | 597 | 876 | 0.130 / 0.165 |
| pelvis (`pelvis`) | 251 | 384 | 0.121 / 0.144 |
| upper arm | 170 | 273 | 0.060 / 0.078 |
| forearm | 138 | 184 | 0.046 / 0.059 |
| hand (`hand_l`) | 76 | 142 | 0.038 / 0.052 |
| fingers (all phalanges) | 2905 | 4767 | index 0.0101, middle 0.0111, ring 0.0104, pinky 0.0090, thumb 0.0075 (median radius); chain lengths 0.106 / 0.111 / 0.102 / 0.096 / 0.133 |
| thigh | 160 | 216 | 0.075 / 0.091 |
| shin (`calf`) | 161 | 244 | 0.044 / 0.070 |
| foot (`foot`, `ball`) | 105 | 188 | 0.054 / 0.093 |

Palm thickness about the `hand_l` joint (T-pose, along Y): −0.047 / +0.022 m (`motion/hand.rs`
`PALM` = 0.022). Body capsules (`motion/body.rs`): torso 0.12, head 0.10, arm 0.04, thigh 0.065,
calf 0.045: inner capsules, below the median skin radii above.

## 2. Source parts: `assets/characters/base/` (from `main`)

Quaternius Universal Base Characters (Superhero male / female) with their hair and eyebrows and
textures, Blender glTF exports (`.gltf` + `.bin`; the Godot `.import` files were not copied).
**Licence:** Quaternius publishes its free packs, Universal Base Characters among them, as
**CC0 1.0** (public domain). The folder contains no licence or readme, and quaternius.com could
not be reached from this machine, so the licence is recorded here from the publisher's usual
terms: **confirm it on the pack's download page** before shipping.

| File | Contents | Verts / tris |
|---|---|---|
| `Superhero_Male_FullBody` | body (`MI_Superhero_Male`, 2048² base colour, normal, roughness), eyes (`MI_Eyes`), brows (`MI_Hair_1`) | body 7281 / 12566 (6285 welded); eyes 556 / 768; brows 646 / 984 |
| `Superhero_Female_FullBody` | same, female | body 7376 / 12812 (6408 welded); eyes 556 / 768; brows 912 / 1480 |
| `Hair_SimpleParted`, `Hair_Buzzed`, `Hair_Beard` | male-fitted hair (`MI_Hair_1`) | 757/1301, 466/830, 596/1034 |
| `Hair_Long`, `Hair_Buns`, `Hair_BuzzedFemale` | female-fitted hair (`MI_Hair_2`) | 3080/2906, 3501/3284, 466/830 |
| `Eyebrows_Regular`, `Eyebrows_Female` | brows | 646/984, 912/1480 |

Two gltf files reference missing images (`T_Eye_Normal_png.png`, `T_Hair_1_Normal_png.png`); the
tools fall back to the `.png` without `_png`. `T_Hair_{1,2}_BaseColor_png.png` duplicate the
`T_Hair_{1,2}_BaseColor.png` byte for byte.

**Skeleton check against ual2:** same 65 joint names in the same order and the same parent
structure, but **not** the same rest pose or inverse bind matrices: joint positions differ by up to
5.9 cm (male body, feet), 8.4 cm (male hair files, fingertips) and 15.4 cm (female, fingertips);
max IBM difference 0.33 (male), 0.37 (female). Bone frames match ual2's up to the turn between bone
directions (≤ 14°, thumbs 13° of roll), so the axis conventions agree. The male body file and the
male hair files even differ from each other (hand at x 0.706 vs 0.683). Hence every mesh is
**rebound** onto ual2 (section 3); ual2's skeleton is never changed.

Proportions vs ual2 (bone lengths, source → ual2): male upper arm 0.251 → 0.274, forearm 0.244 →
0.273, thigh 0.429 → 0.400, shin 0.459 → 0.429; female clavicle 0.142 → 0.221, upper arm 0.240 →
0.274, hand 0.095 → 0.124.

## 3. Rebinding onto ual2 (`tools/model/rebind.py`)

Each vertex moves by the weighted sum, over its joints, of a displacement interpolated along that
joint's bone: the joint's own move (ual2 rest − source bind) at the bone start, the end joint's move
at the bone end, by the vertex's projection on the source bone. Hands and fingers also scale across
the bone by √(length ratio), so lengthened fingers keep their proportions. The clavicle start uses
the neck's move (UAL rigs put the clavicle joint at arbitrary depth in the chest). Normals turn with
the local change of the surface. Weights are kept (same joint indices, renormalised); the files are
then written with ual2's skeleton nodes and inverse bind matrices verbatim.

Result (rebound, before clothes), left side:

| Part | ual2 radius median | male | female |
|---|---|---|---|
| torso | 0.130 | 0.137 (+7 mm) | 0.120 (−10 mm) |
| pelvis | 0.121 | 0.128 | 0.118 |
| upper arm | 0.060 | 0.068 (+8 mm) | 0.046 (−14 mm) |
| forearm | 0.046 | 0.046 | 0.032 |
| thigh | 0.075 | 0.087 (+12 mm) | 0.082 (+7 mm) |
| shin | 0.044 | 0.048 | 0.044 |
| neck | 0.057 | 0.072 | 0.051 |
| fingers (median radius) | 0.009–0.011 | 0.010–0.013 (thumb 0.015) | 0.010–0.011 (thumb 0.012) |
| finger chain lengths | 0.106/0.111/0.102/0.096/0.133 | identical (on ual2's bones) | identical |
| palm Y extent about hand joint | −0.047 / +0.022 | −0.031 / +0.021 | −0.033 / +0.025 |
| height | 1.829 | 1.799 | 1.803 |

## 4. Polygon editing (`tools/model/build.py`)

- **Head proportion:** everything weighted to `Head` (face, skull, eyes, brows, hair, beard) is
  scaled ×1.05 about the head joint, blended by the Head weight so the neck stays continuous: a
  slightly larger head reads better at third-person distance and suits the stylised world.
- **Body-shape morph targets** on the body and on every garment (same deltas, so clothes follow):
  `bulk` (muscle: up to 12 mm along the normal on arms, chest, shoulders, thighs), `heavy` (soft
  weight: up to 12 mm on the trunk and limbs plus up to 28 mm on the front of the belly), `slim`
  (−8 mm on trunk and limbs), `jaw` (up to 7 mm wider and further forward at the jaw line). Amounts
  are smoothed over the welded mesh; deltas run along the smoothed normal.
- **Body split for clothes:** the body mesh is cut into nodes `body` and
  `body_hide_<garments>` by which garments cover each triangle (all three vertices at least
  2–3.5 cm inside the garment's edges). The game hides a group while one of its garments is worn,
  so no body polygon under clothes can poke through. Male groups (tris): body 5752, tunic 748,
  shirt 14, tunic+shirt 1768, trousers 1614, tunic+trousers 78, boots 408, trousers+boots 400,
  boots+shoes 980, gloves 804; eyes 768.
- **Clean soles:** boots and shoes are flattened to one plane 5 mm below the sole
  (`y = sole − 0.005`), sole band darkened in the vertex colours.
- **Weights for hair and beard** are transferred from the nearest skin (inverse-distance blend of 4
  vertices): scalp hair stays 100 % Head, hair at the nape and on the back follows the neck and upper
  back. Head-only hair (as authored) swung through the shoulders by up to 13 cm in turning clips.
- **Hair fit:** native hair keeps its authored roots; hair on the other base is mapped through the
  two heads' bounding boxes and pushed 1.5 mm out of the scalp; all hair is pushed 4 mm outside the
  outermost cloth (tunic, shirt, baldric at full bulk + heavy) and 2.6 cm off the torso skin.
  Every hair and brow style fits both bases; the beard is male only.
- **Skin texture** is divided by its average skin colour (neutral, ×0.92), so the look's skin tone
  is the material base colour; textures reduced to 1024² (skin), 512² (hair), 256² (eyes).
  Eye colours are recoloured irises: brown, blue, green, grey, hazel, amber.

## 5. Clothing (`tools/model/surface.py`, `build.py` `GARMENTS`)

Each garment is cut from the welded body surface along iso-lines of scalar fields (height, arm
coordinate |x|, a V/round neckline, skin weights for the glove cuff and knuckles, a plane band for
the baldric). The cut inserts vertices on crossed edges with **all attributes interpolated**
(position, normal, the 65 dense skin weights, morph deltas), so cut edges are clean and new vertices
carry weights interpolated from the original surface. The patch then becomes a shell: outer surface
offset along the smoothed normal, inner surface, and a rim joining them at every open edge, with a
rolled bead (thicker hem). A drape term lets cloth bridge hollows: each vertex is lifted by how far
nearby garment points stand above its tangent plane (tapered to zero at 6–11 cm), so a shirt spans
the chest instead of following every muscle, with no sliding along the skin (weights stay valid).
Weights: those of the skin under each vertex (top 4, renormalised).

Layering (offsets in metres, outer / inner): gloves 0.005 / 0.002, trousers 0.008 / 0.004, shirt
0.014 / 0.011, tunic 0.015 / 0.011 (+ up to 3 cm hem flare, 6 mm cuff flare), boots and shoes
0.016 / 0.012 (+ 8 mm boot cuff), baldric 0.020 / 0.016, belt 0.030 / 0.023. Each inner layer's
outer face sits under the next layer's inner face, and because all layers come from the same skin
with the same weights their order holds at every vertex in any pose.

| Garment | Slot | Covers | Verts / tris male (female) | Mounts |
|---|---|---|---|---|
| tunic | top | V-neck to hem at y 0.90 (hip), long sleeves to 0.715 (wrist cuff) | 4286 / 6756 (4734 / 7620) | |
| shirt | top | round neck, short sleeves to 0.37, hem 0.95 over the trousers | 3312 / 4952 (3708 / 5728) | |
| trousers | legs | waist 1.035 to ankle 0.075 | 3350 / 5376 (3282 / 5304) | |
| boots | feet | below 0.36 (mid calf), flat sole | 2392 / 4136 (2348 / 4048) | |
| shoes | feet | below the ankle line (0.125, lower over the instep) | 1972 / 3160 (1940 / 3144) | |
| gloves | hands | fingerless: wrist cuff (0.685) to the knuckles (finger weight < 0.5) | 2588 / 3072 (2500 / 2976) | |
| belt | belt | band 0.995–1.055 + brass buckle box | 976 / 828 (1006 / 888) | `sheath_hip_l`, `sheath_hip_r` (parent `pelvis`) |
| baldric | strap | 4.8 cm band from the right shoulder across chest and back to the left hip, under the belt | 1680 / 1400 (1734 / 1516) | `sheath_back` (parent `spine_03`) |

Mount frames: origin at the scabbard throat 12–15 mm off the cloth, **+Y toward the hilt**, **+Z
away from the body**. `sheath_hip_l` hangs a sword on the left hip for a right-hand draw (hilt
forward-up, Y = (0, 1, 0.55) normalised), `sheath_hip_r` a dagger (Y = (0, 1, 0.25)), `sheath_back`
lies along the baldric with the hilt over the right shoulder. In game they are re-parented onto the
rig's `pelvis` / `spine_03` bones, so weapon sheathing can find them by `Name`.

Colours (sRGB, from the world `Palette` family: earthy, muted): madder 0.56 0.17 0.14 (cloth red,
cf. Palette.cloth 0.70 0.21 0.17), moss, woad, linen, ochre, umber, charcoal, leather 0.45 0.31 0.19,
dark leather 0.26 0.17 0.11 (cf. dark_wood 0.39 0.26 0.15), brass buckle. Garments carry painterly
vertex colours (soft low-frequency variation ±7 %, darker low down, inner faces 0.55, rims 0.8).

## 5b. The wardrobe (2026-10-18)

Thirteen more garments, built for both bodies:

| Kind | Garments | Built as |
|---|---|---|
| Body-surface shells (`GARMENTS`) | `haori` (open front, wide sleeves), `robe_top` (long sleeves), `vest` (open front, no sleeves), `mantle`, `hood` (face opening), `bracers`, `hakama` (legs flaring to the ankle), `leg_wraps` | like the first set |
| Hanging cloth (`tools/model/drape.py`) | `robe_skirt`, `kilt`, `apron` (front only), `cloak` (open front, fitted to the torso, not the T-posed arms), `tabard` (front and back panels) | ring grids around the body axis: each ring wraps the 2D convex hull of the body's cross-section in a thin band (arms out), and is never narrower than the ring above, so the cloth falls from the widest point and bridges the legs; `flare` widens the hem |

Hanging cloth weights:
- The top follows the nearest body surface.
- Below the hips, each side blends toward its own thigh, then its own calf, keeping 20 % pelvis.
- The front and back centre lines are shared between the legs, so a stride swings the skirt without tearing it between them.
- Morph targets come from the nearest body vertices.

The game wires them up through `appearance.json`:
- `classes` sets the player's look per class:
  - Blade: haori, hakama, sash, kasa;
  - Arcanist: robe, mantle, hood;
  - Hunter: vest, cloak, bracers, leg wraps.
- `cast` gives every named NPC a look.
- `areas` (guild, village, ruins, pond) provides seeded outfits for anyone else.

## 5c. Our own textures (`tools/model/textures.py`)

Every texture the kit ships is generated; no pixel of the Quaternius textures is used, only the meshes' UV layouts.
- **Skin** (base colour + normal map, 1024²), baked in UV space from the mesh:
  - cavity (nearby surface above the tangent plane) darkens and cools creases;
  - convexity lightens ridges;
  - flushed zones near the cheeks, nose, ears, knuckles, elbows and knees;
  - eye sockets around the eye meshes, and lips as an ellipsoid below the nose tip (the head's frontmost point);
  - fractal mottling and pores;
  - the normal map comes from a height field of the same features.

  It is near-white and neutral: the game multiplies it by the look's skin tone.
- **Eyes:** sclera with veins, a fibrous iris with collarette and limbal ring, pupil and catchlight, painted where the eye mesh's UVs put the iris; one per eye colour.
- **Hair:** anisotropic strand streaks with clumping, neutral for tinting, plus a normal map.
- **Cloth:** tileable linen, twill, wool, leather and silk, each with a normal map. Garments get UVs (`textures.cloth_uv`): wrapped around the body axis, or around and along the arm for sleeves, tiling every 0.12 m (0.09 m for leather). `build.FABRIC` picks each garment's weave.

## 6. Files

- `assets/characters/hero/{male,female}/body.glb`: skeleton + body groups + eyes, morphs.
- `assets/characters/hero/{male,female}/{hair_parted,hair_buzzed,hair_buzzed_f,hair_long,hair_buns,brows_regular,brows_soft,beard(male),tunic,shirt,trousers,boots,shoes,gloves,belt,baldric}.glb`
- `assets/characters/hero/tex/*.png` (shared, referenced by relative URI `../tex/`).
- `assets/anim/hero_styled.glb`: the wanderer (male, tunic, trousers, boots, gloves, belt,
  baldric, parted hair, brows, beard) in one self-contained file (textures embedded).
- `assets/data/appearance.json`: palette, slots, looks (`wanderer` = player default, `ranger`,
  `villager`, `smith`).

Every file has ual2's 65 joints in ual2's order with ual2's node TRS and inverse bind matrices.

## 7. In game (`play/appearance.rs`)

- `GB_HERO` unset or `look`: the player wears the wardrobe's `player` look (`GB_LOOK=<name>` picks
  another). `GB_HERO=styled`: `anim/hero_styled.glb` through `ModelRequest` alone. `GB_HERO=mannequin`:
  the old yellow ual2.
- The body file is the `ModelRequest.path`, so `motion::rig` binds it exactly like ual2 (descendant
  `Name`s → canonical indices; 65/65 bones bound in the headless run). Parts load as separate scenes
  under the model; their `SkinnedMesh.joints` are re-pointed at the rig's bone entities by name, the
  mesh nodes move under the model, mount nodes onto the rig bones, and the part's own skeleton copy
  is despawned. Swapping a part = changing the look; nothing is re-exported.
- Materials named `hero_*` are copied per character and recoloured from the look (skin tone, hair
  colour on hair/brows/beard, garment colours, eye texture). `motion::rig`'s `tint` skips `hero_*`
  materials, so a tint never washes the kit (the player has no tint anyway).
- Morph weights (`shape`) are written to every `MorphWeights` of the body and the garments.
- NPCs keep the mannequin. `appearance::dress(commands, root, wardrobe.appearance("smith")?)` puts
  a look on any character root.

## 8. Armour, capsules, hands

- `play/armor.rs` builds its plates from `motion::MODEL_GLB` (still `anim/ual2.glb`), and the armour
  stand is still the ual2 mannequin. Plates are fitted per frame from skin samples of the ual2 body
  driven by `RigPose`, which depends only on the skeleton, so they follow the hero exactly as they
  followed the mannequin. **But the hero is bulkier than the mannequin the clearances were designed
  for**: male upper arm +8 mm, thigh +12 mm, torso +7 mm, plus 1.5 cm of tunic. Vambrace / greave gaps
  are 1.2–1.3 cm, so sleeves and trousers will show through arm and leg plates, and hair (long, buns)
  through the bascinet (gap 1.6 cm). Not handled yet: hide the covered garments and hair while a
  plate set is worn, or build the set from the hero body.
- `motion/body.rs` capsules are unchanged (inner capsules, below the skin). With the bulkier male
  skin plus cloth, arms can visually touch the torso slightly earlier than the capsules react.
  `BodyPadding` belongs to `armor`, so it was not touched.
- `motion/hand.rs`: finger bones are ual2's, so curl axes and fingertips are unchanged; finger skin is
  1–3 mm thicker than ual2's (`FINGER_R` 0.009), so closed fingers can sink 1–3 mm into a handle.
  Gloves are fingerless; the palm gets 2–5 mm of leather.

## 9. Penetration under real clips (`tools/model/verify.py`)

**Static checks (all 34 exported GLBs):** joint names and order identical to ual2, parent indices
identical, inverse bind matrices identical (max abs difference 0.0), skeleton node TRS identical
(0.0), weight sums 1 ± 4.8e-8, every joint index < 65, morph targets 4 on bodies and garments, 0
on hair; mounts present (`sheath_hip_l`, `sheath_hip_r` on the belt, `sheath_back` on the baldric,
all three in `hero_styled.glb`).

**Clips:** 710 poses: every clip of ual1 (45), ual2 (43) and sekiro (57, through the same world-space
bind-delta Mixamo retarget as `motion/retarget.rs`) at 5 evenly spaced times; ual clips copy local
rotations by name and the pelvis translation, like the game. Not covered: the game's springs,
contact IK, body capsules and hand grips on top of the clips. As a stand-in for the F1
Exaggeration knob, a second pass pushes every rotation ×1.3 away from rest. Morph passes use every
third pose.

**Measure:** for a pair "A > B" (A under B), every A vertex lying under B's outer surface in the rest
pose (more than 1 cm in from B's edges) is tied to that point of B (triangle + barycentrics). In each
pose the signed distance from A's vertex to the posed point along the posed face normal is taken;
positive means A pokes through B. Pairs whose rest point rides different bones (skin weights differ
by L1 ≥ 0.5: crotch to the other leg, torso side to the sleeve) are dropped as limb-on-limb contact,
which is the body capsules' job, not layering. "visible body" counts only body vertices that are *not*
hidden for that garment (the 2–3.5 cm band along its edges). Hair "<" garment is measured along the
skin normal: garment height minus hair height over the same skin point.

| Pair | base | probes | worst mm (pose) | p99 mm | share through | visible body worst | exaggerated 1.3 worst/p99 | bulk+heavy worst/p99 | slim worst |
|---|---|---|---|---|---|---|---|---|---|
| body>tunic | male | 1684 | 44.27 (sekiro/Zombie Death Backwards@1.50) | 6.28 | 1.50 % | 49.85 | 43.3 / 9.2 | 49.85 / 17.47 | 43.71 |
| body>shirt | male | 1264 | 43.38 (sekiro/Zombie Death Backwards@1.50) | 5.92 | 1.36 % | 33.54 | 42.27 / 8.4 | 44.18 / 16.81 | 42.7 |
| body>trousers | male | 1470 | 16.84 (ual1/Push_Loop@1.33) | -2.29 | 0.50 % | 15.62 | 18.0 / -0.96 | 56.77 / 3.17 | 14.76 |
| body>boots | male | 1168 | 17.26 (ual1/Roll@0.00) | -5.94 | 0.03 % | -19.97 | 17.2 / -5.94 | 17.2 / -5.94 | 17.03 |
| body>shoes | male | 879 | 14.33 (ual2/Shield_Dash@0.00) | -5.61 | 0.02 % | 14.33 | 12.93 / -5.01 | 13.66 / -5.3 | 14.13 |
| body>gloves | male | 721 | 25.61 (sekiro/Zombie Smash@1.17) | 6.37 | 2.31 % | 26.02 | 26.02 / 7.72 | 25.48 / 7.95 | 25.43 |
| trousers>tunic | male | 534 | 34.9 (sekiro/Inward Slash@1.05) | -4.48 | 0.23 % |  | 32.84 / -3.25 | 40.98 / 5.42 | 27.8 |
| trousers>shirt | male | 495 | 11.7 (ual2/Slide_Loop@1.50) | -3.79 | 0.10 % |  | 16.52 / -3.33 | 21.15 / -1.54 | 10.26 |
| trousers>boots | male | 891 | 6.25 (sekiro/Magic Action Energy Beam@1.68) | -3.36 | 0.02 % |  | 7.51 / -3.33 | 6.65 / -3.44 | 5.87 |
| trousers>shoes | male | 607 | 5.33 (ual2/Shield_Dash@0.00) | -3.25 | 0.00 % |  | 5.32 / -3.23 | 4.9 / -3.08 | 5.74 |
| trousers>belt | male | 442 | 15.8 (sekiro/Zombie Death Forwards@2.52) | -13.89 | 0.00 % |  | 14.36 / -12.91 | 27.18 / -7.93 | 0.0 |
| gloves>tunic | male | 396 | 5.3 (ual1/Push_Loop@0.00) | -8.05 | 0.00 % |  | 7.31 / -8.0 | 8.02 / -8.24 | 4.11 |
| tunic>belt | male | 197 | 10.33 (sekiro/Zombie Death Forwards@2.52) | -5.03 | 0.01 % |  | 9.49 / -4.66 | 16.05 / -2.74 | 1.35 |
| shirt>belt | male | 489 | 13.23 (sekiro/Death@3.93) | 1.54 | 2.45 % |  | 13.86 / 2.44 | 19.87 / 1.22 | 11.78 |
| tunic>baldric | male | 299 | 30.66 (ual2/Melee_Hook@0.12) | 0.94 | 3.05 % |  | 34.8 / 1.33 | 25.88 / 1.39 | 12.52 |
| shirt>baldric | male | 273 | 30.89 (ual2/Melee_Hook@0.12) | 0.87 | 3.84 % |  | 34.96 / 0.88 | 26.5 / 1.51 | 11.73 |
| baldric>belt | male | 225 | 0.0 | -7.45 | 0.00 % |  | 0.0 / -7.02 | 0.0 / -6.49 | 0.0 |
| hair_long<tunic | male | 148 | 10.17 (sekiro/Magic Action Energy Beam@1.68) | -10.58 | 0.02 % |  | 20.57 / -9.85 | 10.52 / 2.6 | 0.0 |
| hair_long<shirt | male | 148 | 9.11 (sekiro/Magic Action Energy Beam@1.68) | -11.76 | 0.02 % |  | 19.6 / -10.96 | 9.38 / 1.48 | 0.0 |
| hair_long<body | male | 474 | 42.58 (sekiro/Magic Death Forwards@1.85) | -22.02 | 0.05 % |  | 44.08 / -19.27 | 17.95 / -9.71 | 0.0 |
| hair_buns<tunic | male | 24 | 0.0 | -22.74 | 0.00 % |  | 6.15 / -18.2 | 0.0 / -12.36 | 0.0 |
| hair_buns<body | male | 56 | 0.0 | -26.48 | 0.00 % |  | 0.0 / -25.21 | 0.0 / -20.29 | 0.0 |
| body>tunic | female | 1959 | 31.42 (sekiro/Sitting Idle@2.72) | 3.23 | 1.37 % | 62.99 | 44.76 / 7.62 | 62.99 / 20.67 | 56.96 |
| body>shirt | female | 1491 | 25.3 (sekiro/Zombie Walk@3.05) | 4.25 | 1.61 % | 30.92 | 26.74 / 8.47 | 30.92 / 20.43 | 25.44 |
| body>trousers | female | 1489 | 26.25 (ual2/Sword_Regular_C@1.00) | -1.93 | 0.69 % | 9.64 | 41.1 / 0.61 | 42.3 / 6.25 | 21.91 |
| body>boots | female | 1114 | 17.17 (ual1/Push_Loop@0.67) | -3.14 | 0.07 % | -23.29 | 17.39 / -3.14 | 16.65 / -3.14 | 17.28 |
| body>shoes | female | 852 | 14.06 (sekiro/Kneel to Stand@0.82) | -3.13 | 0.04 % | 14.13 | 14.34 / -3.03 | 13.71 / -3.11 | 14.13 |
| body>gloves | female | 660 | 20.68 (sekiro/Finisher on Kneeling Victim@1.24) | -1.29 | 0.77 % | 27.16 | 24.89 / -0.42 | 27.16 / -0.74 | 20.6 |
| trousers>tunic | female | 497 | 31.34 (sekiro/Run-Forwards@0.55) | 0.34 | 1.07 % |  | 38.8 / 2.72 | 40.63 / 12.78 | 41.28 |
| trousers>shirt | female | 464 | 16.09 (ual1/Roll_RM@0.73) | -0.47 | 0.91 % |  | 19.24 / 1.67 | 17.28 / 8.4 | 15.94 |
| trousers>boots | female | 850 | 6.57 (ual2/Melee_Hook@0.12) | -3.95 | 0.06 % |  | 6.58 / -3.92 | 6.55 / -4.01 | 6.56 |
| trousers>shoes | female | 594 | 3.77 (ual2/Sword_Regular_C@0.00) | -3.89 | 0.00 % |  | 5.59 / -3.86 | 3.86 / -3.96 | 3.68 |
| trousers>belt | female | 418 | 31.32 (sekiro/Kneel Idle@0.22) | -2.2 | 0.73 % |  | 30.4 / -0.41 | 39.21 / 15.88 | 29.89 |
| gloves>tunic | female | 422 | 0.0 | -9.84 | 0.00 % |  | 0.0 / -9.84 | 0.0 / -9.98 | 0.0 |
| tunic>belt | female | 178 | 23.85 (ual1/PickUp_Table@0.21) | 3.57 | 1.85 % |  | 23.51 / 4.31 | 29.99 / 12.06 | 22.95 |
| shirt>belt | female | 326 | 13.02 (ual2/Sword_Regular_A@0.33) | 0.31 | 1.07 % |  | 13.59 / 0.98 | 17.13 / 9.18 | 10.92 |
| tunic>baldric | female | 287 | 13.47 (ual1/Fixing_Kneeling@3.90) | 0.82 | 3.77 % |  | 16.5 / 0.9 | 21.07 / 11.5 | 9.39 |
| shirt>baldric | female | 272 | 14.28 (ual1/Fixing_Kneeling@3.90) | 0.7 | 1.74 % |  | 15.84 / 1.39 | 21.65 / 7.36 | 9.97 |
| baldric>belt | female | 129 | 5.87 (sekiro/Death@0.98) | -2.05 | 0.24 % |  | 7.5 / -2.5 | 5.85 / 0.38 | 3.45 |
| hair_long<tunic | female | 162 | 0.17 (sekiro/Magic Action Energy Beam@2.52) | -13.93 | 0.00 % |  | 7.42 / -13.24 | 10.55 / 6.87 | 0.0 |
| hair_long<baldric | female | 28 | 1.92 (sekiro/Magic Action Energy Beam@2.52) | -17.08 | 0.04 % |  | 9.38 / -14.42 | 4.32 / -3.06 | 0.0 |
| hair_long<body | female | 160 | 1.85 (sekiro/Magic Death Forwards@1.85) | -32.5 | 0.00 % |  | 47.12 / -29.46 | 24.34 / -13.89 | 0.0 |

Short hair (buzzed, parted) never lies on the clothes or torso (0 probes). The full JSON, with
the shirt rows and every pair's worst pose, is `assets/characters/hero/verify_report.json`.

**Reading it:** the median vertex is well inside (negative p99 on most pairs), and 0–4 % of
vertex-poses poke through at all. The worst cases are extremes:
- body>tunic / shirt (≈ 44 mm, male): the **armpit** skin (spine_03 / upperarm / clavicle blend)
  in death and dive clips where the arm is driven through the torso. These vertices are hidden under
  the tunic. The worst *visible* body points are the groin under the tunic hem when a thigh is raised
  high (Inward Slash) and the nape against the back of the collar when the head is thrown back
  (OverhandThrow), 30–50 mm.
- trousers>tunic (≈ 31–35 mm): the same hip flexion at the tunic hem; the skirt follows the thigh
  with the skin weights, so a high knee can push the trousers through its front.
- tunic/shirt>baldric (≈ 30 mm, male, Melee_Hook): the strap crosses the shoulder where the clavicle
  and upper arm weights blend.
- body>gloves (≈ 21–26 mm): finger-base skin at a full fist, just inside the knuckle edge.
- hair_long<body (43 mm male, 0.05 % of samples): hair against the upper back in a forward death fall.
- With the `bulk + heavy` morphs the p99 rises to 12–20 mm on tops: skin in hollows that the drape
  spans swells under `heavy`. Hair has no morphs and was fitted against the cloth at full bulk +
  heavy, so it stays clear.
