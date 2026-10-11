using System;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class CodexHudBuildThroughAura
{
    public static string Execute()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Stop the HUD preview before rebuilding.");
        var scene = EditorSceneManager.GetActiveScene();
        if (scene.isDirty && scene.path != "Assets/Game/Scenes/HudPreview.unity" && scene.path != "Assets/Game/UI/PanelKit/HudPlaytest.unity")
            throw new InvalidOperationException("The open scene has unsaved edits; preserve them before rebuilding.");
        Guildbound.UI.Editor.HudBuilder.Build();
        Guildbound.UI.Editor.HudPlaytestBuilder.PrepareScene();
        EditorSceneManager.OpenScene("Assets/Game/UI/PanelKit/HudPlaytest.unity");
        EditorApplication.ExecuteMenuItem("Window/General/Game");
        Application.runInBackground = true;
        return "Production HUD rebuilt; one Canvas; HudPlaytest scene open. Play Mode is still off.";
    }
}
