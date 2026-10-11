using Guildbound.Contracts;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // One page under the production HUD's L7. Reads slots and forwards requests to the rules model.
    public sealed class InventoryView : MonoBehaviour
    {
        public Button[] slots;
        public Image[] itemIcons;
        public TMP_Text[] counts;
        public GameObject[] equippedMarks;
        public string[] iconIds;
        public Sprite[] iconSprites;
        public TMP_Text capacity, detailTitle, detailCategory, detailDescription, actionLabel, actionReason;
        public Image detailIcon, crystalLight;
        public Button action;
        IInventoryModel model;
        int selected;

        public void Bind(IInventoryModel source)
        {
            if (model != null) model.Changed -= Refresh;
            model = source;
            for (int i = 0; i < slots.Length; i++)
            {
                int index = i;
                slots[i].onClick.RemoveAllListeners();
                slots[i].onClick.AddListener(() => Select(index));
            }
            action.onClick.RemoveAllListeners();
            action.onClick.AddListener(RequestAction);
            model.Changed += Refresh;
            Refresh();
        }

        void OnDestroy()
        {
            if (model != null) model.Changed -= Refresh;
        }

        void Update()
        {
            float alpha = !HudSettings.EffectsOnHud ? 0f : HudSettings.ReducedMotion ? .16f
                : .12f + .06f * Mathf.Sin(Time.unscaledTime * 1.5f);
            var colour = crystalLight.color;
            colour.a = alpha;
            crystalLight.color = colour;
        }

        public void Refresh()
        {
            int occupied = 0;
            for (int i = 0; i < slots.Length; i++)
            {
                var item = i < model.SlotCount ? model.GetSlot(i) : default;
                bool has = !item.IsEmpty;
                slots[i].interactable = has;
                itemIcons[i].gameObject.SetActive(has);
                itemIcons[i].sprite = has ? Icon(item.IconId) : null;
                counts[i].text = has ? item.Count.ToString() : "EMPTY";
                equippedMarks[i].SetActive(has && item.Equipped);
                if (has) occupied++;
            }
            capacity.text = occupied + " / " + slots.Length + " SLOTS  /  GUILD SATCHEL";
            if (selected >= model.SlotCount || model.GetSlot(selected).IsEmpty)
            {
                selected = 0;
                for (int i = 0; i < model.SlotCount; i++)
                    if (!model.GetSlot(i).IsEmpty) { selected = i; break; }
            }
            ShowDetails();
        }

        public void Select(int index)
        {
            selected = index;
            ShowDetails();
        }

        public void FocusFirst()
        {
            for (int i = 0; i < slots.Length; i++)
                if (slots[i].interactable)
                {
                    Select(i);
                    // Edit-mode builder captures have no active EventSystem; runtime navigation does.
                    if (EventSystem.current) EventSystem.current.SetSelectedGameObject(slots[i].gameObject);
                    return;
                }
        }

        void ShowDetails()
        {
            var item = model.GetSlot(selected);
            detailIcon.sprite = Icon(item.IconId);
            detailIcon.gameObject.SetActive(!item.IsEmpty);
            detailTitle.text = item.IsEmpty ? "Empty satchel" : item.Name;
            detailCategory.text = item.IsEmpty ? "" : item.Category.ToString().ToUpperInvariant();
            detailDescription.text = item.Description ?? "";
            bool equipment = item.Category == ItemCategory.Equipment;
            action.interactable = equipment ? model.CanEquip(selected, out var reason) : model.CanUse(selected, out reason);
            actionLabel.text = equipment ? item.Equipped ? "UNEQUIP" : "EQUIP" : "USE";
            actionReason.text = action.interactable ? "APPEARANCE ONLY" : reason;
        }

        void RequestAction()
        {
            if (model.GetSlot(selected).Category == ItemCategory.Equipment)
            {
                if (model.CanEquip(selected, out _)) model.RequestEquip(selected);
            }
            else if (model.CanUse(selected, out _)) model.RequestUse(selected);
        }

        Sprite Icon(string id)
        {
            for (int i = 0; i < iconIds.Length; i++)
                if (iconIds[i] == id) return iconSprites[i];
            return null;
        }
    }
}
