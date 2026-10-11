using System;
using UnityEngine;

namespace Guildbound.UI
{
    // Player HUD options (GDD AMEND-UI-01). Opacity values are requests; HudLayer clamps them to the layer floors.
    public static class HudSettings
    {
        public static event Action Changed;

        public static float Master { get => Get("hud.master", 1f); set => Set("hud.master", value); }
        public static float PanelOpacity { get => Get("hud.l1", 1f); set => Set("hud.l1", value); }   // L1
        public static float AccentOpacity { get => Get("hud.l0", 1f); set => Set("hud.l0", value); }  // L0
        public static float PromptOpacity { get => Get("hud.l5", 1f); set => Set("hud.l5", value); }  // L5
        public static float TextScale { get => Get("hud.text", 0.5f); set => Set("hud.text", value); }
        public static bool ReducedMotion { get => Get("hud.reducedMotion", 0f) > 0.5f; set => Set("hud.reducedMotion", value ? 1f : 0f); }
        public static bool EffectsOnHud { get => Get("hud.effects", 1f) > 0.5f; set => Set("hud.effects", value ? 1f : 0f); }
        public static bool CameraShake { get => Get("hud.shake", 1f) > 0.5f; set => Set("hud.shake", value ? 1f : 0f); }

        public static float LayerRequest(int layer) => layer switch
        {
            0 => AccentOpacity,
            1 => PanelOpacity,
            5 => PromptOpacity,
            _ => 1f,
        };

        static float Get(string key, float fallback) => PlayerPrefs.GetFloat(key, fallback);

        static void Set(string key, float value)
        {
            PlayerPrefs.SetFloat(key, Mathf.Clamp01(value));
            Changed?.Invoke();
        }
    }
}
