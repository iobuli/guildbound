using System;
using System.IO;
using System.Linq;
using TMPro;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;
using UnityEngine.SceneManagement;

namespace Guildbound.UI.Editor
{
    public static class HudPlaytestBuilder
    {
        const string Scene = "Assets/Game/UI/PanelKit/HudPlaytest.unity";

        [MenuItem("Guildbound/UI/Play HUD Sandbox")]
        public static void OpenInEditor()
        {
            EditorSceneManager.OpenScene(Scene);
            EditorApplication.ExecuteMenuItem("Window/General/Game");
            EditorApplication.delayCall += () => EditorApplication.isPlaying = true;
        }

        [InitializeOnLoadMethod]
        static void FocusSandboxAfterReload()
        {
            EditorApplication.delayCall += () =>
            {
                if (File.Exists(OpenRequest))
                {
                    if (EditorApplication.isPlaying)
                    {
                        EditorApplication.playModeStateChanged += ResumeRequestedSandbox;
                        EditorApplication.isPlaying = false;
                    }
                    else ConsumeOpenRequest();
                    return;
                }
                if (EditorApplication.isPlaying && SceneManager.GetActiveScene().path == Scene)
                    EditorApplication.ExecuteMenuItem("Window/General/Game");
            };
        }

        static string OpenRequest => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "FreeGuild/HudPlaytest/open-in-editor.request");
        static void ResumeRequestedSandbox(PlayModeStateChange state)
        {
            if (state != PlayModeStateChange.EnteredEditMode) return;
            EditorApplication.playModeStateChanged -= ResumeRequestedSandbox;
            ConsumeOpenRequest();
        }
        static void ConsumeOpenRequest()
        {
            File.Delete(OpenRequest);
            OpenInEditor();
        }

        [MenuItem("Guildbound/UI/Build Small HUD Playtest")]
        public static void Build()
        {
            PrepareScene();
            string project = Path.GetDirectoryName(Application.dataPath);
            string destination = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "FreeGuild/HudPlaytest/20261011-v1");
            Directory.CreateDirectory(destination);
            string exe = Path.Combine(destination, "FreeGuild-HUD.exe");
            var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
            {
                scenes = new[] { Scene }, locationPathName = exe,
                target = BuildTarget.StandaloneWindows64, options = BuildOptions.Development
            });
            if (report.summary.result != BuildResult.Succeeded)
                throw new InvalidOperationException("HUD playtest build failed: " + report.summary.result + "/" + report.summary.totalErrors + " errors");
            File.WriteAllText(Path.Combine(project, "Documentation/concepts/hud/panel-kit/playtest-path.txt"), exe);
            Debug.Log("FreeGuild HUD sandbox built: " + exe + "; " + report.summary.totalErrors + " errors.");
        }

        [MenuItem("Guildbound/UI/Prepare HUD Sandbox Scene")]
        public static void PrepareScene()
        {
            // Read Claude's preview as a source. Save only our own derived scene.
            var scene = EditorSceneManager.OpenScene("Assets/Game/Scenes/HudPreview.unity");
            var model = UnityEngine.Object.FindFirstObjectByType<HudPreviewModel>();
            var view = UnityEngine.Object.FindFirstObjectByType<HudView>();
            model.demo = false;
            var controls = new GameObject("HUD Sandbox Controls").AddComponent<HudPlaytestControls>();
            controls.model = model;
            controls.view = view;
            var canvas = view.GetComponent<Canvas>();
            var labelRect = new GameObject("Sandbox controls", typeof(RectTransform)).GetComponent<RectTransform>();
            labelRect.SetParent(canvas.transform, false);
            labelRect.anchorMin = labelRect.anchorMax = new Vector2(.5f, 0);
            labelRect.pivot = new Vector2(.5f, 0);
            labelRect.anchoredPosition = new Vector2(0, 10);
            labelRect.sizeDelta = new Vector2(1800, 40);
            var backing = labelRect.gameObject.AddComponent<Image>();
            backing.color = new Color(.027f, .075f, .16f, .95f);
            backing.raycastTarget = false;
            var textRect = new GameObject("Instruction text", typeof(RectTransform)).GetComponent<RectTransform>();
            textRect.SetParent(labelRect, false);
            textRect.anchorMin = Vector2.zero; textRect.anchorMax = Vector2.one;
            textRect.offsetMin = new Vector2(12, 0); textRect.offsetMax = new Vector2(-12, 0);
            var text = textRect.gameObject.AddComponent<TextMeshProUGUI>();
            text.font = TMP_Settings.defaultFontAsset;
            text.fontSize = 23;
            text.color = new Color(.92f, .97f, 1f, 1);
            text.alignment = TextAlignmentOptions.Center;
            text.raycastTarget = false;
            controls.instructions = text;
            EditorSceneManager.SaveScene(scene, Scene, true);

            Debug.Log("FreeGuild HUD sandbox scene prepared with the production one-canvas HUD.");
        }
    }
}
