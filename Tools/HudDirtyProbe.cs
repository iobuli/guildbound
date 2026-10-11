using System.IO;
using System.Linq;
using System.Text;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

// READ-ONLY probe. Reports the active scene's dirty state and the prefab override
// paths held in memory, next to the same paths as saved on disk.
// Saves nothing, changes nothing.
public static class HudDirtyProbe
{
    public static string Execute()
    {
        var sb = new StringBuilder();
        var scene = EditorSceneManager.GetActiveScene();

        sb.AppendLine("scene=" + scene.name);
        sb.AppendLine("isDirty=" + scene.isDirty);
        sb.AppendLine("path=" + scene.path);
        sb.AppendLine("rootCount=" + scene.rootCount);

        foreach (var go in scene.GetRootGameObjects())
        {
            var mods = PrefabUtility.GetPropertyModifications(go);
            sb.AppendLine("root=" + go.name
                          + " status=" + PrefabUtility.GetPrefabInstanceStatus(go)
                          + " liveOverrides=" + (mods == null ? 0 : mods.Length));

            if (mods != null && mods.Length > 0)
            {
                var paths = mods.Select(m => m.propertyPath).Distinct().OrderBy(p => p).ToArray();
                sb.AppendLine("  livePaths=" + string.Join(",", paths));
            }
        }

        // Count the override entries already written to disk.
        if (File.Exists(scene.path))
        {
            var disk = File.ReadAllText(scene.path);
            int diskOverrides = 0;
            int idx = 0;
            while ((idx = disk.IndexOf("propertyPath:", idx, System.StringComparison.Ordinal)) >= 0)
            {
                diskOverrides++;
                idx += 13;
            }
            sb.AppendLine("diskOverrideEntries=" + diskOverrides);
            sb.AppendLine("diskWriteTime=" + File.GetLastWriteTime(scene.path));
        }

        return sb.ToString();
    }
}
