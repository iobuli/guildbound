using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // Presentation sandbox. Production inventory rules/bindings are owned by Claude.
    public sealed class HudInventoryPreview : MonoBehaviour
    {
        public HudView hud;
        public HudPreviewModel model;
        public Sprite frame, cardFrame, focusFrame;
        public Sprite[] icons;
        public bool startOpen = true;
        GameObject overlay;
        TMP_Text detailTitle, detailBody, capacity;
        Image detailIcon, shimmer;
        Button first;
        bool visible;
        public bool Visible => visible;
        readonly string[] names = { "Preparation flask", "Red tonic", "Blue flask", "Guild purse", "Sealed parcel", "Fire attunement" };
        readonly string[] descriptions = {
            "Carried preparation / 2\n\nThe optional preparation item shortens out-of-combat recovery. Consumed only on completion.\n\nSelection preview; production carry rules are pending.",
            "Supplied item-art preview\n\nA red tonic shape from the approved icon library. Its gameplay use has not been defined.",
            "Supplied item-art preview\n\nA blue flask shape from the approved icon library. Its gameplay use has not been defined.",
            "Wallet / 30 coins\n\nWallet presentation belongs to the guild board. These sample coins are not awarded by this inventory view.",
            "Carried objective / 1\n\nReturn the parcel to the guild hall. A contract system will own delivery and the one-time reward.",
            "Technique record\n\nFire readiness, charge and cooldown stay distinct. This record is not an equipment bonus."
        };

        void Start()
        {
            var canvasRect = Rect("Inventory preview canvas", null, Vector2.zero, Vector2.zero);
            var canvas = canvasRect.gameObject.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = 100;
            var scaler = canvasRect.gameObject.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1920, 1080);
            scaler.matchWidthOrHeight = .5f;
            canvasRect.gameObject.AddComponent<GraphicRaycaster>();
            overlay = canvasRect.gameObject;
            var dim = ImageChild("World dim", canvasRect, null, new Color(.015f, .025f, .08f, .8f));
            dim.raycastTarget = true;
            var window = Rect("Inventory", canvasRect, Vector2.zero, new Vector2(1360, 820));
            Panel(window, frame, .5f);
            Text(window, "Title", "SATCHEL / INVENTORY", new Vector2(72, 48), new Vector2(780, 58), 38);
            capacity = Text(window, "Capacity", "CARRIED ITEMS / SAMPLE", new Vector2(72, 118), new Vector2(660, 40), 23);
            var close = Button(window, "Close", new Vector2(1100, 56), new Vector2(184, 64), "CLOSE / I");
            close.onClick.AddListener(Hide);
            for (int index = 0; index < 12; index++)
            {
                int slot = index;
                var rect = TopRect("Slot " + (index + 1), window, new Vector2(72 + index % 4 * 158, 184 + index / 4 * 164), new Vector2(140, 146));
                Panel(rect, cardFrame, .9f);
                var hit = ImageChild("Control hit area", rect, null, Color.clear);
                hit.raycastTarget = true;
                var control = rect.gameObject.AddComponent<Button>();
                control.targetGraphic = rect.Find("Frame").GetComponent<Image>();
                if (index < icons.Length)
                {
                    var iconRect = TopRect("Item art", rect, new Vector2(28, 20), new Vector2(84, 84));
                    var icon = iconRect.gameObject.AddComponent<Image>();
                    icon.sprite = icons[index]; icon.preserveAspect = true; icon.raycastTarget = false;
                    Text(rect, "Count", index == 0 ? "2" : "1", new Vector2(88, 104), new Vector2(28, 28), 22);
                    control.onClick.AddListener(() => Select(slot));
                    if (index == 0) first = control;
                }
                else
                {
                    Text(rect, "Empty", "EMPTY", new Vector2(28, 60), new Vector2(90, 28), 18);
                    control.interactable = false;
                }
            }
            var details = TopRect("Item details", window, new Vector2(780, 184), new Vector2(500, 540));
            Panel(details, cardFrame, .8f);
            var detailArt = TopRect("Selected item art", details, new Vector2(170, 32), new Vector2(160, 160));
            detailIcon = detailArt.gameObject.AddComponent<Image>();
            detailIcon.preserveAspect = true; detailIcon.raycastTarget = false;
            detailTitle = Text(details, "Selected title", "", new Vector2(40, 206), new Vector2(420, 52), 29);
            detailBody = Text(details, "Selected description", "", new Vector2(40, 274), new Vector2(420, 230), 21);
            detailBody.textWrappingMode = TextWrappingModes.Normal;
            Text(window, "Footer", "SELECT A SLOT TO INSPECT  /  I OR ESC TO CLOSE  /  INVENTORY PREVIEW", new Vector2(72, 732), new Vector2(1208, 42), 22);
            shimmer = ImageChild("Crystal light", window, focusFrame, new Color(.42f, .87f, 1f, .16f));
            shimmer.type = Image.Type.Sliced; shimmer.fillCenter = false;
            shimmer.pixelsPerUnitMultiplier = .5f;
            Select(0);
            overlay.SetActive(false);
            if (startOpen) Show();
        }

        void Update()
        {
            var keys = Keyboard.current;
            if (keys == null) return;
            if (keys.iKey.wasPressedThisFrame) { if (visible) Hide(); else Show(); }
            else if (visible && keys.escapeKey.wasPressedThisFrame) Hide();
            if (!visible) return;
            capacity.text = "6 SAMPLE ITEMS   /   PREPARATION 2   /   WALLET " + model.wallet;
            var colour = shimmer.color;
            colour.a = !HudSettings.EffectsOnHud ? 0 : HudSettings.ReducedMotion ? .16f : .12f + .07f * Mathf.Sin(Time.unscaledTime * 1.5f);
            shimmer.color = colour;
        }
        void Show()
        {
            hud.CloseModal(); hud.enabled = false;
            visible = true; overlay.SetActive(true);
            EventSystem.current.SetSelectedGameObject(first.gameObject);
        }
        public void Hide()
        {
            visible = false; overlay.SetActive(false); hud.enabled = true;
            EventSystem.current.SetSelectedGameObject(null);
        }
        void Select(int index)
        {
            detailIcon.sprite = icons[index];
            detailTitle.text = names[index]; detailBody.text = descriptions[index];
        }
        static RectTransform Rect(string name, Transform parent, Vector2 position, Vector2 size)
        {
            var rect = new GameObject(name, typeof(RectTransform)).GetComponent<RectTransform>();
            rect.SetParent(parent, false); rect.anchoredPosition = position; rect.sizeDelta = size;
            return rect;
        }
        static RectTransform TopRect(string name, Transform parent, Vector2 position, Vector2 size)
        {
            var rect = Rect(name, parent, new Vector2(position.x, -position.y), size);
            rect.anchorMin = rect.anchorMax = rect.pivot = new Vector2(0, 1);
            return rect;
        }
        static Image ImageChild(string name, RectTransform parent, Sprite sprite, Color colour)
        {
            var rect = Rect(name, parent, Vector2.zero, Vector2.zero);
            rect.anchorMin = Vector2.zero; rect.anchorMax = Vector2.one;
            rect.offsetMin = rect.offsetMax = Vector2.zero;
            var image = rect.gameObject.AddComponent<Image>(); image.sprite = sprite;
            image.color = colour; image.raycastTarget = false;
            return image;
        }
        static void Panel(RectTransform parent, Sprite sprite, float cornerMultiplier)
        {
            var plate = ImageChild("Backplate", parent, null, new Color(.025f, .055f, .13f, .96f));
            plate.rectTransform.offsetMin = new Vector2(12, 12); plate.rectTransform.offsetMax = new Vector2(-12, -12);
            var art = ImageChild("Frame", parent, sprite, Color.white);
            art.type = Image.Type.Sliced; art.fillCenter = false; art.pixelsPerUnitMultiplier = cornerMultiplier;
        }
        Button Button(RectTransform parent, string name, Vector2 position, Vector2 size, string label)
        {
            var rect = TopRect(name, parent, position, size); Panel(rect, focusFrame, 1.2f);
            var hit = ImageChild("Control hit area", rect, null, Color.clear); hit.raycastTarget = true;
            var control = rect.gameObject.AddComponent<Button>();
            control.targetGraphic = rect.Find("Frame").GetComponent<Image>();
            Text(rect, "Label", label, new Vector2(18, 18), size - new Vector2(36, 36), 20);
            return control;
        }
        static TMP_Text Text(RectTransform parent, string name, string value, Vector2 position, Vector2 size, float fontSize)
        {
            var rect = TopRect(name, parent, position, size);
            var text = rect.gameObject.AddComponent<TextMeshProUGUI>();
            text.font = TMP_Settings.defaultFontAsset; text.text = value;
            text.fontSize = fontSize; text.color = new Color(.88f, .97f, 1f);
            text.raycastTarget = false;
            return text;
        }
    }
}
