using System.Collections.Generic;
using System.IO;
using Guildbound.Contracts;
using TMPro;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem.UI;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using UnityEngine.UI;

namespace Guildbound.UI.Editor
{
    // Builds the layered HUD prefab from the frame kit at exact 1920x1080 reference positions, a preview scene,
    // and captures (combat / board / options) to Documentation/build/hud/. Every control is a real uGUI
    // Selectable; effects are scripts. Layout numbers below are the contract for where content goes.
    public static class HudBuilder
    {
        const string Kit = "Assets/Game/UI/Kit/";
        const string PrefabPath = Kit + "Hud.prefab";
        const string ScenePath = "Assets/Game/Scenes/HudPreview.unity";
        static readonly Color TextColor = new Color(0.918f, 0.965f, 1f);
        static readonly Color Caps = new Color(0.62f, 0.87f, 1f);
        static readonly Color Edge = new Color(0.37f, 0.84f, 1f);
        static readonly Color Fire = new Color(1f, 0.62f, 0.22f);
        static readonly float[] Floors = { 0f, 0.875f, 0.85f, 0.85f, 0.85f, 0.875f, 0.9f, 1f };
        static readonly string[] LayerNames = { "L0 Accents", "L1 Plates", "L2 Meters", "L3 Icons", "L4 Text", "L5 Prompts", "L6 Alerts", "L7 Modal" };

        static Transform[] layers;
        static readonly Dictionary<string, RectTransform> clusters = new();

        [MenuItem("Guildbound/UI/Build HUD")]
        public static void Build()
        {
            if (Resources.Load<TMP_Settings>("TMP Settings") == null)
            {
                TMP_PackageResourceImporter.ImportResources(true, false, false);
                AssetDatabase.Refresh();
                Debug.Log("GUILDBOUND_HUD imported TMP essentials; run Build HUD again");
                return;
            }
            var root = BuildHud();
            PrefabUtility.SaveAsPrefabAsset(root.gameObject, PrefabPath);
            Object.DestroyImmediate(root.gameObject);
            BuildScene();
            Debug.Log("GUILDBOUND_HUD_BUILT " + PrefabPath + " " + ScenePath);
        }

        // ---------- layout ----------
        static RectTransform BuildHud()
        {
            clusters.Clear();
            var canvasGo = new GameObject("Hud", typeof(RectTransform), typeof(Canvas), typeof(CanvasScaler), typeof(GraphicRaycaster));
            var canvas = canvasGo.GetComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = 10;
            var scaler = canvasGo.GetComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1920, 1080);
            scaler.matchWidthOrHeight = 0.5f;
            var rootRt = (RectTransform)canvasGo.transform;

            layers = new Transform[8];
            var hudLayers = new HudLayer[8];
            for (int i = 0; i < 8; i++)
            {
                var l = Stretch(rootRt, LayerNames[i]);
                l.gameObject.AddComponent<CanvasGroup>();
                hudLayers[i] = l.gameObject.AddComponent<HudLayer>();
                hudLayers[i].layer = i;
                hudLayers[i].floor = Floors[i];
                layers[i] = l;
            }
            var view = canvasGo.AddComponent<HudView>();
            view.modalLayer = hudLayers[7];

            // L0 vignette (scripted low-HP pulse)
            view.vignette = Img(Stretch(layers[0], "Vignette"), "vignette", new Color(0, 0, 0, 0.35f));
            view.vignette.raycastTarget = false;

            // COMBAT cluster: top-left, 600 x 264 at (32, 32)
            var tl = new Vector2(0, 1);
            Img(Stretch(C(1, "Combat", tl, new Vector2(32, -32), new Vector2(600, 264)), "Plate"), PlateFor("panel_frame"));
            Art(C(1, "Combat"), "status_frame");
            Img(R(C(1, "Combat"), "PortraitSlot", 20, 18, 120, 120), "portrait_slot");
            Img(R(C(3, "Combat", tl, new Vector2(32, -32), new Vector2(600, 264)), "PortraitMark", 68, 66, 24, 24), "ornament_diamond");
            T(C(4, "Combat", tl, new Vector2(32, -32), new Vector2(600, 264)), "HpLabel", "HP", 18, 156, 26, 64, 30, TextAlignmentOptions.Left, Caps, true);
            Img(R(C(1, "Combat"), "HpFrame", 250, 26, 326, 28), "bar_frame");
            var hpTrail = Filled(R(C(2, "Combat", tl, new Vector2(32, -32), new Vector2(600, 264)), "HpTrail", 255, 31, 316, 18), "fill_mask", new Color(1f, 0.85f, 0.85f, 0.55f), Image.FillMethod.Horizontal);
            var hpFill = Filled(R(C(2, "Combat"), "HpFill", 255, 31, 316, 18), "fill_mask", new Color(1f, 0.29f, 0.35f), Image.FillMethod.Horizontal);
            view.hp = hpFill.gameObject.AddComponent<MeterView>();
            view.hp.fill = hpFill; view.hp.trail = hpTrail;
            view.hp.normal = new Color(1f, 0.36f, 0.42f); view.hp.low = new Color(1f, 0.29f, 0.35f);
            T(C(4, "Combat"), "GuardLabel", "GUARD", 18, 156, 66, 72, 30, TextAlignmentOptions.Left, Caps, true);
            var segRow = R(C(2, "Combat"), "GuardSegments", 250, 70, 326, 18);
            var segs = new Image[5];
            for (int i = 0; i < 5; i++) segs[i] = Img(R(segRow, "Seg" + i, i * 66.5f, 2, 60, 14), "fill_mask", Edge);
            view.guard = segRow.gameObject.AddComponent<SegmentRow>();
            view.guard.segments = segs;

            Img(R(C(1, "Combat"), "FireSlot", 30, 150, 100, 100), "icon_slot_diamond");
            var fireGlow = Img(R(C(0, "Combat", tl, new Vector2(32, -32), new Vector2(600, 264)), "FireGlow", 10, 130, 140, 140), "glow_blob", new Color(Fire.r, Fire.g, Fire.b, 0.55f));
            view.fireGlow = fireGlow.gameObject.AddComponent<UiPulse>();
            view.fireGlow.speed = 0.8f; view.fireGlow.min = 0.35f;
            view.chargeRing = Filled(R(C(2, "Combat"), "ChargeRing", 30, 150, 100, 100), "ring_mask", Fire, Image.FillMethod.Radial360);
            view.fireIcon = Img(R(C(3, "Combat"), "FireIcon", 44, 164, 72, 72), "Icons/fire_ready");
            view.fireIcon.preserveAspect = true;
            string[] fireIcons = { "fire_locked", "fire_ready", "fire_charging", "fire_release", "fire_cooldown", "fire_interrupted" };
            for (int i = 0; i < fireIcons.Length; i++) view.fireSprites[i] = S("Icons/" + fireIcons[i]);
            view.cooldownDisc = Filled(R(C(3, "Combat"), "CooldownSweep", 50, 170, 60, 60), "disc_mask", new Color(0.02f, 0.05f, 0.12f, 0.6f), Image.FillMethod.Radial360);
            view.fireLabel = T(C(4, "Combat"), "FireLabel", "FIRE", 22, 150, 168, 220, 30, TextAlignmentOptions.Left, Caps, true);
            view.fireState = T(C(4, "Combat"), "FireState", "READY", 28, 150, 198, 300, 36, TextAlignmentOptions.Left, Fire, true);

            // OBJECTIVE pill: 440 x 56 under the combat panel
            var objPos = new Vector2(32, -308);
            var objPlate = Img(Stretch(C(1, "Objective", tl, objPos, new Vector2(440, 56)), "Plate"), "toast_frame");
            Img(R(C(3, "Objective", tl, objPos, new Vector2(440, 56)), "Mark", 14, 16, 24, 24), "ornament_diamond");
            view.objectiveText = T(C(4, "Objective", tl, objPos, new Vector2(440, 56)), "Text", "Return the seal", 22, 50, 10, 376, 36, TextAlignmentOptions.Left, TextColor, false);
            view.objectiveRoot = objPlate.transform.parent.gameObject;
            GroupToggle(view, "Objective");

            // PROMPT (L5): bottom centre, 460 x 56, 120 px above the bottom edge (outside the centre third)
            var bc = new Vector2(0.5f, 0);
            var prompt = C(5, "Prompt", bc, new Vector2(0, 120), new Vector2(460, 56));
            Img(Stretch(prompt, "Plate"), "toast_frame");
            Img(R(prompt, "Keycap", 12, 10, 36, 36), "keycap_frame");
            view.promptKey = T(prompt, "Key", "E", 20, 12, 10, 36, 36, TextAlignmentOptions.Center, TextColor, true);
            view.promptText = T(prompt, "Text", "Open the guild board", 22, 62, 10, 380, 36, TextAlignmentOptions.Left, TextColor, false);
            view.promptRoot = prompt.gameObject;

            // RECOVER (L5): ring above the prompt
            var rec = C(5, "Recover", bc, new Vector2(0, 196), new Vector2(240, 130));
            Img(R(rec, "Track", 76, 0, 88, 88), "ring_mask", new Color(Edge.r, Edge.g, Edge.b, 0.25f));
            view.recoverRing = Filled(R(rec, "Ring", 76, 0, 88, 88), "ring_mask", Edge, Image.FillMethod.Radial360);
            T(rec, "Label", "RECOVERING", 18, 0, 94, 240, 30, TextAlignmentOptions.Center, Caps, true);
            view.recoverRoot = rec.gameObject;

            // ALERTS (L6): top-right, two 440 x 64 slots, newest on top
            var alerts = C(6, "Alerts", new Vector2(1, 1), new Vector2(-32, -32), new Vector2(440, 140));
            for (int i = 0; i < 2; i++)
            {
                var slot = R(alerts, "Alert" + i, 0, i * 76, 440, 64);
                Img(Stretch(slot, "Plate"), "toast_frame");
                view.alertAccents[i] = Img(R(slot, "Accent", 12, 12, 6, 40), "fill_mask", Edge);
                view.alertTexts[i] = T(slot, "Text", "Alert", 20, 30, 10, 396, 44, TextAlignmentOptions.Left, TextColor, false);
                view.alertSlots[i] = slot.gameObject;
                slot.gameObject.SetActive(false);
            }

            BuildModal(view);
            return rootRt;
        }

        static void BuildModal(HudView view)
        {
            var dim = Img(Stretch(layers[7], "Dim"), null, new Color(0, 0, 0, 0.6f));
            dim.raycastTarget = true;
            var win = C(7, "Window", new Vector2(0.5f, 0.5f), Vector2.zero, new Vector2(1120, 700));
            Img(Stretch(win, "Frame"), PlateFor("modal_frame"));
            Art(win, "modal_frame", 1f, 10f);
            Img(R(win, "Header", 24, 20, 1072, 52), "header_bar");
            view.windowTitle = T(win, "Title", "GUILD BOARD", 26, 60, 26, 420, 40, TextAlignmentOptions.Left, TextColor, true);

            var group = win.gameObject.AddComponent<ToggleGroup>();
            view.boardTab = Tab(win, "TabBoard", "BOARD", 690, 28, group);
            view.optionsTab = Tab(win, "TabOptions", "OPTIONS", 840, 28, group);
            view.closeButton = Btn(win, "Close", "CLOSE", 990, 28, 96, 36, 16);

            // BOARD page
            var board = Stretch(win, "Board");
            view.boardWindow = board.gameObject;
            Block(board, "Trust", 24, 88, "trust_crystal", "TRUST", out view.trustName, 24);
            Img(R(board, "TrustBarFrame", 114, 158, 400, 16), "bar_frame");
            view.trustBar = Filled(R(board, "TrustBar", 118, 162, 392, 8), "fill_mask", Edge, Image.FillMethod.Horizontal);
            Block(board, "Wallet", 576, 88, "wallet_coin", "WALLET", out view.wallet, 34);

            int n = 2;
            view.cardTitles = new TMP_Text[n]; view.cardObjectives = new TMP_Text[n]; view.cardServices = new TMP_Text[n];
            view.cardButtons = new Button[n]; view.cardButtonLabels = new TMP_Text[n];
            for (int i = 0; i < n; i++)
            {
                var card = R(board, "Card" + i, 24 + i * 548, 200, 524, 470);
                Img(Stretch(card, "Frame"), PlateFor("card_frame"));
                Art(card, "card_frame");
                Img(R(card, "Ornament", 248, -2, 28, 28), "ornament_diamond");
                var art = R(card, "ArtSlot", 22, 28, 480, 160);
                Img(Stretch(art, "Plate"), "panel_frame_plain", new Color(1, 1, 1, 0.9f));
                T(art, "Hint", "contract art 480 x 160", 14, 0, 64, 480, 30, TextAlignmentOptions.Center, new Color(Caps.r, Caps.g, Caps.b, 0.4f), false);
                view.cardTitles[i] = T(card, "Title", "Contract title", 24, 26, 200, 472, 64, TextAlignmentOptions.TopLeft, TextColor, true);
                Img(R(card, "Divider", 26, 268, 472, 12), "divider");
                view.cardObjectives[i] = T(card, "Objective", "Objective", 19, 26, 286, 472, 52, TextAlignmentOptions.TopLeft, TextColor, false);
                Img(R(card, "ServiceMark", 26, 346, 20, 20), "ornament_diamond");
                view.cardServices[i] = T(card, "Service", "Service", 19, 54, 342, 444, 52, TextAlignmentOptions.TopLeft, Caps, false);
                view.cardButtons[i] = Btn(card, "Accept", "ACCEPT", 152, 402, 220, 48, 20);
                view.cardButtonLabels[i] = view.cardButtons[i].GetComponentInChildren<TMP_Text>();
            }

            // OPTIONS page
            var options = Stretch(win, "Options");
            view.optionsWindow = options.gameObject;
            string[] sliderNames = { "Master HUD opacity", "Panel opacity (L1)", "Prompt opacity (L5)", "Accent opacity (L0)", "Text size" };
            for (int i = 0; i < sliderNames.Length; i++)
            {
                var row = Row(options, sliderNames[i], 96 + i * 68);
                view.sliders[i] = MakeSlider(row, 420, 16, 500);
                view.sliderValues[i] = T(row, "Value", "100%", 20, 940, 14, 100, 30, TextAlignmentOptions.Right, TextColor, true);
            }
            view.reducedMotion = Switch(Row(options, "Reduced motion (pulses and fades off)", 96 + 5 * 68));
            view.effectsOnHud = Switch(Row(options, "Effects on HUD", 96 + 6 * 68));
            view.cameraShake = Switch(Row(options, "Camera shake", 96 + 7 * 68));
            options.gameObject.SetActive(false);
            view.modalLayer.visible = false;
            view.modalLayer.GetComponent<CanvasGroup>().alpha = 0f;
        }

        // ---------- widgets ----------
        static void Block(RectTransform parent, string name, int x, int y, string icon, string caption, out TMP_Text value, int size)
        {
            var b = R(parent, name, x, y, 520, 100);
            Img(Stretch(b, "Plate"), "panel_frame_plain");
            Art(b, "inset_frame", 1.6f, 4f);
            var ic = Img(R(b, "Icon", 22, 22, 56, 56), "Icons/" + icon);
            ic.preserveAspect = true;
            T(b, "Caption", caption, 15, 92, 14, 300, 22, TextAlignmentOptions.Left, Caps, true);
            value = T(b, "Value", "-", size, 92, 34, 400, 40, TextAlignmentOptions.Left, TextColor, true);
        }

        static RectTransform Row(RectTransform parent, string label, int y)
        {
            var row = R(parent, label, 24, y, 1072, 60);
            Img(Stretch(row, "Plate"), "panel_frame_plain");
            T(row, "Label", label, 20, 24, 14, 380, 32, TextAlignmentOptions.Left, TextColor, false);
            return row;
        }

        static Button Btn(RectTransform parent, string name, string label, int x, int y, int w, int h, int size)
        {
            var rt = R(parent, name, x, y, w, h);
            var frame = Img(rt, "button_frame");
            var glow = Art(rt, "focus_frame", 2.6f, 6f) ?? Img(Stretch(rt, "Glow"), "button_frame_active");
            glow.color = new Color(1, 1, 1, 0);
            glow.raycastTarget = false;
            T(rt, "Label", label, size, 0, 0, w, h, TextAlignmentOptions.Center, TextColor, true);
            var b = rt.gameObject.AddComponent<Button>();
            b.targetGraphic = frame;
            b.colors = Tint();
            rt.gameObject.AddComponent<UiSelectGlow>().glow = glow;
            return b;
        }

        static Toggle Tab(RectTransform parent, string name, string label, int x, int y, ToggleGroup group)
        {
            var rt = R(parent, name, x, y, 140, 36);
            var bg = Img(rt, "tab_idle");
            var on = Img(Stretch(rt, "Active"), "tab_active");
            on.raycastTarget = false;
            T(rt, "Label", label, 16, 0, 0, 140, 36, TextAlignmentOptions.Center, TextColor, true);
            var t = rt.gameObject.AddComponent<Toggle>();
            t.targetGraphic = bg; t.graphic = on; t.group = group; t.colors = Tint();
            var glow = Img(Stretch(rt, "Glow"), "button_frame_active", new Color(1, 1, 1, 0));
            glow.raycastTarget = false;
            rt.gameObject.AddComponent<UiSelectGlow>().glow = glow;
            return t;
        }

        static Slider MakeSlider(RectTransform row, int x, int y, int w)
        {
            var rt = R(row, "Slider", x, y, w, 28);
            var bg = Img(R(rt, "Track", 0, 8, w, 12), "slider_track");
            bg.raycastTarget = true;
            var fillArea = Stretch(rt, "FillArea");
            fillArea.offsetMin = new Vector2(4, 11); fillArea.offsetMax = new Vector2(-4, -11);
            var fill = Img(Stretch(fillArea, "Fill"), "fill_mask", Edge);
            var slideArea = Stretch(rt, "HandleArea");
            slideArea.offsetMin = new Vector2(10, 0); slideArea.offsetMax = new Vector2(-10, 0);
            var handle = Img(Stretch(slideArea, "Handle"), "slider_knob");
            handle.rectTransform.sizeDelta = new Vector2(28, 0);
            var s = rt.gameObject.AddComponent<Slider>();
            s.fillRect = fill.rectTransform; s.handleRect = handle.rectTransform; s.targetGraphic = handle;
            s.direction = Slider.Direction.LeftToRight; s.minValue = 0; s.maxValue = 1; s.value = 1;
            s.colors = Tint();
            return s;
        }

        static Toggle Switch(RectTransform row)
        {
            var rt = R(row, "Switch", 860, 16, 64, 28);
            var track = Img(rt, "toggle_track");
            var knob = Img(Anchored(rt, "Knob", new Vector2(0.5f, 0.5f), new Vector2(18, 0), new Vector2(22, 22)), "toggle_knob");
            knob.raycastTarget = false;
            var t = rt.gameObject.AddComponent<Toggle>();
            t.targetGraphic = track; t.colors = Tint();
            var sw = rt.gameObject.AddComponent<UiSwitch>();
            sw.knob = knob.rectTransform; sw.knobImage = knob;
            sw.label = T(row, "State", "ON", 20, 940, 14, 100, 30, TextAlignmentOptions.Right, TextColor, true);
            return t;
        }

        static ColorBlock Tint()
        {
            var c = ColorBlock.defaultColorBlock;
            c.normalColor = new Color(0.82f, 0.9f, 1f);
            c.highlightedColor = Color.white;
            c.selectedColor = Color.white;
            c.pressedColor = new Color(0.6f, 0.8f, 1f);
            c.disabledColor = new Color(0.45f, 0.5f, 0.6f, 0.7f);
            c.fadeDuration = 0.08f;
            return c;
        }

        // ---------- primitives ----------
        // A cluster is one named rect repeated in every layer that draws part of it, so plates (L1), meters (L2),
        // icons (L3) and text (L4) of the same panel share exact coordinates while each layer keeps its CanvasGroup.
        static RectTransform C(int layer, string name, Vector2 anchor = default, Vector2 pos = default, Vector2 size = default)
        {
            var key = layer + "/" + name;
            if (clusters.TryGetValue(key, out var rt)) return rt;
            if (size == default)
                foreach (var kv in clusters)
                    if (kv.Key.EndsWith("/" + name)) { anchor = kv.Value.anchorMin; pos = kv.Value.anchoredPosition; size = kv.Value.sizeDelta; break; }
            rt = Anchored(layers[layer], name, anchor, pos, size);
            clusters[key] = rt;
            return rt;
        }

        static void GroupToggle(HudView view, string name)
        {
            // one root per cluster is toggled by the view; keep the other layers' parts as children of a follower
            var follower = view.objectiveRoot.AddComponent<ClusterFollower>();
            var list = new List<GameObject>();
            foreach (var kv in clusters) if (kv.Key.EndsWith("/" + name) && kv.Value.gameObject != view.objectiveRoot) list.Add(kv.Value.gameObject);
            follower.others = list.ToArray();
        }

        static RectTransform Anchored(Transform parent, string name, Vector2 anchor, Vector2 pos, Vector2 size)
        {
            var rt = new GameObject(name, typeof(RectTransform)).GetComponent<RectTransform>();
            rt.SetParent(parent, false);
            rt.anchorMin = rt.anchorMax = rt.pivot = anchor;
            rt.anchoredPosition = pos;
            rt.sizeDelta = size;
            return rt;
        }

        // child rect by top-left offset (x right, y down) and size, in reference pixels
        static RectTransform R(Transform parent, string name, float x, float y, float w, float h)
            => Anchored(parent, name, new Vector2(0, 1), new Vector2(x, -y), new Vector2(w, h));

        static RectTransform Stretch(Transform parent, string name)
        {
            var rt = new GameObject(name, typeof(RectTransform)).GetComponent<RectTransform>();
            rt.SetParent(parent, false);
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = rt.offsetMax = Vector2.zero;
            return rt;
        }

        // Codex's generated crystal perimeter (Assets/Game/UI/PanelKit/Sprites, 128 px borders at 400 PPU = 32 canvas
        // units). Drawn over the procedural plate, centre not filled, never a raycast target; skipped if absent.
        static Image Art(RectTransform host, string id, float ppuMultiplier = 1f, float outset = 6f)
        {
            var sprite = AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Game/UI/PanelKit/Sprites/" + id + ".asset");
            if (sprite == null) return null;
            var rt = Stretch(host, "Art");
            rt.offsetMin = new Vector2(-outset, -outset); rt.offsetMax = new Vector2(outset, outset);
            var img = rt.gameObject.AddComponent<Image>();
            img.sprite = sprite; img.type = Image.Type.Sliced; img.fillCenter = false;
            img.pixelsPerUnitMultiplier = ppuMultiplier; img.raycastTarget = false;
            return img;
        }

        static string PlateFor(string framed) => AssetDatabase.LoadAssetAtPath<Sprite>("Assets/Game/UI/PanelKit/Sprites/status_frame.asset") != null ? "panel_frame_plain" : framed;

        static Sprite S(string name) => name == null ? null : AssetDatabase.LoadAssetAtPath<Sprite>(Kit + (name.StartsWith("Icons/") ? name : "Sprites/" + name) + ".png");

        static Image Img(RectTransform rt, string sprite, Color? color = null)
        {
            var img = rt.gameObject.AddComponent<Image>();
            img.sprite = S(sprite);
            if (sprite != null && img.sprite == null) Debug.LogError("HUD_KIT missing sprite " + sprite);
            img.color = color ?? Color.white;
            img.type = img.sprite != null && img.sprite.border != Vector4.zero ? Image.Type.Sliced : Image.Type.Simple;
            img.raycastTarget = false;
            return img;
        }

        static Image Filled(RectTransform rt, string sprite, Color color, Image.FillMethod method)
        {
            var img = Img(rt, sprite, color);
            img.type = Image.Type.Filled;
            img.fillMethod = method;
            img.fillOrigin = method == Image.FillMethod.Horizontal ? (int)Image.OriginHorizontal.Left : (int)Image.Origin360.Top;
            img.fillClockwise = true;
            img.fillAmount = 1f;
            return img;
        }

        static TextMeshProUGUI T(Transform parent, string name, string text, float size, float x, float y, float w, float h, TextAlignmentOptions align, Color color, bool bold)
        {
            var rt = R(parent, name, x, y, w, h);
            var t = rt.gameObject.AddComponent<TextMeshProUGUI>();
            t.text = text; t.fontSize = size; t.color = color; t.alignment = align;
            t.fontStyle = bold ? FontStyles.Bold : FontStyles.Normal;
            t.textWrappingMode = TextWrappingModes.Normal;
            t.overflowMode = TextOverflowModes.Ellipsis;
            t.raycastTarget = false;
            return t;
        }

        // ---------- scene + captures ----------
        static void BuildScene()
        {
            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            var cam = new GameObject("Main Camera", typeof(Camera)).GetComponent<Camera>();
            cam.tag = "MainCamera";
            cam.clearFlags = CameraClearFlags.SolidColor;
            cam.backgroundColor = new Color(0.55f, 0.48f, 0.38f);
            var sun = new GameObject("Directional Light", typeof(Light)).GetComponent<Light>();
            sun.type = LightType.Directional;
            sun.transform.rotation = Quaternion.Euler(50, -30, 0);
            new GameObject("EventSystem", typeof(EventSystem), typeof(InputSystemUIInputModule));
            var model = new GameObject("HudPreviewModel").AddComponent<HudPreviewModel>();
            var hud = (GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(PrefabPath));
            var view = hud.GetComponent<HudView>();
            view.modelSource = model;
            EditorSceneManager.SaveScene(scene, ScenePath);

            Capture(hud, view, model, cam);
            EditorSceneManager.OpenScene(ScenePath);
        }

        static void Capture(GameObject hud, HudView view, HudPreviewModel model, Camera cam)
        {
            var canvas = hud.GetComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceCamera;
            canvas.worldCamera = cam;
            canvas.planeDistance = 1;
            cam.GetUniversalAdditionalCameraData().renderPostProcessing = false;
            var rtex = new RenderTexture(1920, 1080, 24, RenderTextureFormat.ARGB32);
            cam.targetTexture = rtex;
            model.demo = false;
            model.fire = FireState.Charging; model.charge = 0.65f; model.hp = 0.72f; model.guardCurrent = 3;
            model.recover = -1;
            model.Push("Contract accepted: Recover the guild parcel.", AlertLevel.Info);
            model.Push("Guard broken", AlertLevel.Critical);
            view.Init();
            Directory.CreateDirectory("Documentation/build/hud");
            Shot(view, cam, rtex, "Documentation/build/hud/hud_combat.png");
            view.OpenModal(true);
            Shot(view, cam, rtex, "Documentation/build/hud/hud_board.png");
            view.OpenModal(false);
            Shot(view, cam, rtex, "Documentation/build/hud/hud_options.png");
            cam.targetTexture = null;
            Object.DestroyImmediate(rtex);
        }

        static void Shot(HudView view, Camera cam, RenderTexture rtex, string path)
        {
            view.Tick();
            foreach (var l in view.GetComponentsInChildren<HudLayer>(true)) l.Apply();
            foreach (var s in view.GetComponentsInChildren<UiSwitch>(true)) s.Refresh(true);
            Canvas.ForceUpdateCanvases();
            RenderPipeline.SubmitRenderRequest(cam, new RenderPipeline.StandardRequest { destination = rtex });
            var read = new Texture2D(1920, 1080, TextureFormat.RGBA32, false);
            RenderTexture.active = rtex;
            read.ReadPixels(new Rect(0, 0, 1920, 1080), 0, 0);
            read.Apply();
            RenderTexture.active = null;
            File.WriteAllBytes(path, read.EncodeToPNG());
            Object.DestroyImmediate(read);
        }
    }
}
