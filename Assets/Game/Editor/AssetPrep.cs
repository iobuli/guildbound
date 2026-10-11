using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace Guildbound.Editor
{
    // Makes the imported packs usable under URP and renders contact sheets so the look can be judged by eye.
    public static class AssetPrep
    {
        [MenuItem("Guildbound/Assets/Convert Built-in Materials")]
        public static void ConvertMaterials()
        {
#pragma warning disable 618
            UnityEditor.Rendering.Universal.Converters.RunInBatchMode(UnityEditor.Rendering.Universal.ConverterContainerId.BuiltInToURP,
                new List<UnityEditor.Rendering.Universal.ConverterId> { UnityEditor.Rendering.Universal.ConverterId.Material, UnityEditor.Rendering.Universal.ConverterId.ReadonlyMaterial },
                UnityEditor.Rendering.Universal.ConverterFilter.Inclusive);
#pragma warning restore 618
            AssetDatabase.SaveAssets();
            var left = AssetDatabase.FindAssets("t:Material", new[] { "Assets" }).Select(AssetDatabase.GUIDToAssetPath)
                .Select(p => (p, s: AssetDatabase.LoadAssetAtPath<Material>(p).shader.name))
                .Where(x => !x.s.StartsWith("Universal Render Pipeline") && !x.s.StartsWith("Shader Graphs") && !x.s.StartsWith("Skybox")).ToList();
            File.WriteAllLines("Logs/catalog/unconverted.txt", left.Select(x => x.s + "\t" + x.p));
            Debug.Log("GUILDBOUND_MATERIALS_CONVERTED unconverted=" + left.Count);
        }

        [MenuItem("Guildbound/Assets/Render Contact Sheets")]
        public static void Sheets()
        {
            var prefabs = File.ReadAllLines("Logs/catalog/prefabs.txt").Select(l => l.Split('\t')).ToList();
            string Path(string[] f) => f.Last();
            Sheet("characters", prefabs.Where(f => f[0].Contains("skinned") && f[0].Contains("animator")).Select(Path), 1.0f, false);
            Sheet("town", prefabs.Where(f => Path(f).Contains("/RPGPP_LT/")).Select(Path), 0.9f, false);
            Sheet("tavern", prefabs.Where(f => Path(f).Contains("/Low Poly Tarvern Pack/")).Select(Path), 0.9f, false);
            Sheet("nature", prefabs.Where(f => Path(f).Contains("/Anime Trees/")).Select(Path), 0.9f, false);
            Sheet("vfx_hovl", prefabs.Where(f => Path(f).Contains("/Hovl Studio/")).Select(Path), 1.2f, true);
            Sheet("vfx_arsenal_fire_lightning", prefabs.Where(f => Path(f).Contains("/Magic Pig") && (Path(f).Contains("- Fire ") || Path(f).Contains("- Lightning ") || Path(f).Contains("- Light ") || Path(f).Contains("- Frost "))).Select(Path), 1.2f, true);
            Debug.Log("GUILDBOUND_SHEETS_DONE");
        }

        // One tile per prefab: three-quarter view framed on its renderer bounds; particle systems are pre-simulated.
        internal static void Sheet(string name, IEnumerable<string> paths, float margin, bool simulate, float time = .8f)
        {
            var list = paths.ToList();
            if (list.Count == 0) return;
            const int tile = 256;
            int columns = 8, rows = Mathf.CeilToInt(list.Count / (float)columns);
            EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            var sun = new GameObject("Sun").AddComponent<Light>();
            sun.type = LightType.Directional; sun.intensity = 1.6f; sun.transform.rotation = Quaternion.Euler(40, -30, 0);
            RenderSettings.ambientMode = AmbientMode.Flat; RenderSettings.ambientLight = new Color(.45f, .47f, .5f);
            var eye = new GameObject("Eye").AddComponent<Camera>();
            eye.clearFlags = CameraClearFlags.SolidColor; eye.backgroundColor = simulate ? new Color(.06f, .07f, .09f) : new Color(.62f, .7f, .78f);
            eye.fieldOfView = 30; eye.nearClipPlane = .01f; eye.farClipPlane = 500;
            eye.GetUniversalAdditionalCameraData().renderPostProcessing = false;
            var target = new RenderTexture(tile, tile, 24, RenderTextureFormat.ARGB32);
            var read = new Texture2D(tile, tile, TextureFormat.RGBA32, false);
            var sheet = new Texture2D(columns * tile, rows * tile, TextureFormat.RGBA32, false);
            var index = new List<string>();
            for (int i = 0; i < list.Count; i++)
            {
                var instance = (GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(list[i]));
                if (simulate) foreach (var ps in instance.GetComponentsInChildren<ParticleSystem>(true)) ps.Simulate(time, true, true);
                var renderers = instance.GetComponentsInChildren<Renderer>(true);
                var bounds = renderers.Length > 0 ? renderers.Select(r => r.bounds).Aggregate((a, b) => { a.Encapsulate(b); return a; }) : new Bounds(Vector3.zero, Vector3.one);
                float radius = Mathf.Max(bounds.extents.magnitude, .2f) * margin;
                var direction = Quaternion.Euler(18, 150, 0) * Vector3.forward;
                eye.transform.position = bounds.center - direction * (radius / Mathf.Sin(eye.fieldOfView * .5f * Mathf.Deg2Rad));
                eye.transform.rotation = Quaternion.LookRotation(direction);
                var request = new RenderPipeline.StandardRequest { destination = target };
                RenderPipeline.SubmitRenderRequest(eye, request);
                RenderTexture.active = target;
                read.ReadPixels(new Rect(0, 0, tile, tile), 0, 0); read.Apply();
                RenderTexture.active = null;
                int x = i % columns, y = rows - 1 - i / columns;
                sheet.SetPixels(x * tile, y * tile, tile, tile, read.GetPixels());
                index.Add(i + "\t" + list[i]);
                Object.DestroyImmediate(instance);
            }
            sheet.Apply();
            Directory.CreateDirectory("Logs/sheets");
            File.WriteAllBytes("Logs/sheets/" + name + ".png", sheet.EncodeToPNG());
            File.WriteAllLines("Logs/sheets/" + name + ".txt", index);
        }
    }
}
