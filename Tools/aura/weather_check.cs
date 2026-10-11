using System.Text;
using Guildbound.UI;
using UnityEditor;
using UnityEngine;

// Throwaway in-editor check for the A1 weather wiring. Lives outside Assets/ so Unity does not import it.
public static class WeatherCheck
{
    public static object Execute()
    {
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>("Assets/Game/UI/Kit/Hud.prefab");
        if (prefab == null) return "MISSING PREFAB Assets/Game/UI/Kit/Hud.prefab";

        var go = Object.Instantiate(prefab);
        try
        {
            var view = go.GetComponent<HudView>();
            var weather = go.GetComponent<HudWeather>();
            if (view == null) return "NO HudView ON ROOT";
            if (weather == null) return "NO HudWeather ON ROOT";
            if (!view.weather) return "HudView.weather NOT WIRED";

            var sb = new StringBuilder();
            sb.AppendLine("surfaces=" + weather.surfaces.Length + " wired(view.weather)=" + (view.weather == weather));
            foreach (var s in weather.surfaces)
            {
                sb.Append(s.id.PadRight(10) + " groups=" + s.groups.Length + " authored=" + s.town + "/" + s.approach + "/" + s.fight);
                weather.SetEngagement(0f);
                sb.Append("  town=" + Alpha(s));
                weather.SetEngagement(0.5f);
                sb.Append(" approach=" + Alpha(s));
                weather.SetEngagement(1f);
                sb.AppendLine(" fight=" + Alpha(s));
            }
            weather.SetEngagement(1f);
            sb.AppendLine("phase@1=" + weather.Phase);
            weather.SetEngagement(0f);
            sb.AppendLine("phase@0=" + weather.Phase);
            return sb.ToString();
        }
        finally
        {
            Object.DestroyImmediate(go);
        }
    }

    static string Alpha(HudWeather.Surface s)
    {
        var sb = new StringBuilder();
        for (int i = 0; i < s.groups.Length; i++) sb.Append(s.groups[i] ? s.groups[i].alpha.ToString("0.00") + " " : "NULL ");
        return sb.ToString();
    }
}
