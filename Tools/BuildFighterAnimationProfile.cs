using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using Guildbound.Anim;
using Guildbound.Contracts;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;

// Rerunnable local builder. Reuses the owned FBX subassets and Aura's existing controller.
public static class BuildFighterAnimationProfile
{
    const string ControllerPath = "Assets/Game/Art/Animation/HeroAnimator.controller";
    const string ProfilePath = "Assets/Game/Anim/Resources/FighterPresentation.asset";
    const string RigPath = "Assets/Brawler Animations Sample/Models/Armature/Armature.fbx";
    const string AuditDirectory = "Documentation/build/animation";

    public static string Execute()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Build the profile outside Play Mode.");
        var rows = File.ReadAllLines(AuditDirectory + "/pose-markers.csv").Skip(1)
            .Where(s => !string.IsNullOrWhiteSpace(s)).Select(s => s.Split(',')).ToDictionary(s => s[0]);
        var controller = AssetDatabase.LoadAssetAtPath<AnimatorController>(ControllerPath);
        var rig = AssetDatabase.LoadAssetAtPath<GameObject>(RigPath);
        if (!controller || !rig) throw new InvalidOperationException("Missing existing controller or owned Armature.");
        var avatar = rig.GetComponent<Animator>().avatar;
        if (!avatar.isValid || !avatar.isHuman) throw new InvalidOperationException("Armature must be valid Humanoid.");
        if (!controller.parameters.Any(p => p.name == "RulesIdle"))
            controller.AddParameter("RulesIdle", AnimatorControllerParameterType.Bool);
        var machine = controller.layers[0].stateMachine;
        var states = machine.states.Select(s => s.state).ToDictionary(s => s.name);
        var table = new[] {
            ("Idle", "Idle"), ("WalkF", "WalkF"), ("WalkB", "WalkB"),
            ("Light1", "Punch1"), ("Light2", "Punch2"), ("Heavy1", "Punch2"),
            ("Guard", "Guard"), ("GuardHit", "GuardHit"), ("HitF", "HitF"), ("HitB", "HitB"),
            ("StepF", "StepF"), ("StepB", "StepB"), ("GrabStart", "GrabStart"),
            ("GrabThrow", "Throw"), ("GrabBreak", "GrabBreak"), ("Knockdown", "Knockdown"), ("Getup", "GetUp")
        };
        var clips = new List<FighterClip>();
        var doc = new StringBuilder("# Armature action binding\n\nOwned source rig and clips. Existing HeroAnimator extended, root motion disabled, combat windows unchanged.\n\n");
        doc.AppendLine("| State | Source | Mirror | Contact (source s) | Follow-through (source s) |");
        doc.AppendLine("|---|---|---|---:|---:|");
        foreach (var entry in table)
        {
            var row = rows[entry.Item2];
            var clip = AssetDatabase.LoadAllAssetsAtPath(row[1]).OfType<AnimationClip>()
                .Single(c => !c.name.StartsWith("__preview"));
            if (!clip.isHumanMotion || clip.length <= 0) throw new InvalidOperationException("Invalid clip: " + row[1]);
            var state = states[entry.Item1];
            state.motion = clip;
            state.speed = 1;
            state.mirror = entry.Item1 == "Light2";
            bool strike = entry.Item1 == "Light1" || entry.Item1 == "Light2" || entry.Item1 == "Heavy1";
            float contact = strike ? float.Parse(row[4], CultureInfo.InvariantCulture) : 0;
            float follow = strike ? float.Parse(row[5], CultureInfo.InvariantCulture)
                : entry.Item1.StartsWith("Step") ? clip.length : 0;
            if (follow < contact || follow > clip.length) throw new InvalidOperationException("Invalid pose markers: " + state.name);
            var binding = new FighterClip { state = state.name, clip = clip, contact = contact, followThrough = follow };
            clips.Add(binding);
            if (strike) CheckPhases(binding, entry.Item1 == "Heavy1" ? .55f : .25f, .10f, entry.Item1 == "Heavy1" ? .50f : .30f);
            if (entry.Item1.StartsWith("Step")) CheckPhases(binding, 0, .35f, 0);
            if (entry.Item1 == "HitF" || entry.Item1 == "HitB" || entry.Item1 == "Getup")
                CheckPhases(binding, 0, 0, entry.Item1 == "Getup" ? .6f : .4f);
            bool loop = entry.Item1 == "Idle" || entry.Item1.StartsWith("Walk") || entry.Item1 == "Guard";
            if (!loop && !state.transitions.Any(t => t.destinationState == states["Idle"] && t.conditions.Any(c => c.parameter == "RulesIdle")))
            {
                var transition = state.AddTransition(states["Idle"]);
                transition.hasExitTime = true;
                transition.exitTime = 1;
                transition.hasFixedDuration = true;
                transition.duration = .07f;
                transition.AddCondition(AnimatorConditionMode.If, 0, "RulesIdle");
            }
            EditorUtility.SetDirty(state);
            doc.AppendLine("| " + state.name + " | " + row[1] + " | " + state.mirror + " | " + contact.ToString("F4", CultureInfo.InvariantCulture)
                + " | " + follow.ToString("F4", CultureInfo.InvariantCulture) + " |");
        }
        if (!AssetDatabase.IsValidFolder("Assets/Game/Anim/Resources")) AssetDatabase.CreateFolder("Assets/Game/Anim", "Resources");
        var profile = AssetDatabase.LoadAssetAtPath<FighterAnimationProfile>(ProfilePath);
        if (!profile)
        {
            profile = ScriptableObject.CreateInstance<FighterAnimationProfile>();
            AssetDatabase.CreateAsset(profile, ProfilePath);
        }
        profile.rig = rig;
        profile.controller = controller;
        profile.clips = clips.ToArray();
        profile.blendSeconds = .07f;
        profile.walkReferenceSpeed = 2.1f;
        EditorUtility.SetDirty(profile);
        EditorUtility.SetDirty(controller);
        AssetDatabase.SaveAssets();
        doc.AppendLine("\nTiming markers: measured maximum forward reach of the striking hand, then first sample with 10% retraction. These are presentation candidates, not verified collision points. 61 samples per clip. Light2 mirrors the left-handed Punch2. Heavy1 replaces the kick with that punch because the rules sweep a fist. Heavy2 remains an unused authoring state.\n");
        doc.AppendLine("Transitions blend over 0.07 s. Startup/active/recovery sample the same rule clock; no animation event can authorize damage. Reaction clips traverse their complete source duration in the rule recovery window. Step traverses its complete clip in its active-only window.\n");
        doc.AppendLine("Limitations: no foot fitting, hand-to-contact fitting, paired throw victim, cast/recover clips or hit VFX in this first binding. Cast/recover temporarily use guard stance. These remain presentation work; this binding does not pass those gates.\n");
        doc.AppendLine("Importer: CombatClipImports uses lockRootPositionXZ=false on four Brawler imports plus knockdown_A/getup_A; the two FightingMotions clips use lockRootHeightY=true and lockRootRotation=true to retain the fall and rise in the bones. Source model metas are ignored, so the postprocessor reapplies these on pack unpack/reimport.\n");
        File.WriteAllText(AuditDirectory + "/ACTION_CLIPS.md", doc.ToString());
        return "Profile saved: " + clips.Count + " states, 8 phase endpoint checks passed, HeroAnimator reused; Light2 mirrored and Heavy1 is a punch.";
    }

    static void CheckPhases(FighterClip clip, float startup, float active, float recovery)
    {
        var state = new ActionState { Timing = new ActionTiming { Startup = startup, Active = active, Recovery = recovery } };
        void Check(float elapsed, float expected)
        {
            state.Elapsed = elapsed;
            if (Mathf.Abs(clip.Sample(state) - expected) > .0001f)
                throw new InvalidOperationException("Phase endpoint mismatch: " + clip.state + " at " + elapsed);
        }
        Check(0, 0);
        if (startup > 0) Check(startup, clip.contact);
        if (active > 0) Check(startup + active, clip.followThrough);
        Check(startup + active + recovery, clip.clip.length);
    }
}
