using System.Text;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

// READ-ONLY probe. Reports the active scene's dirty state, its roots, any prefab
// override paths, and renderer material / world bounds. Saves nothing.
public static class SceneStateProbe
{
    public static string Execute()
    {
        var sb = new StringBuilder();
        var scene = EditorSceneManager.GetActiveScene();

        sb.AppendLine("scene=" + scene.name + " isDirty=" + scene.isDirty + " roots=" + scene.rootCount);

        foreach (var go in scene.GetRootGameObjects())
        {
            var mods = PrefabUtility.GetPropertyModifications(go);
            sb.AppendLine("root=" + go.name
                          + " prefab=" + PrefabUtility.GetPrefabInstanceStatus(go)
                          + " overrides=" + (mods == null ? 0 : mods.Length));

            var r = go.GetComponent<Renderer>();
            if (r != null)
            {
                var b = r.bounds;
                sb.Append("  renderer bounds=" + b.size
                          + " pos=" + go.transform.position
                          + " scale=" + go.transform.localScale
                          + " mats=");
                foreach (var m in r.sharedMaterials)
                    sb.Append((m == null ? "NULL" : m.name) + ";");
                sb.AppendLine();
            }

            foreach (var c in go.GetComponents<Component>())
                sb.AppendLine("  comp=" + (c == null ? "MISSING" : c.GetType().Name));
        }

        return sb.ToString();
    }
}
