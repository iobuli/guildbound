using TMPro;
using UnityEditor;
using UnityEngine;
using UnityEngine.UI;

namespace Guildbound.UI.Editor
{
    public static class InventoryPanelBuilder
    {
        public static InventoryView Build(RectTransform parent)
        {
            var page = Rect(parent, "Inventory", 0, 0, 1120, 700);
            var view = page.gameObject.AddComponent<InventoryView>();
            view.iconIds = new[] { "objective", "preparation", "equipment", "keepsake" };
            view.iconSprites = new[] {
                AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Game/UI/Inventory/Icons/item-4.png"),
                AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Game/UI/Inventory/Icons/item-0.png"),
                AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Game/UI/Kit/Sprites/ornament_diamond.png"),
                AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Game/UI/Kit/Icons/trust_crystal.png")
            };
            view.capacity = Text(page, "Capacity", "GUILD SATCHEL", 44, 92, 540, 36, 20);
            view.slots = new Button[12]; view.itemIcons = new Image[12];
            view.counts = new TMP_Text[12]; view.equippedMarks = new GameObject[12];
            for (int i = 0; i < 12; i++)
            {
                var slot = Rect(page, "Slot " + (i + 1), 44 + i % 4 * 136, 138 + i / 4 * 160, 118, 138);
                Frame(slot, "card_frame", 1.5f);
                view.slots[i] = Control(slot);
                var art = Rect(slot, "Item art", 21, 20, 76, 76).gameObject.AddComponent<Image>();
                art.preserveAspect = true; art.raycastTarget = false;
                view.itemIcons[i] = art;
                view.counts[i] = Text(slot, "Count", "EMPTY", 14, 104, 90, 26, 18);
                view.counts[i].alignment = TextAlignmentOptions.Center;
                view.equippedMarks[i] = Text(slot, "Equipped", "ON", 8, 8, 38, 22, 14).gameObject;
                var glow = Frame(slot, "focus_frame", 1.8f, false);
                glow.color = new Color(.42f, .87f, 1f, 0);
                slot.gameObject.AddComponent<UiSelectGlow>().glow = glow;
            }
            var details = Rect(page, "Item details", 618, 108, 456, 544);
            Frame(details, "card_frame", 1.2f);
            view.detailIcon = Rect(details, "Item art", 148, 22, 160, 160).gameObject.AddComponent<Image>();
            view.detailIcon.preserveAspect = true; view.detailIcon.raycastTarget = false;
            view.detailTitle = Text(details, "Name", "", 32, 202, 392, 64, 26);
            view.detailCategory = Text(details, "Category", "", 32, 266, 392, 30, 17);
            view.detailDescription = Text(details, "Description", "", 32, 304, 392, 120, 19);
            var action = Rect(details, "Action", 32, 436, 392, 56);
            Frame(action, "focus_frame", 1.8f);
            view.action = Control(action);
            view.actionLabel = Text(action, "Label", "USE", 12, 8, 368, 40, 20);
            view.actionLabel.alignment = TextAlignmentOptions.Center;
            view.actionReason = Text(details, "Reason", "", 32, 500, 392, 28, 17);
            view.actionReason.alignment = TextAlignmentOptions.Center;
            Text(page, "Hint", "SELECT A SLOT  /  I OR ESC TO CLOSE", 44, 628, 540, 32, 18);
            view.crystalLight = Frame(details, "focus_frame", 1.2f, false);
            view.crystalLight.color = new Color(.42f, .87f, 1f, .16f);
            return view;
        }

        static RectTransform Rect(Transform parent, string name, float x, float y, float w, float h)
        {
            var rt = new GameObject(name, typeof(RectTransform)).GetComponent<RectTransform>();
            rt.SetParent(parent, false);
            rt.anchorMin = rt.anchorMax = rt.pivot = new Vector2(0, 1);
            rt.anchoredPosition = new Vector2(x, -y); rt.sizeDelta = new Vector2(w, h);
            return rt;
        }

        static Image Frame(RectTransform parent, string id, float multiplier, bool backing = true)
        {
            if (backing)
            {
                var plate = Rect(parent, "Backplate", 8, 8, parent.sizeDelta.x - 16, parent.sizeDelta.y - 16).gameObject.AddComponent<Image>();
                plate.color = new Color(.025f, .055f, .13f, .96f); plate.raycastTarget = false;
            }
            var image = Rect(parent, "Crystal frame", 0, 0, parent.sizeDelta.x, parent.sizeDelta.y).gameObject.AddComponent<Image>();
            image.sprite = AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Game/UI/PanelKit/Sprites/" + id + ".asset");
            image.type = Image.Type.Sliced; image.fillCenter = false;
            image.pixelsPerUnitMultiplier = multiplier * .65f; image.raycastTarget = false;
            return image;
        }

        static Button Control(RectTransform parent)
        {
            var hit = Rect(parent, "Control hit area", 0, 0, parent.sizeDelta.x, parent.sizeDelta.y).gameObject.AddComponent<Image>();
            hit.color = Color.clear; hit.raycastTarget = true;
            var button = parent.gameObject.AddComponent<Button>();
            button.targetGraphic = parent.Find("Crystal frame").GetComponent<Image>();
            var colors = button.colors;
            colors.highlightedColor = new Color(.65f, .95f, 1f);
            colors.selectedColor = new Color(.65f, .95f, 1f);
            colors.disabledColor = new Color(.5f, .6f, .7f, .7f);
            button.colors = colors;
            return button;
        }

        static TMP_Text Text(Transform parent, string name, string value, float x, float y, float w, float h, float size)
        {
            var text = Rect(parent, name, x, y, w, h).gameObject.AddComponent<TextMeshProUGUI>();
            text.font = TMP_Settings.defaultFontAsset; text.text = value; text.fontSize = size;
            text.color = new Color(.918f, .965f, 1f); text.raycastTarget = false;
            text.textWrappingMode = TextWrappingModes.Normal; text.overflowMode = TextOverflowModes.Ellipsis;
            return text;
        }
    }
}
