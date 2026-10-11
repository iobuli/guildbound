using System;
using System.IO;
using System.Reflection;
using System.Text;
using Guildbound.Anim;
using Guildbound.Combat;
using Guildbound.Contracts;
using Guildbound.Play;
using UnityEditor;
using UnityEngine;

// Actual CombatWorld -> FighterView -> owned Humanoid graph integration, with live input isolated.
// The driver is paused only for this synchronous call and restored/reset in finally.
public static class CheckFighterAnimationRuntime
{
    public static string Execute()
    {
        if (!EditorApplication.isPlaying) throw new InvalidOperationException("Run in Sparring Play Mode.");
        var driver = UnityEngine.Object.FindFirstObjectByType<SparringDriver>();
        var profile = driver.PlayerView.animationProfile;
        bool enabled = driver.enabled;
        float blend = profile.blendSeconds;
        var viewUpdate = typeof(FighterView).GetMethod("LateUpdate", BindingFlags.Instance | BindingFlags.NonPublic);
        var animUpdate = typeof(FighterAnimation).GetMethod("LateUpdate", BindingFlags.Instance | BindingFlags.NonPublic);
        var dispatch = typeof(SparringDriver).GetMethod("Dispatch", BindingFlags.Instance | BindingFlags.NonPublic);
        var cameraRig = driver.viewCamera.GetComponent<SparringCamera>();
        var cameraUpdate = typeof(SparringCamera).GetMethod("LateUpdate", BindingFlags.Instance | BindingFlags.NonPublic);
        var report = new StringBuilder("Controlled animation integration - " + Application.unityVersion + "\n");
        int checks = 0;
        try
        {
            driver.enabled = false;
            profile.blendSeconds = 0; // inspect exact endpoint poses; visual blends are tested separately
            void Sync()
            {
                dispatch.Invoke(driver, null);
                foreach (var view in new[] { driver.PlayerView, driver.OpponentView })
                {
                    viewUpdate.Invoke(view, null);
                    var anim = view.GetComponentInChildren<FighterAnimation>();
                    animUpdate.Invoke(anim, null);
                    if ((view.transform.position - view.Sim.Position).magnitude > .0001f
                        || anim.transform.localPosition.sqrMagnitude > .0000001f || !anim.RootMotionDisabled)
                        throw new InvalidOperationException("Motor authority failed: " + view.name);
                }
            }
            void Reset(bool close = false)
            {
                driver.ResetBout();
                if (close) { driver.Player.Position = new Vector3(-.45f, 0, 0); driver.Opponent.Position = new Vector3(.45f, 0, 0); }
                Sync();
            }
            void Tick(Intent player, Intent partner = default)
            {
                driver.World.Tick(new[] { player, partner });
                Sync();
            }
            void Advance(int count) { for (int i = 0; i < count; i++) Tick(default); }
            void Pose(FighterView view, string expected)
            {
                var anim = view.GetComponentInChildren<FighterAnimation>();
                if (anim.PresentedState != expected) throw new InvalidOperationException("Expected " + view.name + " " + expected + ", got " + anim.PresentedState + " (rule " + view.Current.Kind + ")");
                var avatar = anim.GetComponent<Animator>();
                var hand = anim.transform.InverseTransformPoint(avatar.GetBoneTransform(HumanBodyBones.LeftHand).position);
                report.AppendLine(view.name + ": " + view.Current.Kind + " -> " + expected + " @ " + anim.SourceSeconds + " s; Lhand=" + hand);
                checks++;
            }

            Reset(); Pose(driver.PlayerView, "Idle");
            cameraUpdate.Invoke(cameraRig, null);
            var playerScreen = driver.viewCamera.WorldToViewportPoint(driver.Player.Position + Vector3.up * 1.1f);
            var partnerScreen = driver.viewCamera.WorldToViewportPoint(driver.Opponent.Position + Vector3.up * 1.1f);
            foreach (var point in new[] { playerScreen, partnerScreen })
                if (point.z <= 0 || point.x < .1f || point.x > .9f || point.y < .1f || point.y > .9f)
                    throw new InvalidOperationException("Fighter outside camera frame: " + point);
            if (Mathf.Abs(playerScreen.x - partnerScreen.x) < .025f)
                throw new InvalidOperationException("Camera still stacks the fighters: " + playerScreen + " / " + partnerScreen);
            report.AppendLine("Shoulder camera: Player viewport=" + playerScreen + ", Partner=" + partnerScreen);
            Tick(new Intent { Light = true }); Advance(29); Pose(driver.PlayerView, "Light1");
            var rig = driver.PlayerView.GetComponentInChildren<Animator>();
            float reach = driver.PlayerView.transform.InverseTransformPoint(rig.GetBoneTransform(HumanBodyBones.LeftHand).position).z;
            if (reach < .5f) throw new InvalidOperationException("Actual light pose did not extend its hand: " + reach);
            Advance(22); Tick(new Intent { Light = true }); Advance(55); Pose(driver.PlayerView, "Light2");
            float rightReach = driver.PlayerView.transform.InverseTransformPoint(rig.GetBoneTransform(HumanBodyBones.RightHand).position).z;
            if (rightReach < .4f) throw new InvalidOperationException("Mirrored Light2 did not extend its right hand: " + rightReach);
            Reset(); Tick(new Intent { Heavy = true }); Advance(65); Pose(driver.PlayerView, "Heavy1");
            Reset(); Tick(new Intent { GuardHeld = true }); Pose(driver.PlayerView, "Guard");
            Reset(); Tick(new Intent { Move = Vector2.right, Step = true }); Advance(12); Pose(driver.PlayerView, "StepF");
            Reset(); Tick(new Intent { Move = Vector2.left, Step = true }); Advance(12); Pose(driver.PlayerView, "StepB");
            Reset(true); Tick(new Intent { Throw = true }); Advance(10); Pose(driver.PlayerView, "GrabStart");
            Advance(31); Pose(driver.PlayerView, "GrabThrow"); Pose(driver.OpponentView, "Knockdown");
            Advance(125); Pose(driver.OpponentView, "Getup");
            Reset(true); Tick(new Intent { Light = true }); Advance(36); Pose(driver.OpponentView, "HitF");
            Reset(true); driver.Opponent.Facing = Vector3.right;
            Tick(new Intent { Light = true }); Advance(36); Pose(driver.OpponentView, "HitB");
            report.AppendLine("PASS: " + checks + " actual core-to-rig states, mirrored right-hand reach, root authority after every sample. Driver and profile restored in finally.");
            Directory.CreateDirectory("Documentation/build/animation");
            File.WriteAllText("Documentation/build/animation/runtime-check.txt", report.ToString());
            return report.ToString();
        }
        finally
        {
            profile.blendSeconds = blend;
            driver.ResetBout();
            driver.enabled = enabled;
        }
    }
}
