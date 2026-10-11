using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using UnityEditor;
using UnityEditor.Rendering.Universal.ShaderGUI;
using UnityEngine;

namespace Guildbound.Editor
{
    // Repairs Magic Arsenal prefabs in place. We own only the "Projectile Factory for Magic Arsenal" add-on: its
    // materials, textures, audio and scripts live in two packs we do not own, so every renderer points at a missing
    // material. Each missing material guid is mapped to a URP Particles/Unlit material built on an owned Hovl texture;
    // missing MonoBehaviours are stripped (our code moves projectiles). Re-run after re-importing the pack.
    public static class FxRepair
    {
        const string Root = "Assets/Magic Pig Games (Infinity PBR)/Archanor Magic Arsenal/";
        const string Out = Root + "Materials/Repaired/";
        const string Hovl = "Assets/Hovl Studio/Magic effects pack/Textures/";

        static readonly string[] Prefabs =
        {
            "Projectiles/Magic Arsenal - Fire Charge.prefab",
            "Projectiles/Magic Arsenal - Fire Missile Tiny.prefab",
            "Particles/Muzzles/Magic Arsenal - Fire Muzzle Normal.prefab",
            "Particles/Impacts & Explosions/Magic Arsenal - Fire Explosion Tiny.prefab",
        };

        // missing material guid -> (repaired material name, Hovl texture, additive)
        static readonly Dictionary<string, (string name, string texture, bool additive)> Map = new()
        {
            ["5ebadb3a85da1d54297d8a21786bec58"] = ("Arsenal Glow", "GlowFree1", true),
            ["b4a3cd532f543a4448281dc9c13d9aec"] = ("Arsenal Smoke", "SmokeFree1", false),
            ["27322810aa42bc943960f0952f27361d"] = ("Arsenal Ring", "Circle2", true),
            ["cc1f87ebec52f2a42a16c1b98ea1c2fb"] = ("Arsenal Core", "FlashFree1", true),
            ["01e0386bc3c5b2f45b5b2735939daa3f"] = ("Arsenal Muzzle", "FlashFree3", true),
            ["7c707bedfbf7942459e20471e6f8d740"] = ("Arsenal Fire", "FlashFree1", true),
        };

        [MenuItem("Guildbound/Assets/Repair Magic Arsenal Fire")]
        public static void Run()
        {
            Directory.CreateDirectory(Out);
            var materials = new Dictionary<string, Material>();
            foreach (var (guid, spec) in Map) materials[guid] = Build(spec.name, spec.texture, spec.additive);

            int assigned = 0, unmapped = 0, scripts = 0;
            foreach (var rel in Prefabs)
            {
                var path = Root + rel;
                var byObject = RendererGuids(File.ReadAllText(path));
                var contents = PrefabUtility.LoadPrefabContents(path);
                foreach (var t in contents.GetComponentsInChildren<Transform>(true))
                    scripts += GameObjectUtility.RemoveMonoBehavioursWithMissingScript(t.gameObject);
                foreach (var r in contents.GetComponentsInChildren<ParticleSystemRenderer>(true))
                {
                    if (byObject.TryGetValue(r.gameObject.name, out var guid) && materials.TryGetValue(guid, out var m)) { r.sharedMaterial = m; assigned++; }
                    else if (materials.ContainsValue(r.sharedMaterial)) assigned++;  // repaired by an earlier run
                    else if (r.sharedMaterial == null) { unmapped++; Debug.LogWarning("FX_REPAIR unmapped renderer " + rel + " / " + r.gameObject.name); }
                }
                PrefabUtility.SaveAsPrefabAsset(contents, path);
                PrefabUtility.UnloadPrefabContents(contents);
            }
            AssetDatabase.SaveAssets();
            Debug.Log($"GUILDBOUND_FX_REPAIRED prefabs={Prefabs.Length} renderers={assigned} unmapped={unmapped} missing_scripts_removed={scripts}");
            AssetPrep.Sheet("vfx_arsenal_fire_repaired", Prefabs.Select(p => Root + p), 1.2f, true);
            AssetPrep.Sheet("vfx_arsenal_fire_repaired_early", Prefabs.Select(p => Root + p), 1.2f, true, .12f);
        }

        // GameObject name -> material guid of its ParticleSystemRenderer, read from the prefab YAML because a missing
        // asset reference loads as null and no longer carries its guid.
        static Dictionary<string, string> RendererGuids(string yaml)
        {
            var docs = Regex.Split(yaml, @"\n--- !u!(\d+) &(-?\d+)");
            var names = new Dictionary<string, string>();
            var result = new Dictionary<string, string>();
            for (int i = 1; i + 2 < docs.Length; i += 3)
                if (docs[i] == "1") names[docs[i + 1]] = Regex.Match(docs[i + 2], @"m_Name: (.*)").Groups[1].Value.Trim();
            for (int i = 1; i + 2 < docs.Length; i += 3)
            {
                if (docs[i] != "199") continue;
                var go = Regex.Match(docs[i + 2], @"m_GameObject: \{fileID: (-?\d+)").Groups[1].Value;
                var mat = Regex.Match(docs[i + 2], @"m_Materials:\s*\n\s*- \{fileID: -?\d+, guid: ([0-9a-f]{32})");
                if (mat.Success && names.TryGetValue(go, out var n)) result[n] = mat.Groups[1].Value;
            }
            return result;
        }

        static Material Build(string name, string texture, bool additive)
        {
            var path = Out + name + ".mat";
            var m = AssetDatabase.LoadAssetAtPath<Material>(path);
            if (m == null) { m = new Material(Shader.Find("Universal Render Pipeline/Particles/Unlit")); AssetDatabase.CreateAsset(m, path); }
            m.shader = Shader.Find("Universal Render Pipeline/Particles/Unlit");
            m.SetTexture("_BaseMap", AssetDatabase.LoadAssetAtPath<Texture2D>(Hovl + texture + ".png"));
            m.SetColor("_BaseColor", Color.white);
            m.SetFloat("_Surface", (float)BaseShaderGUI.SurfaceType.Transparent);
            m.SetFloat("_Blend", (float)(additive ? BaseShaderGUI.BlendMode.Additive : BaseShaderGUI.BlendMode.Alpha));
            BaseShaderGUI.SetMaterialKeywords(m, null, ParticleGUI.SetMaterialKeywords);
            EditorUtility.SetDirty(m);
            return m;
        }
    }
}
