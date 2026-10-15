# Animation relationships (hero + NPC), from stored audit/sweep/profile data

Sources read (no code run, no game, no Blender): collab/pose_audit.json (392 findings on fighter.glb post-pipeline,
417 source findings, 4 patterns), collab/possess_sweep.json (3 reports: 15 fps x2 clips, 10 fps x24, 30 fps x24),
rig.json for hero / source RPG / female_peasant / male_peasant, fighter.glb.body.json, fighter.glb.motion.json,
collab/studio_notes.jsonl (6 notes), Tools/blender/hero_mapping.json, glb mesh names (header only).

"Used" = a clip named in hero_mapping.json combat, roles, locomotion, locomotion_relaxed, spells or mage: 99 clips.
Audit lists findings for 61 of them (the other 38 are clean or not audited). Sweep covers only 24 used clips
(Unarmed-* family); nc_*, hp_/bp_/hk_/lk_/bk_, mage_*, mx_*, ual_* are NOT swept yet.

Conventions: pop = one-frame joint-angle jump (deg, with clip median); "inherited" = same finding exists in the
source clip (introduced=false). Of 392 hero findings only 4 are introduced; 388 are inherited.

---------------------------------------------------------------------------------------------------------
## 1. Relationship map (used clips only)

### 1a. Findings by check (used clips, audit)
| check | findings | clips | note |
|---|---|---|---|
| pop | 218 | 56 | 83% of all findings |
| knee_flex (>135) | 21 | 13 | 15 hero + 6 kevin-source |
| range | 14 (hero) | 12 | 7 bones x clips, see 1d |
| knee_back | 10 | 6 | all < 0.35 deg, negligible |
| ground | 3 hero + 4 kevin | 2 hero (Unarmed-Jump, mage_burst) | toes 2-7 cm under floor |
| torso (hand in body) | 1 | 1 (Unarmed-Attack-L1) | R hand 0.045 m from spine @0.667 s |
| source (introduced) | 4 | 4 | Pelvis->Spine 10.6-11.3 deg |

### 1b. Patterns ranked by used clips touched
| rank | pattern | used clips | inherited? | clip families |
|---|---|---|---|---|
| 1 | arm-chain pops (clavicle->upperarm, upperarm->forearm, forearm->hand) | 38 | inherited | kick 7, UA-Attack 6, punch 6, mage 5, mx 5, UA-other 4, UA-RunAtk 2, ual 2 |
| 2 | foot->toe pop (toe flick 57-87 deg in 1 frame) | 28 | inherited (Kevin raw shows same 57-81) | kick 12, nc 8, UA-Run 5, UA-other 2, mx 1 |
| 3 | leg-chain pops (thigh->calf, calf->foot) | 21 | inherited | kick 12, nc 3, UA-Attack 2, UA-other 2 |
| 4 | knee flex over 135 (136-153 deg) | 13 | inherited | nc sprint 3, UA-other 3, mx 2, kick 2, UA-RunAtk 2, UA-Run 1 |
| 5 | neck/spine head-chain pops | 12 | inherited | UA-Attack 4, UA-other 2, mage 2, kick 2 |
| 6 | joint range exceeded | 12 | inherited | UA-other 3, mage 3, UA-Run 3, UA-Attack 1, ual 1 |
| 7 | knee bends backward (<0.35 deg) | 6 | inherited | UA-other 3, mx 2, ual 1 |
| 8 | Pelvis->Spine 11 deg off source | 4 | INTRODUCED | Run-Forward, Run-Forward-Attack1-Left/Right, Strafe-Backward-Right |
| 9 | toe/foot below floor | 2 (+nc_jump_begin raw) | inherited | Unarmed-Jump, mage_burst |
| 10 | hand inside torso | 1 | inherited | Unarmed-Attack-L1 |

nc_* caveat: nc_run_forward/-forwardleft/-forwardright/-left/-right and nc_sprint_* share one source take, so their
findings are identical copies (e.g. R toe 57.5 deg @0.278 appears in 5 clips). Real unique nc sources: run, sprint,
jump_begin, jump_land. Counts above overstate nc by about 3x.

### 1c. Which bones fail together (co-occurrence over 56 clips with pops)
| pair | clips |
|---|---|
| L clavicle->upperarm + R clavicle->upperarm | 17 |
| L upperarm->forearm + R upperarm->forearm | 16 |
| L forearm->hand + R upperarm->forearm | 14 |
| R forearm->hand + R upperarm->forearm | 13 |
| L forearm->hand + L upperarm->forearm | 13 |
| R foot->toe + R thigh->calf | 12 |
| L foot->toe + R foot->toe | 10 |
| R calf->foot + R foot->toe | 10 |
- The arm chain pops as a unit (3-6 bones in the same frame). 19 clips have >=3 bones popping in the same frame;
  worst: Unarmed-Attack-R2 (2 frames), Unarmed-Jump (2), Unarmed-Attack-R3 (2).
- Left and right chain pop together in 12-17 clips although only one fist strikes: the off-hand also snaps
  (shoulder counter-rotation is authored). Run-Forward-Attack1-Left and -Right are exact mirrors (148.759 / 127.24).
- Legs: foot->toe co-pops with thigh->calf in kicks (the kick snap), with calf->foot in runs (toe-off).

### 1d. Timing: strikes
- 78 of 81 arm pops in clips with strike data fall in [contact_start-0.15 s, contact_end+0.05 s].
  They are the authored punch extension, not a bake artifact (docs/STUDIO_LEARNINGS #2 already fixed sampling).
- Peak arm pops (deg in one frame, median in brackets):
  Run-Forward-Attack1 upperarm->forearm 148.8 (9.9) and clavicle->upperarm 127.2 (6.1) @0.333 s;
  Attack-L3 UA->FA 121.3 @0.458; Attack-R2 UA->FA 120.0 @0.375; Attack-R1 UA->FA 110.7 and FA->hand 110.0 @0.417;
  mage_lightning_left UA->FA 109.0 @0.333; Attack-L2 FA->hand 107.1; mage_lightning_right 105.0; Attack-L1 UA->FA 104.2 @0.417;
  mage_burst FA->hand 92.0 @0.217.
- Strike contact_peak (motion.json): L1 0.406 (ext 0.90 m), L2 0.375 (0.61), L3 0.469 (0.94), R1 0.406 (0.91),
  R2 0.375 (1.04), R3 0.406 (0.85), Run-Atk 0.344 (0.86-0.87).
- Start-of-clip pops (t<0.05 s): Unarmed-Jump 0.029 s (6 bones, 53-87 deg, thigh->calf 93), Unarmed-Knockdown1 0.022 s
  (4 bones, 26-38), DiveRoll 0.025 (hand 61). First-frame discontinuity, not motion.

### 1e. Range failures (hero rules joints.rs table)
| clip | bone | bend vs limit | t |
|---|---|---|---|
| ual_hit_knockback | B_Spine2 | 89 vs 50 (+39.4) | 0.00 |
| Unarmed-Knockdown1 | B_Neck | 82 vs 70 (+11.9) | 0.33 |
| Unarmed-Getup1 | B_Neck | 81 vs 70 (+11.1) | 0.31 |
| mage_burst | L forearm.001 / R hand | 88 / 87 vs 80 (+8.1 / +6.6) | 0.44 |
| Unarmed-Strafe-Left | B_Spine | 44 vs 40 (+3.5) | 0.25 |
| Unarmed-Run-Backward-Right / Run-Left / Jump | foot | 12 vs min 15 (-3.4/-3.0/-3.0) | various |
| mage_lightning_left/right, DiveRoll | wrist | 83/81/83 vs 80 | 0.44-0.5 |
| Unarmed-Attack-R2 | B_Spine1 | 38 vs 35 (+2.6) | 0.25 |
Neck and wrist are the only systematic ones; spine/foot are single clips.

### 1f. Knee flex over 135 (hero knee_max_flex_deg = 135)
hk_side_left_A/A2 L 153.3 @0.238 (+18.3); Run-Forward 149.5 L @0.417 / 146.3 R @0.917, same values copied into
Run-Forward-Attack1-Left/Right (+13.5/+11.3); Getup1 142.6/142.2; Jump 142.4/138.3; mx_death 138.8; Fall 136.2;
nc_sprint_* 135.7 (+0.7, trivial); mx_death_backstab 135.7. Run knees at 146-149 are authored foot-to-glute kick-back
at speed, not an error; 135 is the cause.

---------------------------------------------------------------------------------------------------------
## 2. T-pose / bind reference per model

Reference = hero (fighter.glb, Male_Ranger outfit): height 1.661 m, mass 58.9 kg, knee max flex 135, look yaw 52 / pitch 46.
Hero rest bends (deg) and rule windows (min-max): rule window = hero window shifted by (rest - hero rest), min clamped at 0.

| joint | rest | window |
|---|---|---|
| B_Spine | 10.5 | 0-40 |
| B_Spine1 | 4.5 | 0-35 |
| B_Spine2 | 14.3 | 0-50 |
| B_Neck | 29.8 | 0-70 |
| L/R UpperArm | 28.9 | 0-150 |
| L/R Forearm | 3.5 | 0-155 |
| L/R wrist (Forearm.001 / Hand) | 6.1 | 0-80 |
| L/R Foot (ankle) | 69.8 | 15-145 |

Hero bind lengths (m): Spine 0.1234, Spine1 0.1024, Spine2 0.1282, Neck 0.2072, Head 0.0798, Clavicle 0.182,
UpperArm 0.202, Forearm 0.2418, Hand 0.2346, Finger2 0.1122, Thigh(hip offset) 0.1123, Calf 0.413, Foot 0.4418,
Toe0 0.1532, ball_leaf 0.076, B_Pelvis 0.915 (height of root). Fingers: 5 per hand; palm: L thumb side, R opposite thumb.
Hero torso sections (rig.json): pelvis a 0.1203 b 0.1429; mid a 0.1376 b 0.1183; chest a 0.0991 b 0.1235.

### 2a. male_peasant: identical to hero
All 12 rest bends and all 64 bone lengths match hero exactly (0.0 deg, 0.0%). Height 1.682, mass 61.2 (only the uniform
scale differs: normalize_scale 0.976 vs hero outfit). Torso sections differ (pelvis a 0.1477 b 0.1212; mid a 0.1484
b 0.1191; chest a 0.1617 b 0.1222), but no rule shifts. No joint_fit in its profile (not generated).

### 2b. female_peasant: bones differ, rests close
Height 1.684, mass 61.4. Rest diffs vs hero (>3 deg marked *):
| joint | hero | female | diff |
|---|---|---|---|
| B_Neck* | 29.8 | 36.0 | +6.2 |
| L/R Foot (ankle)* | 69.8 | 73.4 | +3.6 |
| B_Spine* | 10.5 | 7.3 | -3.2 |
| B_Spine1 | 4.5 | 6.4 | +1.9 |
| B_Spine2 | 14.3 | 11.6 | -2.7 |
| wrist | 6.1 | 4.7 | -1.4 |
| Forearm | 3.5 | 2.7 | -0.8 |
| UpperArm | 28.9 | 29.2 | +0.3 |
Rules that therefore shift: Neck window 6.2-76.2 (hero 0-70); ankle 18.6-148.6 (hero 15-145); Spine 0-36.8 (hero 0-40).
Bone length diffs >5% (female vs hero): UpperArm -29.3% (0.1429), Clavicle -20.2% (0.1453), Head -10.0% (0.0718),
Finger2 -21.7%, finger roots index/pinky/ring_01 -19.6..-22.3%, thumb_01 -44.5%, Foot +6.5% (0.4703), Hand(forearm.001)
+7.2% (0.2515), Spine1 +8.7%, ball leaf -9.6%, thumb_02/04 -5%. Calf, Forearm, Spine, Spine2, Neck, Toe within 5%.
Consequence: with hero-retargeted clips her upper arms are 29% short, so hand reach shrinks about 0.06 m at the shoulder
while forearm is +0 (hands end 0.06 m closer to the body: higher torso-clip risk and 0.06 m less punch reach; her forearm+hand is +0.017 m).

### 2c. Source RPG model (assets/packs/_sources/rpg_hero) vs hero: the retarget gap
Height 1.708, mass 64.1; different skeleton (52 length entries, Finger0/01/02 naming, no palm info). Rest diffs vs hero:
| joint | hero | source | diff |
|---|---|---|---|
| UpperArm L/R* | 28.9 | 0.0 | -28.9 (T-pose vs A-pose) |
| Neck* | 29.8 | 3.8 | -26.0 |
| Spine* | 10.5 | 20.2 | +9.7 |
| Spine2* | 14.3 | 5.2 | -9.1 |
| Spine1* | 4.5 | 0.0 | -4.5 |
| Foot* | 69.8 | 64.7 | -5.1 |
| Forearm* | 3.5 | 0.2 | -3.3 |
| wrist | 6.1 | 9.0 | +2.9 |
Bone lengths >5%: Clavicle -49.8% (0.0913), Spine -44.7% (0.0682), Thigh -28.2%, Toe0 +28.6%, Spine2 +36.3%,
Spine1 +19.0%, Neck +12.4%, UpperArm -6.9%, Foot -6.9%, Head -5.3%, Finger2 +8.1%. This is why the rest-direction
alignment in build_hero.retarget exists (STUDIO_LEARNINGS #1) and why source-vs-hero comparisons are only meaningful
after that alignment. Source torso section is degenerate (pelvis b 0.0133, cy -0.1153) - do not use as a reference.

### 2d. Rangers
male_ranger.glb / female_ranger.glb have only report.json (npc set, 9 npc_* clips, scales 0.963 / 1.001); no rig.json.
Hero fighter.glb is itself the Male_Ranger outfit (meshes: Pauldron, Arms, Arms_Bracer, Body, Belt_1 x2, Boots, Head_Hood, Legs).
Gap: generate rig.json for both rangers before applying rule shifts to them (assume hero rules until then).

---------------------------------------------------------------------------------------------------------
## 3. Clipping mitigation (arms/hands into torso)

### 3a. Geometry (assumes section a = lateral half-width, b = front-back half-depth; confirm in rig_profile.rs)
body.json torso sections (self-collision) and inflated ellipse for a capsule of radius r: a+r, b+r.
| section | a / b | + forearm r 0.0605 | + hand r 0.0461 | + upperarm r 0.0637 (R) |
|---|---|---|---|---|
| pelvis | 0.1305 / 0.1487 | 0.191 / 0.209 | 0.177 / 0.195 | 0.194 / 0.212 |
| spine to spine2 | 0.1376 / 0.1183 | 0.198 / 0.179 | 0.184 / 0.164 | 0.201 / 0.182 |
| spine2 to neck (chest) | 0.0991 / 0.1235 | 0.160 / 0.184 | 0.145 / 0.170 | 0.163 / 0.187 |
Limb radii: L upperarm 0.0989, R upperarm 0.0637, forearms 0.0605, hands 0.0461.
Anomaly: L upperarm radius is 55% larger than R (0.0989 vs 0.0637). Cause is mesh, not anatomy (see 3c).
Also body.json pelvis a/b (0.1305/0.1487) is 8.5%/4.1% larger than rig.json (0.1203/0.1429): the belt is in body.json.

### 3b. Evidence from audit and notes
- Audit torso finding: Unarmed-Attack-L1, B_R_Hand 0.045 m from spine axis @0.667 s (recovery guard).
  Chest inflated minor axis for the hand is 0.145 m, so the hand centre is about 0.10 m inside the section.
- Studio notes (boss, with pose numbers): Unarmed-Idle t=0 "WHOLE model not aligned" (R forearm 112 deg, R upperarm 69,
  L forearm 96; guard fists at chest, pelvis 0.80 m); mage_fireball t=0 "better but still clips" (R forearm 81, upperarm 66);
  Unarmed-Attack-L1 @0.5 (R forearm 127, R upperarm 89, L knee 114); Unarmed-Attack-L1 index_02_l (fingers manipulable);
  Unarmed-Attack-R1 @0.65; Run-Forward-Attack1-Left B_R_UpperArm (L knee 135, ankle 0.618 m - flight frame).
- Pattern: forearm flexion >95 deg with upper-arm bend 60-90 deg (guard stance) puts the fist at chest depth.
  That combination is in Unarmed-Idle (the game's idle/guard/spell_charge role), mage_fireball, Attack-L1/R1/R2 recoveries,
  hp_/bp_ hooks and uppercuts (hand returns across the chest), mx_block, ual_guard_break.
- Fists: grip 0.0 -> 1.0 shrinks palm span from 0.176 m to 0.107 m (sweep life). A closed fist is 0.069 m thinner
  than the open hand the capsule was fit to, so use hand r 0.046 only for open; use 0.040 for grip>0.7.

### 3c. Outfit pieces that bias the fit (exclude from joint_fit and radii)
| mesh | evidence | action |
|---|---|---|
| Male_Ranger_Acc_Pauldron | L upperarm: 773 verts, centre_off 0.0409 m, dir +z; R upperarm 119 verts, off 0.0068 -> one-sided piece skinned to L upperarm; L radius 0.0989 vs R 0.0637 | exclude; use R radius 0.0637 for both |
| Male_Ranger_Arms_Bracer | both forearms 2059 verts, off 0.0158 / 0.0157 (symmetric, small) | exclude from fit; keeps radius 0.0605 |
| Male_Ranger_Head_Hood | B_Neck joint_fit side 0.0828 m (82 verts), largest on the rig | exclude; Head itself fits 0.0038 |
| Male_Ranger_Body_Belt_1 (+.001) | B_Spine 0.0514 m (64 verts); body.json pelvis a +0.0102 vs rig.json | exclude from section fit; keep a belt-inflated copy only for collision |
| Boots (in Feet_Boots / Legs) | B_Calf along +0.0666 m on 661 verts, 4102 verts centre cluster; B_Foot 0.0361 | fit calf/foot on skin of Legs only |
| Spine2 cloak/body | B_Spine2 side 0.0671 (829 verts), Spine1 0.0522 (1437) | fit torso from Body mesh only, drop accessory verts |
Female peasant has no accessories: joint_fit offsets are 0.0097-0.0468 (Toe0 0.0468 on 14 verts is noise; require >=40 verts).
Rule: ignore joint_fit entries with fewer than 40 verts or dist_m >0.05 unless symmetric L/R within 0.01 m.

### 3d. Proposed runtime mitigation (soft push-out, per frame after joint rules, in crates/game/src/joints.rs or support.rs)
1. Samples per arm: elbow, wrist, hand tip, (forearm mid). Capsule radii: upperarm 0.0637, forearm 0.0605, hand 0.046 (0.040 closed).
2. Transform each sample into the torso section frame (spine segment containing its projection; use the 3 sections above,
   interpolate a, b, cx, cy linearly along the spine).
3. d = sqrt((x/(a+r+0.01))^2 + (y/(b+r+0.01))^2). If d < 1: pen = (1-d) * min(a+r+0.01, b+r+0.01).
4. Soft zone: w = smoothstep(0, 0.03, pen); no hard snap. Response time constant 0.06-0.08 s (critically damped) so
   fast strikes through the body line are not fought.
5. Correction angle at the shoulder (abduct away from the torso centre, shoulder-to-sample lever L):
   theta = min(pen / L, 20 deg) * w. Examples: hand pen 0.10 m, L = 0.202+0.242+0.0 = 0.444 m -> 12.9 deg;
   elbow pen 0.04, L 0.202 -> 11.3 deg; pen 0.02 at wrist, L 0.444 -> 2.6 deg.
6. Split: 70% on the shoulder (clavicle->upperarm hinge), 30% on the elbow (open forearm outward about the elbow axis by
   pen / forearm length 0.2418, max 12 deg). Never exceed rule max_deg.
7. Disable while a strike window is active for that arm (motion.json contact_start-0.05 .. contact_end+0.05): the strike
   line is intentional and the pops are inherited; clip authors cross the body in hooks.
8. Female peasant: UpperArm is -29% (0.1429), so the same pen costs 1.4x the angle: scale L from the profile lengths, do not hardcode.
9. Per-region caps: chest (spine2->neck) section is narrowest laterally (a 0.099), so highest risk there: use margin 0.015 for
   frames where forearm bend >95 deg (idle/guard family).
Data to check first (offline, from existing rig/body json): fraction of frames of each used clip where d < 1; start with
Unarmed-Idle, mage_fireball, Attack-L1/R1, hp_hook_left_A, bp_hook_right_A, mx_block, ual_guard_break.

---------------------------------------------------------------------------------------------------------
## 4. Physics relationships from the sweep

Sweep reports: 15 fps (2 clips), 10 fps (24), 30 fps (24). Use 30 fps as authoritative. At 10 fps momentum jumps are
inflated 9x (e.g. Attack-L1 141 g vs 14.6 g at 30 fps, factor roughly (30/10)^2); do not compare fps.
Frame-2 artifact (known): every clip shows a momentum jump at frame 2: Run-* 11.5-15.1 g (Run-Forward 15.1),
Attack-L2 10.9, Strafe-Backward 6.8-7.1, Attack 4.0-5.9, Run-Atk 6.2, DiveRoll 3.0, Knockdown1 8.1, Jump 40.6.

### 4a. Real jumps over 3 g (frame != 2, 30 fps)
| clip | n>3g | max (g @ frame / s) | other |
|---|---|---|---|
| Unarmed-Jump | 15 | 96.1 @3 (0.10 s), 51.1 @4, 32.5 @22 (0.73 s), 20.1 @23 | peak speed 18.2 m/s; takeoff and landing, plus the t=0.029 first-frame pop and toe ground -0.07 m |
| Unarmed-DiveRoll-Forward1 | 19 | 18.1 @26 (0.87 s), 10.3 @9, 6.9 @27, 6.0 @14, 5.8 @34 | peak speed 8.24 |
| Unarmed-Attack-L1 | 5 | 14.6 @8 (0.27 s), 10.7 @10, 9.6 @11, 8.1 @12 | strike |
| Unarmed-Attack-R3 | 5 | 13.8 @10 (0.33 s), 11.3 @8, 9.1 @11 | strike |
| Unarmed-Attack-R1 | 4 | 13.4 @12 (0.40 s), 10.3 @10 | strike |
| Unarmed-Attack-R2 | 5 | 12.2 @8 (0.27 s), 9.3 @10, 8.2 @9, 7.1 @24 (loop end) | strike |
| Unarmed-Attack-L3 | 6 | 10.2 @13 (0.43 s), 7.7 @10 | strike |
| Unarmed-Knockdown1 | 7 | 10.1 @4 (0.13 s), 5.4 @8, 5.4 @24 | hit impulse, B_R_Foot violation |
| Unarmed-Run-Forward-Attack1-L/R | 8 | 7.9 @24 (0.80 s), 6.2 @6, 5.5 @12, 4.2 @11 | same as Run-Forward at frames 6/12/18/24 |
| Unarmed-Run-Forward | 8 | 7.6 @24 (0.80 s), 6.1 @6, 5.5 @12, 4.1 @7/@18 | footfall impacts every ~0.2 s plus loop seam at frame 24 |
| Unarmed-Attack-L2 | 6 | 5.0 @12, 4.4 @9 | frame 2 = 10.9 |
| Unarmed-Getup1 | 2 | 3.9 @29,30 (0.97-1.0 s) | B_L_Foot violation |
| Unarmed-Run-Right | 1 | 3.3 @14 | |
| Run-Backward x3, Run-Forward-Left/Right, Run-Left, Strafe-Backward x2 | 0 | frame 2 only (6.8-12.6 g) | frame-2 artifact only: clean |

Classification:
- Artifact only (frame 2): 9 clips (Run-Backward, -Left, -Right, Run-Forward-Left/Right, Run-Left, Strafe-Backward, -Left).
- Real but expected impacts (footfalls/loop seam, 3-8 g): Run-Forward and Run-Forward-Attack1-L/R (loop seam at frame 24, 7.6-7.9 g
  = closest-to-pop, candidate loop-close fix); Getup1; Run-Right.
- Real strike peaks (10-15 g at 0.27-0.43 s, 0.05-0.1 s BEFORE contact_peak): all 6 Unarmed-Attack. They coincide with the
  authored arm pops of 1b/1d (the extension snaps within one frame, centre of mass lurches because the arm mass is moved).
- Genuine glitches: Unarmed-Jump (96 g at frame 3 = start pose discontinuity, 32 g at landing), DiveRoll (18 g at 0.87 s).

### 4b. Sustained imbalance (unbalanced_frames, 30 fps) vs travel (motion.json stride)
| clip | unbalanced / frames | stride m | peak speed | reading |
|---|---|---|---|---|
| Unarmed-Strafe-Backward / -Left | 23/25 | 1.77 | 2.5-2.8 | suspicious: walking, nearly always flagged; threshold too strict or backward walk leans |
| Unarmed-Run-Backward-Left / -Backward / -Backward-Right | 19 / 17 / 17 of 25 | 3.1 | 4.2-4.3 | run flight phases flagged; also high |
| Unarmed-Run-Left / -Right | 18 / 13 | 3.12 | 4.2-4.7 | |
| Unarmed-Run-Forward / Forward-Left / Forward-Right | 11 / 11 / 9 | 3.0-3.6 | 4.2-5.3 | lowest, near-expected for running |
| Unarmed-Run-Forward-Attack1-L/R | 10/25 | 3.57 | 2.35 | |
| Unarmed-Attack-R2 | 18/25 | 1.32 | 5.76 | lunge; imbalance spans frames 6-17 (0.2-0.57 s) |
| Unarmed-Attack-L3 | 15/25 | 1.06 | 4.11 | frames 9-17 |
| Unarmed-Attack-L1 | 13/25 | 1.09 | 6.61 | frames 4-12, 20-24 |
| Unarmed-Attack-L2 / R1 / R3 | 8 / 7 / 5 | 0.42 / 0.94 / 0.82 | 3.6 / 4.4 / 5.1 | |
| Unarmed-DiveRoll-Forward1 | 17/41 | 4.82 | 8.24 | expected (roll) |
| Unarmed-Getup1 | 24/121 | 0.0 | 1.32 | NOT travelling: frames 19-30, 41-45, 106-115 + violation B_L_Foot |
| Unarmed-Jump | 9/35 | 0.0 | 18.2 | takeoff 2-4, landing 22-31; in-air expected, but 18 m/s is not |
| Unarmed-Knockdown1 | 3/47 | 0.0 | 3.79 | B_R_Foot violation; frames 3-5 |
| Unarmed-Fall / Idle | 0 | 0 | 0.28-0.7 | clean |
Not travelling and unbalanced: Getup1 (frames above), Knockdown1 (3-5), Jump. Idle and Fall are clean.
Travelling clips (all Run, Strafe, Attack lunges) are flagged because the extrapolated centre of mass leaves the support
polygon during flight/lunge; treat Strafe-Backward (23/25 at walk speed) as the one real outlier.
10 fps only: violations B_R_Calf in Attack-L2, Strafe-Backward, Strafe-Backward-Left (not at 30 fps): sampling artifact.
Violations at 30 fps: Getup1 B_L_Foot, Knockdown1 B_R_Foot (only two).
Gap: the sweep does not yet cover any of the 75 other used clips (kicks, punches, mage, mx, nc, ual): run it on
hk_/lk_/bk_ (balance on one leg) and mage_* (casting, hands in front) first.

---------------------------------------------------------------------------------------------------------
## 5. Prioritised fix list (top 10)

| # | issue | clips | evidence | fix | where |
|---|---|---|---|---|---|
| 1 | Hands pass into torso in guard/recovery poses | Unarmed-Idle (idle, guard, spell_charge roles), mage_fireball, Attack-L1/R1, hp_/bp_ hooks and uppercuts, mx_block | Attack-L1 R hand 0.045 m from spine (0.10 m inside chest ellipse); boss notes on Idle, mage_fireball, L1; forearm 96-127 deg with upperarm 60-90 | soft push-out of section 3d (12.9 deg at 0.10 m pen, cap 20 deg, 0.07 s damping) | runtime rule: joints.rs / procedural.rs, uses body.json |
| 2 | Left upper-arm radius polluted by pauldron | all clips, L arm | L upperarm radius 0.0989 vs R 0.0637; joint_fit L 773 verts off 0.0409 vs R 119 / 0.0068; belt in body.json pelvis a +0.0102 | exclude Acc_Pauldron, Hood, Belts, Bracer from fit; use R radius for both; require >=40 verts | bake / profile generator (build_hero.py + rig_profile generation) |
| 3 | Arm-chain strike snaps (3-6 bones in one frame, 80-149 deg) | Unarmed-Attack-L1..R3, Run-Forward-Attack1-L/R (149 / 127 deg @0.333), hp_/bp_ punches, mage_lightning/burst, mx_* | 38 clips, 78/81 within strike window, all inherited; sweep 10-15 g at 0.27-0.43 s | do not smooth the pop; instead cap angular rate in procedural.rs only outside strike window, and verify at 60 fps source whether the 33 ms step is real; per-clip insert half-frame key at 0.333/0.417 for Run-Atk and R1 | per-clip correction + runtime rate cap |
| 4 | Foot->toe flick in one frame | kicks (lk_rh_right_A/A2 87 deg @0.381, Tiramis 85), nc_run (57.5), nc_sprint (81/77), UA-Run (5), mx | 28 clips; Kevin raw has the same 57-81 so inherited; median 10-15 | rate limit toe bend to about 25 deg/frame or low-pass toe channel (3-frame) | bake (retarget filter on Toe0) or runtime in knee.rs |
| 5 | Knee flex above 135 | hk_side_left_A/A2 (153 @0.238), Run-Forward + Run-Atk (149/146), Getup1 (143), Jump (142) | +18.3, +13.5, +11.3, +7.6, +7.4 over | raise knee_max_flex_deg to 150 for run/kick families, or soft-clamp; hk_side_left needs per-clip bring-in | runtime rule knee.rs (+ per-clip hk_side) |
| 6 | Neck range exceeded in falls | Unarmed-Knockdown1 (82 @0.326), Getup1 (81 @0.308) | limit 70, +11.9 / +11.1; hero rest 29.8 | raise B_Neck max_deg to 85 (and wrist to 90: mage_burst 88/87, lightning 83/81, DiveRoll 83) | runtime rule joints.rs |
| 7 | ual_hit_knockback frame 0 spine fold | ual_hit_knockback | B_Spine2 bend 89 vs 50 at t=0 (+39.4), knee_back 0.25 | source frame 0 is a bent pose; trim first frame or clamp to 50 | per-clip correction |
| 8 | Pelvis->Spine 11 deg off source (only introduced finding) | Unarmed-Run-Forward (10.57 @0.875), Run-Forward-Attack1-Left/Right (10.58 @0.375), Strafe-Backward-Right (11.29 @0.542) | 4 clips, source = 0.0; source B_Spine has location keys and rest bend differs (hero Spine 10.5 vs source 20.2) | retarget spine rest-direction compensation (hero 10.5 vs RPG 20.2 = 9.7 deg residual matches 11 deg) | bake pipeline build_hero.retarget |
| 9 | Start-frame discontinuity (jump/knockdown/dive) | Unarmed-Jump (6 bones 53-93 deg @0.029, 96 g @frame 3, toes -0.060/-0.074 m @0.059), Unarmed-Knockdown1 (4 bones @0.022, 10 g), DiveRoll (61 deg hand @0.025, 18 g @0.87) | 11 pops at t<0.05 s | trim the first 2 frames or add a 0.08 s blend-in from the prior pose; lift pelvis to clear toe -0.07 m | per-clip correction (Jump, Knockdown1, DiveRoll) + runtime blend |
| 10 | Run loop seam and imbalance outliers | Unarmed-Run-Forward + Attack1-L/R (7.6-7.9 g @frame 24), Strafe-Backward / -Left (23/25 unbalanced at 1.77 m stride), Getup1 (24/121 not travelling, B_L_Foot violation), Knockdown1 (B_R_Foot) | sweep 30 fps | close loop on Run-Forward (frame 24 to 0), recheck balance threshold for backward walk, floor-clamp feet in Getup1/Knockdown1 | bake (loop close) + runtime support.rs |

Harness fixes (not game data): drop the frame-2 momentum entry from sweep reports (known artifact, inflates 9 run clips and every
attack), report g only from the 30 fps run, and extend the sweep to kicks, punches, mage and nc clips (75 unswept used clips).
Generate rig.json for male_ranger and female_ranger (missing); regenerate female_peasant joint_fit with >=40 verts rule.
