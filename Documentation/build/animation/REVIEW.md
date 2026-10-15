# Armature binding and motion layer - submitted to Aura

Status: code delivered uncommitted for Aura review. This is the assigned presentation slice, not full M0.5 acceptance.

The owned Brawler Armature now replaces the capsule in Sparring. The existing HeroAnimator drives 17 mapped states. Both source punches are left-handed, so Light2 is mirrored; Heavy1 uses a punch because the rules sweep a fist. Source contact and retraction candidates come from 1,098 sampled Humanoid poses. The six targeted importer overrides preserve the fall/rise in the bones while discarding horizontal root travel.

After the clip pose, FighterProceduralPose adds capped acceleration/velocity lean, delayed hips-to-spine-to-chest turning, breathing and critically damped contact recoil. It restores authored rotations before the next graph evaluation. SparringCamera follows its own spring position, offsets to the shoulder in combat, uses higher exploration framing and applies one directional pulse per resolved contact. Both feedback listeners reject duplicate events.

Verified on Unity 6000.3.26f1:

- Compile: no errors.
- Preview pose check: 30/60/120 fps, all 72 transform positions and scales preserved, object root unchanged. Maximum measured lean 13.26154 degrees, recoil 2.943036 degrees, turn lag 15 degrees. Recoil settled below 0.00004 degrees at one second. Identical spring endpoints within 0.00001 degrees.
- Controlled actual World-to-rig check: 13 state samples passed, including buffered mirrored Light2, heavy punch, guard, both steps, grab/throw, knockdown/get-up and **front and rear hit reactions**. The reversed Contact.Normal sign is now verified corrected. Root motion remained disabled and views matched the motor after every sample.
- Camera: player torso viewport (0.48, 0.46), partner (0.52, 0.53), both in front and inside frame. [Actual Heavy input frame](heavy-live.jpg) shows both owned rigs and the live HUD.
- Real input: after resetting keyboard/mouse transient state and explicitly selecting Defensive, the pattern, F -> Heavy and Heavy1 assertions passed before the capture. Play then exited during the next wait; the rest of that sequence is **aborted, not a pass**. A preceding input sequence also failed and is retained. Full real-input acceptance and stop/start cleanup remain open.

Reproduce through the existing Aura MCP execute_script tool:

1. Outside Play, execute `Tools/CheckFighterProceduralPose.cs`. Result saved in [procedural-check.txt](procedural-check.txt).
2. In Sparring Play, execute `Tools/CheckFighterAnimationRuntime.cs`. It pauses the driver for synchronous World ticks and restores/reset it in finally. Result saved in [runtime-check.txt](runtime-check.txt).
3. In Play, execute `Tools/PrepareAnimationInputCheck.cs`, then run the sequence in `Logs/aura-animation-input-clean.json`. Inspect every step and the aborted flag. This setup resets transient input devices and the bout, and sets the partner to Defensive; it saves nothing.

Sources: `Assets/Game/Anim/`, the narrow `Assets/Game/Play/FighterView.cs` hook, `Assets/Game/Art/Animation/HeroAnimator.controller`, `Assets/Game/Play/SparringCamera.cs`, and the audit/build/check tools in `Tools/`. Clip mapping is in [ACTION_CLIPS.md](ACTION_CLIPS.md); full poses are in `armature-pose-audit.json`.

Open: casting/recovery still use the guard fallback; paired throw victim/contact fitting and VFX have not passed. Foot IK is parked by Aura. No fighter-root interpolation, bone stretch, RigBuilder graph, global time-scale change or speculative stamina resource was added. Claude owns input ramp, momentum/facing, recovery cancels and startup retargeting.
