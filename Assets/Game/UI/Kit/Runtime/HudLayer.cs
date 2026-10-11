using UnityEngine;

namespace Guildbound.UI
{
    // One per HUD layer root (L0-L7). Alpha = master x layer request, never below the layer floor:
    // plates are baked at 0.80 alpha, so floor 0.875 keeps every plate behind text at >= 0.70 effective alpha;
    // glyph floor 0.85; critical alerts 0.90. Toggling hides the layer entirely (that is a choice, not a fade).
    [RequireComponent(typeof(CanvasGroup))]
    public class HudLayer : MonoBehaviour
    {
        public int layer;
        [Range(0, 1)] public float floor;
        public bool visible = true;
        CanvasGroup group;

        void LateUpdate() => Apply();

        public void Apply()
        {
            if (!group) group = GetComponent<CanvasGroup>();
            float requested = HudSettings.Master * HudSettings.LayerRequest(layer);
            group.alpha = visible ? Mathf.Max(requested, floor) : 0f;
            group.blocksRaycasts = visible && layer == 7;
            group.interactable = visible;
        }
    }
}
