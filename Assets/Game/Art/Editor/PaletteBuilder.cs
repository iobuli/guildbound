using System;
using System.IO;
using UnityEditor;
using UnityEngine;

namespace Guildbound.Art.Editor
{
    // Codex-owned M0 palette. Only writes named derivatives in our Art area.
    // Geometry, source pack materials and scene assignments are unchanged.
    public static class PaletteBuilder
    {
        const string Folder = "Assets/Game/Art/Palette";
        static readonly (string Name, string Hex, float Smoothness)[] Colours =
        {
            ("DarkTimber", "4A3122", .08f),
            ("MidTimber", "7A5233", .08f),
            ("TimberFrame", "2A211C", .06f),
            ("QuietStone", "8C8A85", .04f),
            ("Thatch", "9C9268", .03f),
            ("GuildRed", "8E2F25", .04f),
            ("BoneCloth", "E6DAC2", .03f)
        };

        [MenuItem("Guildbound/Art/Prepare M0 Palette")]
        public static void Prepare()
        {
            EnsureFolder(Folder);
            var shader = Shader.Find("Universal Render Pipeline/Lit");
            foreach (var entry in Colours)
            {
                string path = Folder + "/" + entry.Name + ".mat";
                string name = "FreeGuild " + entry.Name;
                var material = AssetDatabase.LoadAssetAtPath<Material>(path);
                if (material == null)
                {
                    if (File.Exists(Path.Combine(Path.GetDirectoryName(Application.dataPath), path)))
                        throw new InvalidOperationException("Preserve unrecognized asset: " + path);
                    material = new Material(shader) { name = name };
                    AssetDatabase.CreateAsset(material, path);
                }
                // Unity may normalize a saved native asset's name to its filename.
                else if (material.name != name && material.name != entry.Name)
                    throw new InvalidOperationException("Preserve material owned by another author: " + path);

                ColorUtility.TryParseHtmlString("#" + entry.Hex, out var colour);
                material.color = colour;
                material.SetFloat("_Metallic", 0);
                material.SetFloat("_Smoothness", entry.Smoothness);
                EditorUtility.SetDirty(material);
            }
            AssetDatabase.SaveAssets();
            Debug.Log("FreeGuild M0 palette prepared: 7 matte materials. Scene/lighting acceptance pending.");
        }

        static void EnsureFolder(string path)
        {
            if (AssetDatabase.IsValidFolder(path)) return;
            string parent = path.Substring(0, path.LastIndexOf('/'));
            EnsureFolder(parent);
            AssetDatabase.CreateFolder(parent, path.Substring(path.LastIndexOf('/') + 1));
        }
    }
}
