using System;
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.Events;
using UnityEngine.SceneManagement;
using UnityEngine.UI;

namespace Guildbound.UI.Editor
{
    public static class PanelKitVerifier
    {
        [MenuItem("Guildbound/UI/Verify Empty Panel Kit")]
        public static void Run()
        {
            var scene = EditorSceneManager.NewPreviewScene();
            try
            {
                var prefab = AssetDatabase.LoadAssetAtPath<GameObject>("Assets/Game/UI/PanelKit/Prefabs/PanelKitFixture.prefab");
                var root = (GameObject)PrefabUtility.InstantiatePrefab(prefab, scene);
                var board = root.GetComponentInChildren<PanelSurface>();
                var canvas = root.GetComponent<Canvas>();
                var scaler = root.GetComponent<CanvasScaler>();
                Require(scaler.referenceResolution == new Vector2(1920, 1080), "Reference canvas differs.");
                Canvas.ForceUpdateCanvases();
                board.Content.ForceUpdateRectTransforms();
                Require(Vector2.Distance(board.Content.rect.size, new Vector2(1024, 664)) < .1f, "Content host size differs.");
                var button = root.GetComponentInChildren<Button>();
                Require(button.onClick.GetPersistentEventCount() == 1, "Button callback not serialized.");
                // Production callbacks are RuntimeOnly. Enable them on this unsaved
                // Editor fixture instance so dispatch tests can exercise the saved binding.
                button.onClick.SetPersistentListenerState(0, UnityEventCallState.EditorAndRuntime);
                var events = root.GetComponentInChildren<EventSystem>();
                board.SetAccentVisible(false);
                ExecuteEvents.Execute(button.gameObject, new PointerEventData(events) { button = PointerEventData.InputButton.Left }, ExecuteEvents.pointerClickHandler);
                Require(board.AccentVisible, "Actual Button pointer-click did not reach panel action.");
                ExecuteEvents.Execute(button.gameObject, new BaseEventData(events), ExecuteEvents.submitHandler);
                Require(!board.AccentVisible, "Keyboard/controller submit did not reach panel action.");
                var slider = root.GetComponentInChildren<Slider>();
                Require(slider.onValueChanged.GetPersistentEventCount() == 1, "Slider callback not serialized.");
                slider.onValueChanged.SetPersistentListenerState(0, UnityEventCallState.EditorAndRuntime);
                slider.value = .75f;
                Require(Mathf.Abs(board.BackplateOpacity - .75f) < .001f, "Slider did not update backplate.");
                board.SetBackplateOpacity(0);
                Require(board.BackplateOpacity >= .70f, "Readability floor failed.");
                int decorations = 0;
                foreach (var image in root.GetComponentsInChildren<Image>(true))
                {
                    if (image.name == "Frame" || image.name == "ScriptedAccent" || image.name == "Backplate")
                    { Require(!image.raycastTarget, "Decoration intercepts input: " + image.name); decorations++; }
                    if (image.name == "Frame") Require(image.type == Image.Type.Sliced && !image.fillCenter, "Frame stretches centre.");
                }
                string report = "Unity " + Application.unityVersion + "\nPASS: canvas 1920x1080; board content 1024x664; serialized Button pointer-click + submit; slider callback; opacity floor; " + decorations + " nonblocking decorations.\nLIMIT: input callbacks tested via ExecuteEvents; end-to-end hardware input, in-game layout/contrast and game-state bindings pending.\n";
                var project = Path.GetDirectoryName(Application.dataPath);
                File.WriteAllText(Path.Combine(project, "Documentation/concepts/hud/panel-kit/unity-verification.txt"), report);
                Debug.Log(report);
                UnityEngine.Object.DestroyImmediate(root);
            }
            finally { EditorSceneManager.ClosePreviewScene(scene); }
        }
        static void Require(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    }
}
