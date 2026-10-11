using UnityEditor;

public static class RefreshAnimationSources
{
    public static string Execute()
    {
        AssetDatabase.Refresh(ImportAssetOptions.ForceUpdate);
        return "Animation source refresh requested.";
    }
}
