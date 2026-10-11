using System.Text;
using UnityEditor;
using UnityEngine;
using UnityEngine.SceneManagement;

/// <summary>
/// Invocation shim for Aura, matching the project's existing Tools/*ThroughAura pattern
/// (see HudBuildThroughAura.cs). Runs the permanent builder, then reads the scene back so the
/// result is evidence rather than an assumption.
/// </summary>
public static class SparringBuildThroughAura
{
    public static string Execute()
    {
        var report = Guildbound.Editor.SparringSceneBuilder.Build();

        var scene = SceneManager.GetActiveScene();
        var roots = scene.GetRootGameObjects();

        var sb = new StringBuilder(report);
        sb.Append("\nACTIVE SCENE: ").Append(scene.path).Append("  roots=").Append(roots.Length);
        foreach (var r in roots)
            sb.Append("\n  - ").Append(r.name).Append("  children=").Append(r.transform.childCount);

        return sb.ToString();
    }
}
