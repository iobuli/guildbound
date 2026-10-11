using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using UnityEditor;
using UnityEngine;

namespace Guildbound.Art.Editor
{
    // M0 candidate only. A valid Avatar is not clip/contact/support acceptance.
    public static class HeroCandidateBuilder
    {
        const string Folder = "Assets/Game/Art/Derived";
        const string ModelPath = Folder + "/HeroCandidateSkinV2.fbx";

        [MenuItem("Guildbound/Art/Prepare M0 Art")]
        public static void PrepareM0Art()
        {
            Prepare();
            PaletteBuilder.Prepare();
        }

        [MenuItem("Guildbound/Art/Prepare Hero Candidate")]
        public static void Prepare()
        {
            string source = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "FreeGuild", "codex_character_lab", "skin_v2", "codex_body_lab.fbx");
            if (!File.Exists(source)) throw new FileNotFoundException("Run the owned-body Blender lab first.", source);
            string project = Path.GetDirectoryName(Application.dataPath);
            if (!File.Exists(Path.Combine(project, Folder, ".gitignore"))) throw new InvalidOperationException("Derived mesh ignore policy is missing.");
            string destination = Path.Combine(Application.dataPath, "Game/Art/Derived/HeroCandidateSkinV2.fbx");
            Directory.CreateDirectory(Path.GetDirectoryName(destination));
            if (File.Exists(destination))
            {
                if (Hash(source) != Hash(destination))
                    throw new InvalidOperationException("Preserve differing candidate: " + destination);
            }
            else File.Copy(source, destination);
            AssetDatabase.ImportAsset(ModelPath, ImportAssetOptions.ForceSynchronousImport);
            var model = AssetDatabase.LoadAssetAtPath<GameObject>(ModelPath);
            var transforms = model.GetComponentsInChildren<Transform>(true);
            var mapping = new List<(HumanBodyBones Human, string Bone)>
            {
                (HumanBodyBones.Hips, "B-hips"), (HumanBodyBones.Spine, "B-spine"),
                (HumanBodyBones.Chest, "B-chest"), (HumanBodyBones.Neck, "B-neck"),
                (HumanBodyBones.Head, "B-head"), (HumanBodyBones.Jaw, "B-jaw")
            };
            foreach (var side in new[] { (Prefix: "Left", Suffix: ".L"), (Prefix: "Right", Suffix: ".R") })
            {
                foreach (var pair in new[] {
                    ("Shoulder", "shoulder"), ("UpperArm", "upperArm"), ("LowerArm", "forearm"),
                    ("Hand", "hand"), ("UpperLeg", "thigh"), ("LowerLeg", "shin"),
                    ("Foot", "foot"), ("Toes", "toe") })
                    mapping.Add((Enum.Parse<HumanBodyBones>(side.Prefix + pair.Item1), "B-" + pair.Item2 + side.Suffix));
                foreach (var finger in new[] { ("Index", "indexFinger"), ("Middle", "middleFinger"),
                    ("Ring", "ringFinger"), ("Little", "pinky"), ("Thumb", "thumb") })
                {
                    string[] parts = { "Proximal", "Intermediate", "Distal" };
                    for (int index = 0; index < parts.Length; index++)
                        mapping.Add((Enum.Parse<HumanBodyBones>(side.Prefix + finger.Item1 + parts[index]),
                            "B-" + finger.Item2 + (index + 1).ToString("00") + side.Suffix));
                }
            }
            foreach (var bone in mapping)
                if (transforms.Count(t => t.name == bone.Bone) != 1)
                    throw new InvalidOperationException("Missing or ambiguous mapped bone: " + bone.Bone);

            var importer = (ModelImporter)AssetImporter.GetAtPath(ModelPath);
            var description = importer.humanDescription;
            description.human = mapping.Select(b => new HumanBone {
                boneName = b.Bone, humanName = HumanTrait.BoneName[(int)b.Human],
                limit = new HumanLimit { useDefaultValues = true }
            }).ToArray();
            description.skeleton = transforms.Select(t => new SkeletonBone {
                name = t.name, position = t.localPosition, rotation = t.localRotation, scale = t.localScale
            }).ToArray();
            description.armStretch = 0;
            description.legStretch = 0;
            description.hasTranslationDoF = false;
            importer.animationType = ModelImporterAnimationType.Human;
            importer.avatarSetup = ModelImporterAvatarSetup.CreateFromThisModel;
            importer.humanDescription = description;
            importer.importBlendShapes = true;
            importer.importNormals = ModelImporterNormals.Import;
            importer.importAnimation = false;
            importer.optimizeGameObjects = false;
            importer.SaveAndReimport();

            var avatar = AssetDatabase.LoadAllAssetsAtPath(ModelPath).OfType<Avatar>().Single();
            model = AssetDatabase.LoadAssetAtPath<GameObject>(ModelPath);
            var meshes = model.GetComponentsInChildren<SkinnedMeshRenderer>(true).Select(x => x.sharedMesh).ToArray();
            var report = new ImportReport {
                unityVersion = Application.unityVersion, candidate = ModelPath, derivativeSha256 = Hash(destination),
                avatarValid = avatar.isValid, avatarHuman = avatar.isHuman, mappedBones = mapping.Count,
                meshVertices = meshes.Select(x => x.vertexCount).ToArray(),
                blendKeys = meshes.SelectMany(m => Enumerable.Range(0, m.blendShapeCount).Select(m.GetBlendShapeName)).ToArray(),
                remaining = "Clip playback, fixed reach/contact, support, twist and garment gates are pending."
            };
            string reportDirectory = Path.Combine(project, "Logs", "art");
            Directory.CreateDirectory(reportDirectory);
            File.WriteAllText(Path.Combine(reportDirectory, "hero-candidate-import.json"), JsonUtility.ToJson(report, true));
            if (!avatar.isValid || !avatar.isHuman) throw new InvalidOperationException("Hero Avatar rejected; inspect import report.");
            Debug.Log("Hero candidate Humanoid Avatar imported. M0.5 motion/contact gates remain pending.");
        }

        static string Hash(string path)
        {
            using var sha = SHA256.Create();
            using var file = File.OpenRead(path);
            return BitConverter.ToString(sha.ComputeHash(file)).Replace("-", "").ToLowerInvariant();
        }

        [Serializable]
        sealed class ImportReport
        {
            public string unityVersion, candidate, derivativeSha256, remaining;
            public bool avatarValid, avatarHuman;
            public int mappedBones;
            public int[] meshVertices;
            public string[] blendKeys;
        }
    }
}
