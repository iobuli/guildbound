using UnityEngine;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // Presentation only. Content is a separate live RectTransform, never baked art.
    public sealed class PanelSurface : MonoBehaviour
    {
        [SerializeField] Image backplate;
        [SerializeField] Image frame;
        [SerializeField] Image accent;
        [SerializeField] RectTransform content;

        public RectTransform Content => content;
        public bool AccentVisible => accent.enabled;
        public float BackplateOpacity => backplate.color.a;

        public void Configure(Image backing, Image border, Image highlight, RectTransform host)
        {
            backplate = backing;
            frame = border;
            accent = highlight;
            content = host;
            SetBackplateOpacity(.85f);
            SetAccentVisible(false);
        }

        public void SetBackplateOpacity(float value)
        {
            var colour = backplate.color;
            colour.a = Mathf.Clamp(value, .70f, 1f);
            backplate.color = colour;
        }

        public void SetDecorationOpacity(float value)
        {
            var colour = frame.color;
            colour.a = Mathf.Clamp01(value);
            frame.color = colour;
        }

        public void SetAccentVisible(bool visible) => accent.enabled = visible;

        // Fixture action only; integration replaces this with the owning system's intent.
        public void TogglePreviewAccent() => SetAccentVisible(!AccentVisible);
    }
}
