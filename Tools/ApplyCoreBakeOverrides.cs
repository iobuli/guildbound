using Guildbound.Anim.Editor;
using UnityEditor;

public static class ApplyCoreBakeOverrides
{
    public static string Execute()
    {
        if (EditorApplication.isPlaying) throw new System.InvalidOperationException("Stop Play before reimporting animation.");
        CombatClipImports.ReimportCoreClips();
        return "Six core animation imports refreshed under the durable bake overrides.";
    }
}
