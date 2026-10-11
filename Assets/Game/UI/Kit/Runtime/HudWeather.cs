using System;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Guildbound.UI
{
    // A1 - HUD weather. Density follows game state instead of a slider the player has to manage: TOWN parks the
    // combat panel and leaves the objective surface open, APPROACH expands it, FIGHT fades everything except what
    // a fight is actually read from (HP, guard, fire, alerts). The centre third stays empty in all three.
    //
    // The weather multiplies INTO the layer stack rather than replacing it. Each surface is a CanvasGroup on the
    // cluster nodes that draw it, so a surface fades as a whole (plate + glyph + text) and its effective alpha is
    // surface x layer. The layer floors in HudLayer are untouched, so whatever stays on screen stays legible, and a
    // surface the weather parks is a deliberate, visible decision rather than a floor breach.
    //
    // Surfaces are per cluster and not per layer because they share layers: the ledger and the combat panel both
    // draw on L1, so collapsing the ledger by fading L1 would dim HP with it.
    //
    // Phase source: today HudView feeds InCombat as 0 or 1 and this crossfades between the poles. When
    // IHudModel.Engagement01 lands that one expression becomes the value and the crossfade turns continuous.
    public class HudWeather : MonoBehaviour
    {
        [Serializable]
        public class Surface
        {
            public string id;
            public float town = 1f, approach = 1f, fight = 1f;
            public CanvasGroup[] groups = Array.Empty<CanvasGroup>();
        }

        [Header("Weather (A1)")]
        public Surface[] surfaces = Array.Empty<Surface>();
        [Tooltip("Seconds to crossfade between phases. Reduced motion snaps instead of blending.")]
        public float blendSeconds = 0.45f;
        [Tooltip("Hold to pin the weather where it stands, so it cannot reshuffle under a player who is reading it.")]
        public Key holdToPinKey = Key.LeftAlt;

        public float Engagement01 { get; private set; }
        public float Target01 { get; private set; }
        public bool Frozen { get; private set; }

        // 0 town, 1 approach, 2 fight.
        public int Phase => Engagement01 < 0.25f ? 0 : Engagement01 < 0.75f ? 1 : 2;

        // Read every frame by HudView. Reduced motion snaps: the phase change is the same code path with the
        // interpolation disabled, which is what makes it a first-class path and not an exception.
        public void Tick(float engagement01)
        {
            Target01 = Mathf.Clamp01(engagement01);
            Frozen = holdToPinKey != Key.None && Keyboard.current != null && Keyboard.current[holdToPinKey].isPressed;
            if (!Frozen)
            {
                if (HudSettings.ReducedMotion) Engagement01 = Target01;
                else Engagement01 = Mathf.MoveTowards(Engagement01, Target01, Time.unscaledDeltaTime / Mathf.Max(blendSeconds, 1e-3f));
            }
            Apply();
        }

        // Jump to the target without blending: the build captures and tests have no frame budget for a crossfade
        // and the shot has to show the settled phase.
        public void Snap()
        {
            Frozen = false;
            Engagement01 = Target01;
            Apply();
        }

        public void SetEngagement(float engagement01)
        {
            Engagement01 = Target01 = Mathf.Clamp01(engagement01);
            Apply();
        }

        public void Apply()
        {
            if (surfaces == null) return;
            for (int i = 0; i < surfaces.Length; i++)
            {
                var s = surfaces[i];
                if (s == null || s.groups == null) continue;
                float w = HudSettings.WeatherPinned(s.id) ? 1f : Weight(s);
                for (int g = 0; g < s.groups.Length; g++) if (s.groups[g]) s.groups[g].alpha = w;
            }
        }

        // 0 -> .5 town to approach, .5 -> 1 approach to fight: two straight segments, poles authored per surface.
        float Weight(Surface s) => Engagement01 <= 0.5f
            ? Mathf.Lerp(s.town, s.approach, Engagement01 * 2f)
            : Mathf.Lerp(s.approach, s.fight, (Engagement01 - 0.5f) * 2f);
    }
}
