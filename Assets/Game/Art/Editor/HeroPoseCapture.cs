using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Animations;
using UnityEngine.Playables;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using UnityEngine.SceneManagement;
using Object = UnityEngine.Object;

namespace Guildbound.Art.Editor
{
    // Actual Mecanim retarget capture; no motor, contact, support IK or damage.
    public static class HeroPoseCapture
    {
        const string Hero = "Assets/Game/Art/Derived/HeroCandidateSkinV2.fbx";
        const string Brawler = "Assets/Brawler Animations Sample/Animations/";
        const string Explosive = "Assets/ExplosiveLLC/RPG Character Mecanim Animation Pack FREE/Animations/Unarmed/";
        const string Spell = "Assets/Kevin Iglesias/Human Animations/Animations/Male/Combat/Spellcasting/";
        const string Directional = Spell + "MagicAttacks/Directional/";
        static readonly (string Role, string Path)[] Clips = {
            ("stance", Brawler + "Fight_Idle.fbx"),
            ("light1", Explosive + "RPG-Character@Unarmed-Attack-L1.FBX"),
            ("light2", Explosive + "RPG-Character@Unarmed-Attack-R1.FBX"),
            ("heavy", Brawler + "Atk_K_1.fbx"), ("guard", Brawler + "Guard_Idle.fbx"),
            ("step", Brawler + "Quickstep_B.fbx"), ("throw", Brawler + "Grab_Throw_A.fbx"),
            ("hit", "Assets/FightingMotionsVolume1/FBX/hd_front_A.fbx"),
            ("plain", Directional + "HumanM@MagicAttackDirect1H01_R.fbx"),
            ("charge", Directional + "HumanM@MagicAttackDirect2H01 - Load.fbx"),
            ("release", Directional + "HumanM@MagicAttackDirect2H01 - Cast.fbx"),
            ("interrupt", Spell + "HumanM@CastingDamage01.fbx")
        };

        [MenuItem("Guildbound/Art/Capture Hero Clip Poses")]
        public static void Run()
        {
            string directory = Path.Combine(Path.GetDirectoryName(Application.dataPath), "Documentation/experiments/codex_motion_lab/unity");
            Directory.CreateDirectory(directory);
            var preview = EditorSceneManager.NewPreviewScene();
            var model = (GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Hero));
            SceneManager.MoveGameObjectToScene(model, preview);
            var animator = model.GetComponent<Animator>();
            animator.applyRootMotion = false;
            animator.cullingMode = AnimatorCullingMode.AlwaysAnimate;
            var materials = new List<Material>();
            var bakedMeshes = new List<(SkinnedMeshRenderer Source, Mesh Mesh)>();
            foreach (var renderer in model.GetComponentsInChildren<SkinnedMeshRenderer>())
            {
                renderer.updateWhenOffscreen = true;
                renderer.sharedMaterials = renderer.sharedMaterials.Select(m => {
                    var copy = new Material(Shader.Find("Universal Render Pipeline/Lit")) { color = m.color };
                    copy.SetFloat("_Metallic", 0); copy.SetFloat("_Smoothness", .05f);
                    materials.Add(copy); return copy;
                }).ToArray();
                // Multiple renders inside one editor frame can reuse GPU skin
                // buffers. Bake the actual sampled pose on CPU for captures.
                var baked = new Mesh { name = "Sampled pose " + renderer.name };
                baked.MarkDynamic();
                var view = new GameObject("CPU pose view");
                view.transform.SetParent(renderer.transform, false);
                view.AddComponent<MeshFilter>().sharedMesh = baked;
                view.AddComponent<MeshRenderer>().sharedMaterials = renderer.sharedMaterials;
                bakedMeshes.Add((renderer, baked));
                renderer.enabled = false;
            }
            var camera = new GameObject("Pose review camera").AddComponent<Camera>();
            SceneManager.MoveGameObjectToScene(camera.gameObject, preview);
            camera.scene = preview;
            camera.orthographic = true; camera.orthographicSize = 1.35f;
            camera.clearFlags = CameraClearFlags.SolidColor; camera.backgroundColor = new Color(.16f, .19f, .23f);
            camera.transform.position = new Vector3(2.6f, 1.8f, 5);
            camera.transform.LookAt(new Vector3(0, 1.1f, 0));
            camera.GetUniversalAdditionalCameraData().renderPostProcessing = false;
            var light = new GameObject("Pose review light").AddComponent<Light>();
            SceneManager.MoveGameObjectToScene(light.gameObject, preview);
            light.type = LightType.Directional; light.intensity = 1.6f;
            light.transform.rotation = Quaternion.Euler(35, -35, 0);
            var target = new RenderTexture(480, 640, 24, RenderTextureFormat.ARGB32);
            var texture = new Texture2D(480, 640, TextureFormat.RGBA32, false);
            var captures = new List<Capture>();
            try
            {
                foreach (var entry in Clips)
                {
                    var clip = AssetDatabase.LoadAllAssetsAtPath(entry.Path).OfType<AnimationClip>()
                        .Single(c => !c.name.StartsWith("__preview"));
                    if (!clip.isHumanMotion) throw new InvalidOperationException("Not a Humanoid clip: " + entry.Path);
                    animator.Rebind();
                    var graph = PlayableGraph.Create("Codex owned pose review");
                    try
                    {
                        graph.SetTimeUpdateMode(DirectorUpdateMode.Manual);
                        var playable = AnimationClipPlayable.Create(graph, clip);
                        playable.SetApplyFootIK(false); playable.SetApplyPlayableIK(false);
                        var output = AnimationPlayableOutput.Create(graph, "Hero", animator);
                        output.SetSourcePlayable(playable); graph.Play();
                        for (int index = 0; index < 5; index++)
                        {
                            float time = clip.length * index / 4f;
                            playable.SetTime(time); graph.Evaluate(0);
                            foreach (var mesh in bakedMeshes) mesh.Source.BakeMesh(mesh.Mesh);
                            string file = entry.Role + "_" + index + ".png";
                            RenderPipeline.SubmitRenderRequest(camera, new RenderPipeline.StandardRequest { destination = target });
                            var previous = RenderTexture.active;
                            try { RenderTexture.active = target; texture.ReadPixels(new Rect(0, 0, 480, 640), 0, 0); texture.Apply(); }
                            finally { RenderTexture.active = previous; }
                            File.WriteAllBytes(Path.Combine(directory, file), texture.EncodeToPNG());
                            var bones = new[] { HumanBodyBones.Hips, HumanBodyBones.Head, HumanBodyBones.LeftHand,
                                HumanBodyBones.RightHand, HumanBodyBones.LeftFoot, HumanBodyBones.RightFoot };
                            captures.Add(new Capture { role = entry.Role, asset = entry.Path, clip = clip.name,
                                time = time, clipLength = clip.length, file = file,
                                joints = bones.Select(b => new Joint { name = b.ToString(), position = animator.GetBoneTransform(b).position }).ToArray() });
                        }
                    }
                    finally { graph.Destroy(); }
                }
                File.WriteAllText(Path.Combine(directory, "captures.json"), JsonUtility.ToJson(new Report {
                    unityVersion = Application.unityVersion, hero = Hero, captures = captures.ToArray(),
                    limits = "Raw Unity Humanoid retarget at quarter intervals. No foot IK, contact fitting, combat or healing acceptance."
                }, true));
            }
            finally
            {
                Object.DestroyImmediate(target); Object.DestroyImmediate(texture);
                foreach (var material in materials) Object.DestroyImmediate(material);
                foreach (var mesh in bakedMeshes) Object.DestroyImmediate(mesh.Mesh);
                EditorSceneManager.ClosePreviewScene(preview);
            }
            Debug.Log("Codex Hero pose capture complete: 12 Humanoid clips / 60 poses, corrections disabled.");
        }
        [Serializable] sealed class Joint { public string name; public Vector3 position; }
        [Serializable] sealed class Capture { public string role, asset, clip, file; public float time, clipLength; public Joint[] joints; }
        [Serializable] sealed class Report { public string unityVersion, hero, limits; public Capture[] captures; }
    }
}
