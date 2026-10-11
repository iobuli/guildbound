using System.Text;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;

// A base layer at weight 0 plays nothing at all, so verify the generated controller's layer weight and
// correct it. Reports before/after so the result is evidence rather than an assumption.
public static class FixHeroAnimatorLayer
{
    public static string Execute()
    {
        const string path = "Assets/Game/Art/Animation/HeroAnimator.controller";
        var c = AssetDatabase.LoadAssetAtPath<AnimatorController>(path);
        if (c == null) return "MISSING " + path;

        var sb = new StringBuilder();
        var layers = c.layers;
        for (int i = 0; i < layers.Length; i++)
        {
            var sm = layers[i].stateMachine;
            sb.AppendLine(string.Format("layer[{0}] '{1}' defaultWeight={2} sm={3} states={4}",
                i, layers[i].name, layers[i].defaultWeight, sm != null ? sm.name : "null", sm != null ? sm.states.Length : -1));

            if (layers[i].defaultWeight <= 0f)
            {
                layers[i].defaultWeight = 1f;
                sb.AppendLine("  -> was 0, set to 1");
            }
        }
        c.layers = layers;
        EditorUtility.SetDirty(c);
        AssetDatabase.SaveAssets();

        var after = AssetDatabase.LoadAssetAtPath<AnimatorController>(path).layers;
        sb.AppendLine(string.Format("AFTER: '{0}' defaultWeight={1}", after[0].name, after[0].defaultWeight));
        return sb.ToString();
    }
}
