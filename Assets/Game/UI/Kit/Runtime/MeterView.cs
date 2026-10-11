using UnityEngine;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // Horizontal meter: fill follows the value at once; the trail holds, then drains, so a loss stays readable.
    public class MeterView : MonoBehaviour
    {
        public Image fill, trail;
        public float trailDelay = 0.35f, trailSpeed = 0.6f;
        public Color normal = new Color(1f, 0.29f, 0.35f), low = new Color(1f, 0.29f, 0.35f);
        public float lowBelow = 0.25f;
        float shown = 1f, hold;

        public void Set(float value01)
        {
            value01 = Mathf.Clamp01(value01);
            if (value01 < fill.fillAmount) hold = trailDelay;
            fill.fillAmount = value01;
            fill.color = value01 <= lowBelow ? low : normal;
        }

        void Update()
        {
            if (!trail) return;
            if (hold > 0f) hold -= Time.unscaledDeltaTime;
            else shown = Mathf.MoveTowards(shown, fill.fillAmount, trailSpeed * Time.unscaledDeltaTime);
            if (shown < fill.fillAmount || HudSettings.ReducedMotion) shown = fill.fillAmount;
            trail.fillAmount = shown;
        }
    }
}
