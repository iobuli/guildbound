using System.Collections.Generic;
using System.Linq;
using UnityEditor;
using UnityEngine;

namespace Guildbound.Editor
{
    // Fixes "Transform 'X' not found in HumanDescription": a pack FBX gained a transform above the hips (e.g.
    // FightingMotions 'Root') after its avatar was configured. Inserts only the missing ancestors of the hips into
    // the stored skeleton, re-expressing the next stored bone relative to the inserted one. Every other stored rest
    // pose and the human mapping stay exactly as the pack author set them. Clips copying that avatar are reimported.
    public static class AvatarRepair
    {
        static readonly string[] Folders = { "Assets/FightingMotionsVolume1", "Assets/ExplosiveLLC", "Assets/Idle MoCap", "Assets/MoCapCentral", "Assets/Kevin Iglesias" };

        [MenuItem("Guildbound/Assets/Repair Pack Avatars")]
        public static void Run()
        {
            var importers = AssetDatabase.FindAssets("t:Model", Folders).Select(AssetDatabase.GUIDToAssetPath)
                .Select(p => AssetImporter.GetAtPath(p) as ModelImporter)
                .Where(m => m != null && m.animationType == ModelImporterAnimationType.Human).ToList();

            var fixedSources = new HashSet<string>();
            foreach (var m in importers.Where(m => m.avatarSetup == ModelImporterAvatarSetup.CreateFromThisModel))
            {
                var hd = m.humanDescription;
                var hipsName = hd.human.FirstOrDefault(h => h.humanName == "Hips").boneName;
                if (string.IsNullOrEmpty(hipsName) || hd.skeleton == null || hd.skeleton.Length == 0) continue;
                var model = AssetDatabase.LoadAssetAtPath<GameObject>(m.assetPath);
                var hips = model.GetComponentsInChildren<Transform>(true).FirstOrDefault(t => t.name == hipsName);
                if (hips == null) continue;

                var chain = new List<Transform>();
                for (var t = hips.parent; t != null && t != model.transform; t = t.parent) chain.Insert(0, t);
                var skeleton = hd.skeleton.ToList();
                bool changed = false;
                foreach (var t in chain)
                {
                    if (skeleton.Any(b => b.name == t.name)) continue;
                    int parent = skeleton.FindIndex(b => b.name == t.parent.name);
                    int child = skeleton.FindIndex(b => b.name == (chain.IndexOf(t) + 1 < chain.Count ? chain[chain.IndexOf(t) + 1].name : hipsName));
                    if (parent < 0 || child < 0) continue;
                    var inserted = new SkeletonBone { name = t.name, position = t.localPosition, rotation = t.localRotation, scale = t.localScale };
                    var c = skeleton[child];
                    var inv = Matrix4x4.TRS(inserted.position, inserted.rotation, inserted.scale).inverse;
                    var local = inv * Matrix4x4.TRS(c.position, c.rotation, c.scale);
                    c.position = local.GetColumn(3);
                    c.rotation = local.rotation;
                    skeleton[child] = c;
                    skeleton.Insert(parent + 1, inserted);
                    changed = true;
                }
                if (!changed) continue;
                hd.skeleton = skeleton.ToArray();
                m.humanDescription = hd;
                m.SaveAndReimport();
                fixedSources.Add(m.assetPath);
            }

            int dependents = 0;
            foreach (var m in importers.Where(m => m.avatarSetup == ModelImporterAvatarSetup.CopyFromOther && m.sourceAvatar != null))
                if (fixedSources.Contains(AssetDatabase.GetAssetPath(m.sourceAvatar))) { m.SaveAndReimport(); dependents++; }
            Debug.Log($"GUILDBOUND_AVATARS_REPAIRED human_models={importers.Count} sources_fixed={fixedSources.Count} dependents_reimported={dependents}\n" + string.Join("\n", fixedSources));
        }
    }
}
