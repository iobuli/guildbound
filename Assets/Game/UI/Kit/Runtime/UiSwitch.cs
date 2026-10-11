using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // ON/OFF switch drawn from toggle_track + toggle_knob: slides the knob and tints it from the Toggle state.
    [RequireComponent(typeof(Toggle))]
    public class UiSwitch : MonoBehaviour
    {
        public RectTransform knob;
        public Image knobImage;
        public TMP_Text label;
        public float travel = 18f;
        public Color on = new Color(0.37f, 0.84f, 1f), off = new Color(0.45f, 0.5f, 0.6f);
        Toggle toggle;
        float x;

        void Update() => Refresh(HudSettings.ReducedMotion);

        public void Refresh(bool snap)
        {
            if (!toggle) toggle = GetComponent<Toggle>();
            float target = toggle.isOn ? travel : -travel;
            x = snap ? target : Mathf.MoveTowards(x, target, 160f * Time.unscaledDeltaTime);
            knob.anchoredPosition = new Vector2(x, 0f);
            knobImage.color = toggle.isOn ? on : off;
            if (label) label.text = toggle.isOn ? "ON" : "OFF";
        }
    }
}
