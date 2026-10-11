using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

namespace Guildbound.Editor
{
    // Inventory of everything the asset packs provide: materials by shader, models by rig, clips with motion data, prefabs.
    public static class Catalog
    {
        static string Pack(string path) => path.Split('/')[1];

        [MenuItem("Guildbound/Catalog Assets")]
        public static void Run()
        {
            Directory.CreateDirectory("Logs/catalog");
            string[] packs = AssetDatabase.GetSubFolders("Assets").Where(f => f != "Assets/Game").ToArray();

            var materials = AssetDatabase.FindAssets("t:Material", packs).Select(AssetDatabase.GUIDToAssetPath).ToList();
            File.WriteAllLines("Logs/catalog/materials.txt", materials.Select(p => { var m = AssetDatabase.LoadAssetAtPath<Material>(p); return (m.shader != null ? m.shader.name : "NULL") + "\t" + p; }).OrderBy(l => l));

            var models = new List<string>();
            var clips = new List<string>();
            foreach (string path in AssetDatabase.FindAssets("t:Model", packs).Select(AssetDatabase.GUIDToAssetPath))
            {
                if (!(AssetImporter.GetAtPath(path) is ModelImporter importer)) continue;
                var assets = AssetDatabase.LoadAllAssetsAtPath(path);
                var avatar = assets.OfType<Avatar>().FirstOrDefault();
                var root = AssetDatabase.LoadAssetAtPath<GameObject>(path);
                int skins = root != null ? root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Length : 0;
                int meshes = root != null ? root.GetComponentsInChildren<MeshFilter>(true).Length : 0;
                var takes = assets.OfType<AnimationClip>().Where(c => !c.name.StartsWith("__preview")).ToList();
                models.Add(importer.animationType + "\tavatar=" + (avatar == null ? "none" : (avatar.isHuman ? "human" : "generic") + (avatar.isValid ? "" : "-INVALID")) + "\tskins=" + skins + "\tmeshes=" + meshes + "\tclips=" + takes.Count + "\t" + path);
                foreach (var clip in takes)
                    clips.Add(Pack(path) + "\t" + clip.name + "\t" + clip.length.ToString("0.00") + "s\t" + (clip.isLooping ? "loop" : "once") + "\t" + (clip.isHumanMotion ? "human" : "generic") + "\tspeed=" + clip.averageSpeed.magnitude.ToString("0.00") + "\tturn=" + clip.averageAngularSpeed.ToString("0.0") + "\t" + path);
            }
            foreach (string path in AssetDatabase.FindAssets("t:AnimationClip", packs).Select(AssetDatabase.GUIDToAssetPath).Where(p => p.EndsWith(".anim")))
            {
                var clip = AssetDatabase.LoadAssetAtPath<AnimationClip>(path);
                clips.Add(Pack(path) + "\t" + clip.name + "\t" + clip.length.ToString("0.00") + "s\t" + (clip.isLooping ? "loop" : "once") + "\t" + (clip.isHumanMotion ? "human" : "generic") + "\tspeed=" + clip.averageSpeed.magnitude.ToString("0.00") + "\tturn=" + clip.averageAngularSpeed.ToString("0.0") + "\t" + path);
            }
            File.WriteAllLines("Logs/catalog/models.txt", models.OrderBy(l => l.Split('\t').Last()));
            File.WriteAllLines("Logs/catalog/clips.txt", clips.OrderBy(l => l));

            var prefabs = AssetDatabase.FindAssets("t:Prefab", packs).Select(AssetDatabase.GUIDToAssetPath).ToList();
            File.WriteAllLines("Logs/catalog/prefabs.txt", prefabs.Select(p =>
            {
                var go = AssetDatabase.LoadAssetAtPath<GameObject>(p);
                var kinds = new List<string>();
                if (go.GetComponentInChildren<Animator>(true)) kinds.Add("animator");
                if (go.GetComponentInChildren<SkinnedMeshRenderer>(true)) kinds.Add("skinned");
                if (go.GetComponentInChildren<ParticleSystem>(true)) kinds.Add("particles");
                if (go.GetComponentInChildren<Collider>(true)) kinds.Add("collider");
                if (go.GetComponentInChildren<LODGroup>(true)) kinds.Add("lod");
                var bounds = go.GetComponentsInChildren<Renderer>(true).Select(r => r.bounds).DefaultIfEmpty().Aggregate((a, b) => { a.Encapsulate(b); return a; });
                int missing = go.GetComponentsInChildren<Component>(true).Count(c => c == null);
                return string.Join(",", kinds) + "\tsize=" + bounds.size.ToString("F1") + (missing > 0 ? "\tmissingScripts=" + missing : "") + "\t" + p;
            }).OrderBy(l => l.Split('\t').Last()));

            var summary = packs.Select(pack => pack + ": " +
                materials.Count(p => p.StartsWith(pack + "/")) + " materials, " +
                models.Count(m => m.Contains("\t" + pack + "/")) + " models, " +
                clips.Count(c => c.Contains("\t" + pack + "/")) + " clips, " +
                prefabs.Count(p => p.StartsWith(pack + "/")) + " prefabs").ToList();
            summary.Add("");
            summary.Add("Shaders in use:");
            summary.AddRange(materials.Select(p => AssetDatabase.LoadAssetAtPath<Material>(p).shader?.name ?? "NULL").GroupBy(s => s).OrderByDescending(g => g.Count()).Select(g => "  " + g.Count() + "  " + g.Key));
            File.WriteAllLines("Logs/catalog/summary.txt", summary);
            Debug.Log("GUILDBOUND_CATALOG_DONE");
        }
    }
}
