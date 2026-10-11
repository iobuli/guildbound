using System;
using System.IO;
using System.Linq;
using System.Text;
using Guildbound.Anim;
using Guildbound.Contracts;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Animations;
using UnityEngine.Playables;
using UnityEngine.SceneManagement;

// Real owned Humanoid pose + additive layer invariants in an isolated preview scene.
public static class CheckFighterProceduralPose
{
    sealed class Feed : IFighterPresentation
    {
        public int Id => 0;
        public ActionState Current { get; set; }
        public Vector3 Position { get; set; }
        public Vector3 Velocity { get; set; }
        public Vector3 Facing { get; set; } = Vector3.forward;
        public bool Grounded => true;
        public Transform LockTarget => null;
        public float FireCharge01 => 0;
        public event Action<ContactEvent> Contact;
        public event Action<ActionState> ActionStarted;
        public void Hit(ContactEvent hit) => Contact?.Invoke(hit);
    }

    public static string Execute()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Run preview checks outside Play Mode.");
        var preview = EditorSceneManager.NewPreviewScene();
        var graph = PlayableGraph.Create("Procedural pose check");
        var report = new StringBuilder("Procedural pose - " + Application.unityVersion + "\n");
        try
        {
            var profile = AssetDatabase.LoadAssetAtPath<FighterAnimationProfile>("Assets/Game/Anim/Resources/FighterPresentation.asset");
            var model = UnityEngine.Object.Instantiate(profile.rig);
            SceneManager.MoveGameObjectToScene(model, preview);
            model.transform.SetPositionAndRotation(Vector3.zero, Quaternion.identity);
            var animator = model.GetComponent<Animator>();
            animator.applyRootMotion = false;
            animator.cullingMode = AnimatorCullingMode.AlwaysAnimate;
            graph.SetTimeUpdateMode(DirectorUpdateMode.Manual);
            var clip = AnimationClipPlayable.Create(graph, profile.Find("Idle").clip);
            var output = AnimationPlayableOutput.Create(graph, "Owned pose", animator);
            output.SetSourcePlayable(clip);
            graph.Play();
            clip.SetTime(.35);
            graph.Evaluate(0);
            var transforms = model.GetComponentsInChildren<Transform>();
            var before = transforms.Select(t => t.localPosition).ToArray();
            var scales = transforms.Select(t => t.localScale).ToArray();
            float maxLean = 0, maxRecoil = 0, maxTurn = 0, maxBoneTranslation = 0;
            foreach (int fps in new[] { 30, 60, 120 })
            {
                var feed = new Feed();
                var pose = model.AddComponent<FighterProceduralPose>();
                pose.Bind(feed, animator);
                float dt = 1f / fps;
                for (int frame = 0; frame < fps; frame++)
                {
                    float time = frame * dt;
                    feed.Current = time < .25f ? new ActionState { Kind = ActionKind.LightL, Phase = ActionPhase.Startup,
                        Sequence = 1, Elapsed = time, Timing = new ActionTiming { Startup = .25f, Active = .1f, Recovery = .3f } }
                        : new ActionState { Sequence = 1 };
                    feed.Velocity = time < .25f ? Vector3.forward * 4 : Vector3.zero;
                    feed.Position += feed.Velocity * dt;
                    feed.Facing = time < .35f ? Vector3.forward : Vector3.right;
                    if (frame == fps / 4)
                    {
                        var hit = new ContactEvent { AttackerId = 1, DefenderId = 0, Action = ActionKind.Heavy,
                            ActionSequence = 1, Outcome = Outcome.Hit, Normal = Vector3.forward, Time = 1 };
                        feed.Hit(hit);
                        feed.Hit(hit); // same resolved event twice must not multiply feedback
                    }
                    pose.RestoreAuthoredPose();
                    pose.Apply(dt);
                    maxLean = Mathf.Max(maxLean, pose.LeanDegrees);
                    maxRecoil = Mathf.Max(maxRecoil, pose.RecoilDegrees);
                    maxTurn = Mathf.Max(maxTurn, Mathf.Abs(pose.TurnLagDegrees));
                    for (int i = 0; i < transforms.Length; i++)
                    {
                        maxBoneTranslation = Mathf.Max(maxBoneTranslation, Vector3.Distance(transforms[i].localPosition, before[i]));
                        if (transforms[i].localScale != scales[i]) throw new InvalidOperationException("Bone scale changed: " + transforms[i].name);
                    }
                    if (model.transform.position.sqrMagnitude > .0000001f || Quaternion.Angle(model.transform.rotation, Quaternion.identity) > .001f)
                        throw new InvalidOperationException("Procedural pose changed fighter root.");
                }
                if (pose.RecoilDegrees > .001f) throw new InvalidOperationException("Recoil did not settle: " + fps + " fps, " + pose.RecoilDegrees);
                report.AppendLine(fps + " fps: recoil at 1 s=" + pose.RecoilDegrees + " deg; root fixed; " + transforms.Length + " transform positions/scales preserved.");
                pose.RestoreAuthoredPose();
                UnityEngine.Object.DestroyImmediate(pose);
            }
            if (maxBoneTranslation > .000001f || maxLean > 15.001f || maxRecoil > 8.001f || maxTurn > 15.001f)
                throw new InvalidOperationException("Bound failed: lean=" + maxLean + " recoil=" + maxRecoil + " turn=" + maxTurn + " translation=" + maxBoneTranslation);
            Vector2 reference = Vector2.zero;
            foreach (int fps in new[] { 30, 60, 120 })
            {
                var p = Vector2.zero;
                var v = new Vector2(160, 0);
                for (int i = 0; i < fps / 2; i++) FighterProceduralPose.StepSpring(ref p, ref v, 20, 1f / fps);
                if (fps == 30) reference = p;
                if (Vector2.Distance(p, reference) > .00001f) throw new InvalidOperationException("Spring differs across frame rates.");
            }
            report.AppendLine("PASS: maximum lean=" + maxLean + " deg, recoil=" + maxRecoil + " deg, turn lag=" + maxTurn
                + " deg, local-position change=" + maxBoneTranslation + " m. Spring endpoint agrees at 30/60/120 fps within 0.00001 deg.");
            Directory.CreateDirectory("Documentation/build/animation");
            File.WriteAllText("Documentation/build/animation/procedural-check.txt", report.ToString());
            return report.ToString();
        }
        finally
        {
            graph.Destroy();
            EditorSceneManager.ClosePreviewScene(preview);
        }
    }
}
