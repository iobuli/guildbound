using UnityEngine;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // Scripted glow/pulse on any Graphic (alpha) or its transform (scale). Holds the max under reduced motion.
    public class UiPulse : MonoBehaviour
    {
        public enum Mode { Alpha, Scale }
        public Mode mode = Mode.Alpha;
        public float speed = 2f;
        public float min = 0.35f, max = 1f;
        public bool running = true;
        Graphic graphic;
        Color baseColor;

        void Awake()
        {
            graphic = GetComponent<Graphic>();
            if (graphic) baseColor = graphic.color;
        }

        void Update()
        {
            float t = running && !HudSettings.ReducedMotion && HudSettings.EffectsOnHud
                ? Mathf.Lerp(min, max, 0.5f + 0.5f * Mathf.Sin(Time.unscaledTime * speed * Mathf.PI * 2f))
                : max;
            if (mode == Mode.Alpha && graphic) graphic.color = new Color(baseColor.r, baseColor.g, baseColor.b, baseColor.a * t);
            else if (mode == Mode.Scale) transform.localScale = Vector3.one * t;
        }
    }
}
