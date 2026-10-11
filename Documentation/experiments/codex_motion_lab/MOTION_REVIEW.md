# Owned motion lab - implementation handoff

Owner: Codex. Runtime rules and acceptance fixtures: Claude.

## Verified evidence

- 20 owned FBX clips evaluated in Blender on the derived HumanM body.
- Five captures per clip at actual authored key timestamps nearest quarter intervals, with the exact frames and imported key lists in results.json.
- FightingMotions, Kevin Iglesias and Brawler have separate explicit bone mappings. Casting's extra animated FBX root container needs axis/scale normalization. Brawler has a different bind pose and forward axis; this study transfers absolute bone directions with yaw alignment. Axial twist remains unverified.
- A 93-frame review sequence at 30 fps: Brawler light 1, light 2, home stance. Four-frame cubic-weight handovers are separate from authored clip duration. A 120ms starting value becomes 133ms at this capture rate.
- Derived vest now follows the armature root through an explicit preserved bind parent. The earlier static laboratory did not exercise root motion.
- Source geometry and animations remain untouched. The review .blend is local under the previous character laboratory's derivative directory.
- Claude's independent spatial audit caught the remaining casting-up-axis mismatch after container correction. Corrected source rest/pose/translation alignment; the delivered sampler exports actual source and target joint positions. Independent comparison in actual_export_measurements.json reports worst casting joint-angle RMS 0.005396 degrees, below the 0.1-degree regression threshold. This verifies that correction, not the runtime gates.

## Choreography decisions

1. Home stance is a loaded, relaxed guard. Held guard is a distinct higher forearm cover; returning to stance must not automatically activate guard capacity.
2. Light 1 -> light 2 is the first phrase. Each verified contact gets its own action identity. Curate the first follow-through into the second preparation; the current montage preserves complete clips and is a comparison baseline, not the final string.
3. Input queues in late recovery for up to 0.25s (H). Holding, blending or buffering does not cancel committed preparation, active frames or recovery.
4. Hit, loaded block and miss need different bodies. Verified contact may hold involved visual poses for 25-45ms (H). Misses complete their authored arc without a hold or spark. Logical clocks keep advancing during the visual hold.
5. Heavy has a readable chamber and a punishable exit. Step must hand support between feet; no roll is accepted as a lateral step merely because it can be retimed.
6. Throw reach-in remains interruptible; both paired clips are separately sampled here. The montage proves neither legal staging nor collision-safe landing. Claude's motor/staging check authorizes the paired motion.
7. Cast preparation, charge, release and interruption remain distinct. The charge/release endpoints need curated handover; a guessed midpoint is not a verified release/contact frame.

## Healing and recovery

Confirmed v5.1: defeat/abandon returns to the hall bench at full HP; retry and hall recovery cost zero. One optional preparation item shortens out-of-combat recovery and is never required. No passive regeneration or combat heal is specified.

Requested from Claude: define threat/out-of-combat checks, duration, interruption, HP amount and item consumption as one completion rule. Proposal for review: completion changes HP and consumes an optional item once; interruption changes neither. Proposed presentation: settle/breathe, a clear progress indicator, one completion accent; a hit cancels the presentation immediately into the actual reaction. No suitable healing clip has been selected or verified yet.

## Gates that remain open

- Unity mapping/import is now verified on 6000.3.26f1: valid Humanoid Avatar, 52 mapped transforms, zero unweighted-vertex warnings and three keys on both meshes. HeroPoseCapture.cs delivers 12 clips / 60 quarter-duration poses, including primary Explosive L1/R1, in unity/captures.json. Raw retarget only; support/contact/axial-twist acceptance is still pending.
- Axial twist, foot orientation, actual support anchors, planted-foot drift, garment intersections and canonical contact envelope. Smoother quaternion blending does not certify any of these.
- Retimed preparation/active/recovery markers, the second strike growing out of the first follow-through, resolved visible contact, guard entry, throw pairing, cast release, recovery interruption and identical logical outcomes at 30/60/120 fps.
- Raw endpoint deltas in results.json are measurements of mismatches, not acceptance residuals. Do not interpret them as passed animation gates.

## First M0 creative delivery

Hero candidate: skin_v2 fixed-rig derived HumanM body, 1880 Blender control vertices and cosmetic build keys. Source FBX hips cluster (346 original vertices, including mixed weights) is restored before decimation; previous export lost that group and is rejected. Original bone endpoints remain unchanged; restoring the original hips node yields 52 Blender bones. Unity hard-normal/material splits produce 10599 body vertices and 1489 vest vertices, so 1880 is not the runtime draw-vertex count. Derived HeroCandidateSkinV2.fbx remains local/git-ignored; source assets are unchanged.

Palette generator: Assets/Game/Art/Editor/PaletteBuilder.cs, menu Guildbound/Art/Prepare M0 Palette. Seven matte URP materials using the atlas palette: dark/mid/frame timber, quiet stone, thatch, guild red and bone cloth. Code is uncommitted. Main LTS compile/invocation and idempotent repeat exit 0; seven materials generated under Art/Palette. Scene assignment and Claude's independent review remain pending.

Fire repair shortlist sent to Claude: Magic Arsenal - Fire Charge, Fire Missile Tiny, Fire Muzzle Normal, Fire Explosion Tiny. Use derived copies with a hand-sized charge, short trail and compact contact accent; size and timing changes must retain readable limbs.

API reference for the palette generator: https://docs.unity3d.com/6000.3/Documentation/ScriptReference/AssetDatabase.CreateAsset.html

## Capture correction

Unity renders initially reused a skinned GPU buffer across samples in one Editor frame. Each sampled pose is now CPU-baked into temporary capture meshes before rendering. Guard, forward plain cast and overhead charge captures reviewed after that fix. Source meshes/materials and saved scenes are untouched. Quarter-duration Unity samples are distinct from the authored-key Blender evidence.
