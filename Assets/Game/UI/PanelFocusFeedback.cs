using UnityEngine;
using UnityEngine.EventSystems;

namespace Guildbound.UI
{
    [RequireComponent(typeof(PanelSurface))]
    public sealed class PanelFocusFeedback : MonoBehaviour,
        IPointerEnterHandler, IPointerExitHandler, ISelectHandler, IDeselectHandler
    {
        PanelSurface surface;
        bool pointerInside;
        bool selected;

        void Awake() => surface = GetComponent<PanelSurface>();
        public void Configure(PanelSurface owner) => surface = owner;
        void Refresh() => surface.SetAccentVisible(pointerInside || selected);
        public void OnPointerEnter(PointerEventData data) { pointerInside = true; Refresh(); }
        public void OnPointerExit(PointerEventData data) { pointerInside = false; Refresh(); }
        public void OnSelect(BaseEventData data) { selected = true; Refresh(); }
        public void OnDeselect(BaseEventData data) { selected = false; Refresh(); }
        void OnDisable() { pointerInside = selected = false; surface.SetAccentVisible(false); }
    }
}
