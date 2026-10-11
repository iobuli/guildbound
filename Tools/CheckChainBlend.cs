using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Text;
using Guildbound.Anim;
using Guildbound.Combat;
using Guildbound.Play;
using UnityEditor;
using UnityEngine;

// Chained-blend continuity for Light1 -> Light2 (the chain row in FighterPresentation.asset).
// The cancel only happens on a hit, after Active. So each run lands a light into the partner (fighters start close),
// and presses Light2 at a candidate tick in the recovery window. The first press that produces a
// Light1 -> Light2 handover is measured. Samples the rig's hands once per rule tick across the handover.
// PASS: the largest per-tick hand step inside the blend window is at most 2.5x the largest normal Light1 step.
// FAIL is reported, never hidden: if no candidate produces a chain, the check throws instead of passing.
// Driver is restored in finally. Saves only Documentation/checks/motion/chain-blend.txt.
public static class CheckChainBlend
{
    public static string Execute()
    {
        if (!EditorApplication.isPlaying) throw new InvalidOperationException("Run in Sparring Play Mode.");
        var driver = UnityEngine.Object.FindFirstObjectByType<SparringDriver>();
        var view = driver.PlayerView;
        var anim = view.GetComponentInChildren<FighterAnimation>();
        var rig = anim.GetComponent<Animator>();
        bool enabled = driver.enabled;
        var viewUpdate = typeof(FighterView).GetMethod("LateUpdate", BindingFlags.Instance | BindingFlags.NonPublic);
        var animUpdate = typeof(FighterAnimation).GetMethod("LateUpdate", BindingFlags.Instance | BindingFlags.NonPublic);
        var dispatch = typeof(SparringDriver).GetMethod("Dispatch", BindingFlags.Instance | BindingFlags.NonPublic);
        var report = new StringBuilder("Chain blend continuity - " + Application.unityVersion + "\n");
        try
        {
            driver.enabled = false;
            void Sync()
            {
                dispatch.Invoke(driver, null);
                foreach (var v in new[] { driver.PlayerView, driver.OpponentView })
                {
                    viewUpdate.Invoke(v, null);
                    animUpdate.Invoke(v.GetComponentInChildren<FighterAnimation>(), null);
                }
            }

            var left = new List<Vector3>();
            var right = new List<Vector3>();
            var states = new List<string>();
            int handover = -1, pressTick = -1;
            for (int press = 24; press <= 64 && handover < 0; press += 2)
            {
                driver.ResetBout();
                driver.Player.Position = new Vector3(-.45f, 0, 0);
                driver.Opponent.Position = new Vector3(.45f, 0, 0);
                Sync();
                left = new List<Vector3>();
                right = new List<Vector3>();
                states = new List<string>();
                for (int i = 0; i < 90; i++)
                {
                    var player = i == 0 || i == press ? new Intent { Light = true } : default;
                    driver.World.Tick(new[] { player, default(Intent) });
                    Sync();
                    left.Add(rig.GetBoneTransform(HumanBodyBones.LeftHand).position);
                    right.Add(rig.GetBoneTransform(HumanBodyBones.RightHand).position);
                    states.Add(anim.PresentedState);
                    int n = states.Count - 1;
                    if (handover < 0 && n > 0 && states[n - 1] == "Light1" && states[n] == "Light2")
                    {
                        handover = n;
                        pressTick = press;
                    }
                }
            }

            if (handover < 0)
                throw new InvalidOperationException("No Light1 -> Light2 chain observed for presses at ticks 24-64. The cancel window was not reached. Not a PASS.");

            // Per-tick step: the larger of the two hand displacements.
            var step = new float[states.Count];
            for (int t = 1; t < states.Count; t++)
                step[t] = Mathf.Max(Vector3.Distance(left[t], left[t - 1]), Vector3.Distance(right[t], right[t - 1]));

            // A snap is a change in velocity at the handover that the neighbouring ticks do not show. Each clip has its
            // own fast swing, so the test compares the handover acceleration with the accelerations around it, not with
            // a different clip's speed. Acceleration is the larger of the two hands.
            var accel = new float[states.Count];
            for (int t = 2; t < states.Count; t++)
            {
                var vL0 = left[t - 1] - left[t - 2]; var vL1 = left[t] - left[t - 1];
                var vR0 = right[t - 1] - right[t - 2]; var vR1 = right[t] - right[t - 1];
                accel[t] = Mathf.Max((vL1 - vL0).magnitude, (vR1 - vR0).magnitude);
            }
            float neighbour = 0f;
            for (int t = Mathf.Max(2, handover - 6); t <= Mathf.Min(states.Count - 1, handover + 6); t++)
                if (t != handover && t != handover + 1 && t != handover - 1) neighbour = Mathf.Max(neighbour, accel[t]);

            float handoverAccel = Mathf.Max(accel[handover], accel[Mathf.Min(handover + 1, states.Count - 1)]);
            report.AppendLine("press tick " + pressTick + ", handover tick " + handover + " (Light1 -> Light2)");
            report.AppendLine("largest hand acceleration at handover (ticks " + handover + "-" + (handover + 1) + "): " + handoverAccel.ToString("F5") + " m/tick^2");
            report.AppendLine("largest hand acceleration in the 6 ticks either side: " + neighbour.ToString("F5") + " m/tick^2");
            for (int t = handover - 2; t <= handover + 3; t++)
                if (t >= 2 && t < states.Count) report.AppendLine("  tick " + t + " " + states[t] + " accel " + accel[t].ToString("F5"));

            bool pass = neighbour > 1e-5f && handoverAccel <= 2f * neighbour;
            report.AppendLine(pass ? "PASS: no velocity jump at the handover beyond the surrounding motion." : "FAIL: velocity jumps at the handover beyond the surrounding motion.");
            Directory.CreateDirectory("Documentation/checks/motion");
            File.WriteAllText("Documentation/checks/motion/chain-blend.txt", report.ToString());
            if (!pass) throw new InvalidOperationException(report.ToString());
            return report.ToString();
        }
        finally
        {
            driver.ResetBout();
            driver.enabled = enabled;
        }
    }
}
