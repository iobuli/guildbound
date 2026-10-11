using UnityEngine;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // Guard capacity as discrete segments; a break turns them amber and shakes the row once.
    public class SegmentRow : MonoBehaviour
    {
        public Image[] segments;
        public Color full = new Color(0.37f, 0.84f, 1f), empty = new Color(0.12f, 0.2f, 0.32f), broken = new Color(1f, 0.7f, 0.28f);
        float shake;
        bool wasBroken;
        Vector2 home;
        RectTransform rect;

        void Awake()
        {
            rect = (RectTransform)transform;
            home = rect.anchoredPosition;
        }

        public void Set(int current, int max, bool isBroken)
        {
            for (int i = 0; i < segments.Length; i++)
            {
                segments[i].gameObject.SetActive(i < max);
                segments[i].color = isBroken ? broken : i < current ? full : empty;
            }
            if (isBroken && !wasBroken) shake = 0.3f;
            wasBroken = isBroken;
        }

        void Update()
        {
            if (shake <= 0f || HudSettings.ReducedMotion) { rect.anchoredPosition = home; shake = 0f; return; }
            shake -= Time.unscaledDeltaTime;
            rect.anchoredPosition = home + new Vector2(Mathf.Sin(Time.unscaledTime * 90f) * 4f * shake / 0.3f, 0f);
        }
    }
}
