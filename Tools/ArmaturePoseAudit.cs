using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Animations;
using UnityEngine.Playables;
using UnityEngine.SceneManagement;
using Object = UnityEngine.Object;

// Read-only pose audit in an isolated preview scene. Does not save/switch the active scene or alter pack imports.
public static class ArmaturePoseAudit
{
    const string Rig = "Assets/Brawler Animations Sample/Models/Armature/Armature.fbx";
    const string Brawler = "Assets/Brawler Animations Sample/Animations/";
    const string Fighting = "Assets/FightingMotionsVolume1/FBX/";
    static readonly (string Role, string Path)[] Sources = {
        ("Idle", Brawler + "Fight_Idle.fbx"),
        ("WalkF", Brawler + "Walk_F.fbx"), ("WalkB", Brawler + "Walk_B.fbx"),
        ("Punch1", Brawler + "Atk_P_1.fbx"), ("Punch2", Brawler + "Atk_P_2.fbx"),
        ("Heavy", Brawler + "Atk_K_1.fbx"),
        ("Guard", Brawler + "Guard_Idle.fbx"), ("GuardHit", Brawler + "Guard_Hit.fbx"),
        ("HitF", Brawler + "Hit_F.fbx"), ("HitB", Brawler + "Hit_B.fbx"),
        ("StepF", Brawler + "Quickstep_F.fbx"), ("StepB", Brawler + "Quickstep_B.fbx"),
        ("GrabStart", Brawler + "Grab_Start.fbx"), ("Throw", Brawler + "Grab_Throw_A.fbx"),
        ("ThrowVictim", Brawler + "Grab_Throw_B.fbx"), ("GrabBreak", Brawler + "Grab_Break_A.fbx"),
        ("Knockdown", Fighting + "knockdown_A.fbx"), ("GetUp", Fighting + "getup_A.fbx")
    };

    [Serializable] class Sample
    {
        public float time;
        public Vector3 leftHand, rightHand, leftFoot, rightFoot, hips;
    }
    [Serializable] class ClipReport
    {
        public string role, path, clip;
        public float duration, rootDrift, leftHandPeak, rightHandPeak, leftFootPeak, rightFootPeak;
        public bool human, loop;
        public string[] rootCurves;
        public Sample[] samples;
    }
    [Serializable] class Report
    {
        public string unity, rig, limits;
        public bool avatarValid, avatarHuman;
        public ClipReport[] clips;
    }

    public static string Execute()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Run the preview audit outside Play Mode.");
        var preview = EditorSceneManager.NewPreviewScene();
        var reports = new List<ClipReport>();
        try
        {
            var model = Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>(Rig));
            SceneManager.MoveGameObjectToScene(model, preview);
            model.transform.SetPositionAndRotation(Vector3.zero, Quaternion.identity);
            var animator = model.GetComponent<Animator>();
            if (!animator.avatar.isValid || !animator.avatar.isHuman) throw new InvalidOperationException("Invalid Armature Avatar.");
            animator.applyRootMotion = false;
            animator.cullingMode = AnimatorCullingMode.AlwaysAnimate;
            foreach (var source in Sources)
            {
                var clip = AssetDatabase.LoadAllAssetsAtPath(source.Path).OfType<AnimationClip>()
                    .Single(c => !c.name.StartsWith("__preview"));
                if (!clip.isHumanMotion) throw new InvalidOperationException("Not Humanoid: " + source.Path);
                animator.Rebind();
                var graph = PlayableGraph.Create("Armature pose audit");
                try
                {
                    graph.SetTimeUpdateMode(DirectorUpdateMode.Manual);
                    var playable = AnimationClipPlayable.Create(graph, clip);
                    playable.SetApplyFootIK(false);
                    playable.SetApplyPlayableIK(false);
                    var output = AnimationPlayableOutput.Create(graph, "Pose", animator);
                    output.SetSourcePlayable(playable);
                    graph.Play();
                    var samples = new List<Sample>();
                    float drift = 0;
                    for (int i = 0; i <= 60; i++)
                    {
                        float time = clip.length * i / 60f;
                        playable.SetTime(time);
                        graph.Evaluate(0);
                        drift = Mathf.Max(drift, model.transform.position.magnitude);
                        Vector3 Bone(HumanBodyBones b) => model.transform.InverseTransformPoint(animator.GetBoneTransform(b).position);
                        samples.Add(new Sample { time = time, leftHand = Bone(HumanBodyBones.LeftHand),
                            rightHand = Bone(HumanBodyBones.RightHand), leftFoot = Bone(HumanBodyBones.LeftFoot),
                            rightFoot = Bone(HumanBodyBones.RightFoot), hips = Bone(HumanBodyBones.Hips) });
                    }
                    float Peak(Func<Sample, float> value) => samples.OrderByDescending(value).First().time;
                    reports.Add(new ClipReport {
                        role = source.Role, path = source.Path, clip = clip.name, duration = clip.length,
                        human = clip.isHumanMotion, loop = clip.isLooping, rootDrift = drift,
                        leftHandPeak = Peak(s => s.leftHand.z), rightHandPeak = Peak(s => s.rightHand.z),
                        leftFootPeak = Peak(s => s.leftFoot.z), rightFootPeak = Peak(s => s.rightFoot.z),
                        rootCurves = AnimationUtility.GetCurveBindings(clip)
                            .Where(b => b.propertyName.StartsWith("Root") || b.propertyName.StartsWith("Motion"))
                            .Select(b => b.propertyName).ToArray(), samples = samples.ToArray()
                    });
                }
                finally { graph.Destroy(); }
            }
            string directory = "Documentation/build/animation";
            Directory.CreateDirectory(directory);
            File.WriteAllText(directory + "/armature-pose-audit.json", JsonUtility.ToJson(new Report {
                unity = Application.unityVersion, rig = Rig, avatarValid = animator.avatar.isValid,
                avatarHuman = animator.avatar.isHuman, clips = reports.ToArray(),
                limits = "61 pose samples per clip on Armature with root motion off; reach peaks are candidate timing markers, not contact proof. No support fitting, throw pairing or runtime acceptance."
            }, true));
            return "Armature audit saved: " + reports.Count + " Humanoid clips, " + reports.Sum(c => c.samples.Length) + " actual poses. Active scene preserved.";
        }
        finally { EditorSceneManager.ClosePreviewScene(preview); }
    }
}
