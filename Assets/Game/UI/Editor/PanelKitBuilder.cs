using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using UnityEditor;
using UnityEditor.Events;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem.UI;
using UnityEngine.SceneManagement;
using UnityEngine.UI;

namespace Guildbound.UI.Editor
{
    public static class PanelKitBuilder
    {
        const string Folder = "Assets/Game/UI/PanelKit";
        [Serializable] sealed class Manifest { public string source_sha256; public Slice[] sprites; public Surface[] surfaces; }
        [Serializable] sealed class Slice { public string id; public int[] unity_rect_px; public int[] nine_slice_lbrt_px; public float pixels_per_unit; }
        [Serializable] sealed class Surface { public string id; public string frame; public float[] size; public float[] padding; }

        [MenuItem("Guildbound/UI/Prepare Empty Panel Kit")]
        public static void Prepare()
        {
            string project = Path.GetDirectoryName(Application.dataPath);
            string source = Path.Combine(project, "Documentation/concepts/hud/panel-kit");
            var manifest = JsonUtility.FromJson<Manifest>(File.ReadAllText(Path.Combine(source, "panel-kit-manifest.json")));
            string sourceImage = Path.Combine(source, "frames-sheet-v1.png");
            Require(Hash(sourceImage) == manifest.source_sha256, "Generated source differs from measured manifest.");
            EnsureFolder(Folder + "/Sprites");
            EnsureFolder(Folder + "/Prefabs");
            string texturePath = Folder + "/frames-sheet-v1.png";
            string textureAbsolute = Path.Combine(project, texturePath);
            if (File.Exists(textureAbsolute)) Require(Hash(textureAbsolute) == manifest.source_sha256, "Preserve different frame sheet.");
            else File.Copy(sourceImage, textureAbsolute);
            AssetDatabase.ImportAsset(texturePath, ImportAssetOptions.ForceSynchronousImport);
            var importer = (TextureImporter)AssetImporter.GetAtPath(texturePath);
            importer.textureType = TextureImporterType.Default;
            importer.alphaSource = TextureImporterAlphaSource.FromInput;
            importer.alphaIsTransparency = true;
            importer.mipmapEnabled = false;
            importer.npotScale = TextureImporterNPOTScale.None;
            importer.maxTextureSize = 2048;
            importer.textureCompression = TextureImporterCompression.Uncompressed;
            importer.wrapMode = TextureWrapMode.Clamp;
            importer.filterMode = FilterMode.Bilinear;
            importer.SaveAndReimport();
            var texture = AssetDatabase.LoadAssetAtPath<Texture2D>(texturePath);
            Require(texture.width == 1536 && texture.height == 1024, "Unexpected imported sheet size.");
            var sprites = new Dictionary<string, Sprite>();
            foreach (var slice in manifest.sprites)
            {
                var r = slice.unity_rect_px;
                var b = slice.nine_slice_lbrt_px;
                var rect = new Rect(r[0], r[1], r[2], r[3]);
                var border = new Vector4(b[0], b[1], b[2], b[3]);
                string path = Folder + "/Sprites/" + slice.id + ".asset";
                var sprite = AssetDatabase.LoadAssetAtPath<Sprite>(path);
                if (sprite == null)
                {
                    Require(!File.Exists(Path.Combine(project, path)), "Preserve unknown sprite asset: " + path);
                    sprite = Sprite.Create(texture, rect, new Vector2(.5f, .5f), slice.pixels_per_unit, 0, SpriteMeshType.FullRect, border);
                    sprite.name = slice.id;
                    AssetDatabase.CreateAsset(sprite, path);
                }
                Require(sprite.rect == rect && sprite.border == border && sprite.texture == texture,
                    "Existing sprite differs from measured cut: " + path);
                sprites.Add(slice.id, sprite);
            }

            var preview = EditorSceneManager.NewPreviewScene();
            try
            {
                foreach (var spec in manifest.surfaces)
                {
                    var panel = MakeSurface(spec, sprites, preview);
                    Save(panel.gameObject, spec.id);
                    UnityEngine.Object.DestroyImmediate(panel.gameObject);
                }
                MakeFixture(manifest, sprites, preview);
            }
            finally { EditorSceneManager.ClosePreviewScene(preview); }
            AssetDatabase.SaveAssets();
            Debug.Log("FreeGuild empty panel kit: 6 measured sprites, 7 content-host prefabs and interactive fixture prepared.");
        }

        static PanelSurface MakeSurface(Surface spec, Dictionary<string, Sprite> sprites, Scene preview)
        {
            var root = Rect(spec.id, null);
            SceneManager.MoveGameObjectToScene(root.gameObject, preview);
            root.sizeDelta = new Vector2(spec.size[0], spec.size[1]);
            var backing = ImageChild("Backplate", root, null, new Color(.043f, .102f, .227f, .85f), 12);
            var frame = ImageChild("Frame", root, sprites[spec.frame], Color.white, 0);
            var accent = ImageChild("ScriptedAccent", root, sprites["focus_frame"], new Color(.4f, .9f, 1f, .8f), 0);
            var host = Rect("Content", root);
            Stretch(host, new Vector2(spec.padding[0], spec.padding[1]));
            var surface = root.gameObject.AddComponent<PanelSurface>();
            surface.Configure(backing, frame, accent, host);
            return surface;
        }

        static void MakeFixture(Manifest manifest, Dictionary<string, Sprite> sprites, Scene preview)
        {
            var root = Rect("PanelKitFixture", null);
            SceneManager.MoveGameObjectToScene(root.gameObject, preview);
            var canvas = root.gameObject.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            var scaler = root.gameObject.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1920, 1080);
            scaler.matchWidthOrHeight = .5f;
            root.gameObject.AddComponent<GraphicRaycaster>();

            var board = MakeSurface(manifest.surfaces.Single(x => x.id == "GuildBoard"), sprites, preview);
            board.transform.SetParent(root, false);
            var button = MakeSurface(manifest.surfaces.Single(x => x.id == "ActionButton"), sprites, preview);
            button.transform.SetParent(board.Content, false);
            var buttonRect = (RectTransform)button.transform;
            buttonRect.anchorMin = buttonRect.anchorMax = new Vector2(.5f, 0);
            buttonRect.pivot = new Vector2(.5f, 0);
            var hit = ImageChild("ControlHitArea", buttonRect, null, new Color(1, 1, 1, 0), 0);
            hit.raycastTarget = true;
            hit.transform.SetAsFirstSibling();
            var selectable = button.gameObject.AddComponent<Button>();
            selectable.targetGraphic = hit;
            selectable.transition = Selectable.Transition.None;
            button.gameObject.AddComponent<PanelFocusFeedback>().Configure(button);
            UnityEventTools.AddPersistentListener(selectable.onClick, board.TogglePreviewAccent);

            var sliderRect = Rect("PanelOpacitySlider", board.Content);
            sliderRect.sizeDelta = new Vector2(320, 64);
            sliderRect.anchoredPosition = new Vector2(0, -200);
            var sliderHit = ImageChild("HitArea", sliderRect, null, new Color(1, 1, 1, 0), 0);
            sliderHit.raycastTarget = true;
            var track = ImageChild("Track", sliderRect, null, new Color(.1f, .25f, .4f, 1), 0);
            track.rectTransform.anchorMin = new Vector2(0, .45f);
            track.rectTransform.anchorMax = new Vector2(1, .55f);
            var fill = ImageChild("Fill", sliderRect, null, new Color(.37f, .84f, 1f, 1), 0);
            fill.rectTransform.anchorMin = new Vector2(0, .45f);
            fill.rectTransform.anchorMax = new Vector2(1, .55f);
            var handle = ImageChild("Handle", sliderRect, null, Color.white, 0);
            handle.raycastTarget = true;
            handle.rectTransform.anchorMin = handle.rectTransform.anchorMax = new Vector2(1, .5f);
            handle.rectTransform.sizeDelta = new Vector2(48, 48);
            var slider = sliderRect.gameObject.AddComponent<Slider>();
            slider.targetGraphic = handle;
            slider.fillRect = fill.rectTransform;
            slider.handleRect = handle.rectTransform;
            slider.minValue = .7f;
            slider.maxValue = 1f;
            slider.value = .85f;
            UnityEventTools.AddPersistentListener(slider.onValueChanged, board.SetBackplateOpacity);

            var events = new GameObject("EventSystem", typeof(EventSystem), typeof(InputSystemUIInputModule));
            events.transform.SetParent(root, false);
            var module = events.GetComponent<InputSystemUIInputModule>();
            module.AssignDefaultActions();
            events.GetComponent<EventSystem>().firstSelectedGameObject = button.gameObject;
            Save(root.gameObject, "PanelKitFixture");
            UnityEngine.Object.DestroyImmediate(root.gameObject);
        }

        static Image ImageChild(string name, RectTransform parent, Sprite sprite, Color colour, float padding)
        {
            var rect = Rect(name, parent);
            Stretch(rect, new Vector2(padding, padding));
            var image = rect.gameObject.AddComponent<Image>();
            image.sprite = sprite;
            image.color = colour;
            image.raycastTarget = false;
            if (sprite != null) { image.type = Image.Type.Sliced; image.fillCenter = false; }
            return image;
        }

        static RectTransform Rect(string name, Transform parent)
        {
            var rect = new GameObject(name, typeof(RectTransform)).GetComponent<RectTransform>();
            rect.SetParent(parent, false);
            return rect;
        }
        static void Stretch(RectTransform rect, Vector2 padding)
        {
            rect.anchorMin = Vector2.zero;
            rect.anchorMax = Vector2.one;
            rect.offsetMin = padding;
            rect.offsetMax = -padding;
        }
        static void Save(GameObject root, string id)
        {
            string path = Folder + "/Prefabs/" + id + ".prefab";
            var existing = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            if (existing != null) Require(existing.GetComponentInChildren<PanelSurface>() != null, "Preserve unknown prefab: " + path);
            PrefabUtility.SaveAsPrefabAsset(root, path);
        }
        static string Hash(string path)
        {
            using var sha = SHA256.Create();
            return BitConverter.ToString(sha.ComputeHash(File.ReadAllBytes(path))).Replace("-", "").ToLowerInvariant();
        }
        static void Require(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
        static void EnsureFolder(string path)
        {
            if (AssetDatabase.IsValidFolder(path)) return;
            string parent = path.Substring(0, path.LastIndexOf('/'));
            EnsureFolder(parent);
            AssetDatabase.CreateFolder(parent, path.Substring(path.LastIndexOf('/') + 1));
        }
    }
}
