using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using UnityEditor;
using UnityEngine;

// Read-only audit: can every animation clip we own play on ONE base model with no edits?
// Decisive evidence is per-FBX: ModelImporter.animationType, AnimationClip.isHumanMotion (only humanoid
// clips retarget), and whether a model's generated Avatar is valid + human.
public static class AnimRigAudit
{
    class Pack
    {
        public string Name;
        public int Fbx, Clips, Human, Generic, Legacy;
    }

    public static string Execute()
    {
        var packs = new Dictionary<string, Pack>();
        var candidates = new List<string>();
        var brawlerClips = new List<string>();
        int scanned = 0;

        foreach (var guid in AssetDatabase.FindAssets("t:Model"))
        {
            string path = AssetDatabase.GUIDToAssetPath(guid);
            if (!path.StartsWith("Assets/")) continue;
            scanned++;

            var imp = AssetImporter.GetAtPath(path) as ModelImporter;
            var assets = AssetDatabase.LoadAllAssetsAtPath(path);

            int clips = 0, human = 0, generic = 0;
            foreach (var a in assets)
            {
                var c = a as AnimationClip;
                if (c == null || c.name.StartsWith("__preview__")) continue;
                clips++;
                if (c.isHumanMotion) human++; else generic++;
                if (path.Contains("Brawler Animations Sample")) brawlerClips.Add(c.name + (c.isHumanMotion ? " [human]" : " [generic]"));
            }

            string key = PackKey(path);
            if (!packs.TryGetValue(key, out var p)) { p = new Pack { Name = key }; packs[key] = p; }
            p.Fbx++;
            p.Clips += clips;
            p.Human += human;
            p.Generic += generic;

            var avatar = assets.OfType<Avatar>().FirstOrDefault();
            if (avatar != null || (imp != null && imp.animationType == ModelImporterAnimationType.Human))
            {
                string type = imp != null ? imp.animationType.ToString() : "?";
                string valid = avatar == null ? "no-avatar" : (avatar.isValid ? "valid" : "INVALID") + (avatar.isHuman ? "+human" : "+non-human");
                candidates.Add(string.Format("{0}  type={1} avatar={2} clips={3}", path, type, valid, clips));
            }
        }

        var sb = new StringBuilder();
        sb.AppendLine("SCANNED " + scanned + " model assets");
        sb.AppendLine();
        sb.AppendLine("PER PACK (fbx / clips / humanoid / generic)");
        foreach (var p in packs.Values.OrderByDescending(x => x.Clips))
            if (p.Clips > 0 || p.Fbx > 3)
                sb.AppendLine(string.Format("  {0,-46} {1,5} fbx {2,6} clips  human {3,6}  generic {4,6}", p.Name, p.Fbx, p.Clips, p.Human, p.Generic));

        sb.AppendLine();
        sb.AppendLine("MODELS WITH AN AVATAR (base-model candidates)");
        candidates.Sort();
        foreach (var c in candidates) sb.AppendLine("  " + c);

        sb.AppendLine();
        sb.AppendLine("BRAWLER SAMPLE CLIPS (" + brawlerClips.Count + ")");
        brawlerClips.Sort();
        foreach (var c in brawlerClips) sb.AppendLine("  " + c);

        return sb.ToString();
    }

    static string PackKey(string path)
    {
        var s = path.Split('/');
        if (s.Length < 2) return "(root)";
        string key = s[1];
        if ((key == "ExplosiveLLC" || key == "Kevin Iglesias" || key == "Magic Pig Games (Infinity PBR)") && s.Length > 2)
            key += "/" + s[2];
        return key;
    }
}
