using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // Fades a glow Image in while the control is selected (keyboard/gamepad) or hovered (mouse).
    public class UiSelectGlow : MonoBehaviour, ISelectHandler, IDeselectHandler, IPointerEnterHandler, IPointerExitHandler
    {
        public Image glow;
        public float fadeSpeed = 8f;
        bool selected, hovered;

        public void OnSelect(BaseEventData e) => selected = true;
        public void OnDeselect(BaseEventData e) => selected = false;
        public void OnPointerEnter(PointerEventData e) => hovered = true;
        public void OnPointerExit(PointerEventData e) => hovered = false;

        void Update()
        {
            if (!glow) return;
            float target = selected || hovered ? 1f : 0f;
            var c = glow.color;
            c.a = HudSettings.ReducedMotion ? target : Mathf.MoveTowards(c.a, target, fadeSpeed * Time.unscaledDeltaTime);
            glow.color = c;
        }
    }
}
