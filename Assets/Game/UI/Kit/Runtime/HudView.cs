using Guildbound.Contracts;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Guildbound.UI
{
    // Binds the layered HUD to an IHudModel. Presentation only: reads state each frame, forwards requests.
    public class HudView : MonoBehaviour
    {
        public MonoBehaviour modelSource;
        IHudModel model;

        [Header("Combat")]
        public MeterView hp;
        public SegmentRow guard;
        public Image fireIcon, chargeRing, cooldownDisc;
        public Sprite[] fireSprites = new Sprite[6];   // FireState order
        public TMP_Text fireLabel, fireState;
        public UiPulse fireGlow;
        public Image vignette;

        [Header("Objective / prompt / recover")]
        public GameObject objectiveRoot;
        public TMP_Text objectiveText;
        public GameObject promptRoot;
        public TMP_Text promptText, promptKey;
        public GameObject recoverRoot;
        public Image recoverRing;

        [Header("Alerts (max 2, 3 s)")]
        public GameObject[] alertSlots = new GameObject[2];
        public TMP_Text[] alertTexts = new TMP_Text[2];
        public Image[] alertAccents = new Image[2];
        readonly float[] alertTimers = new float[2];

        // World map. compassTape slides horizontally at 5 px per degree behind a fixed centre marker;
        // mapWindow is the circular disc blips project into; mapEmpty is the honest state while the model
        // reports no map data. Binding waits on IHudModel.HasMapData / CompassHeadingDegrees / GetMapBounds
        // (posted 19:30); the layout is finished so that binding is a handful of lines in Tick.
        [Header("World map")]
        public RectTransform compassTape;
        public RectTransform mapWindow;
        public TMP_Text mapEmpty;
        public float compassPixelsPerDegree = 5f;
        public float blipSize = 10f;
        public Sprite blipSprite;
        public Color blipSelf = new Color(0.37f, 0.84f, 1f), blipOpponent = new Color(1f, 0.29f, 0.35f);
        readonly System.Collections.Generic.List<Image> blips = new System.Collections.Generic.List<Image>();
        float compassRestX, compassRestY;
        bool compassRestTaken;

        [Header("Weather (A1)")]
        public HudWeather weather;

        [Header("Modal (L7)")]
        public HudLayer modalLayer;
        public TMP_Text windowTitle;
        public GameObject boardWindow, optionsWindow;
        public Toggle boardTab, optionsTab, inventoryTab;
        public InventoryView inventory;
        public MonoBehaviour inventorySource;
        public TMP_Text wallet, trustName;
        public Image trustBar;
        public TMP_Text[] cardTitles, cardObjectives, cardServices, cardButtonLabels;
        public Button[] cardButtons;
        public Slider[] sliders = new Slider[5];       // master, panels L1, prompts L5, accents L0, text size
        public TMP_Text[] sliderValues = new TMP_Text[5];
        public Toggle reducedMotion, cameraShake, effectsOnHud;
        TMP_Text[] texts;
        float[] baseSizes;
        float appliedScale = -1f;
        public Button closeButton;

        static readonly string[] FireNames = { "LOCKED", "READY", "CHARGING", "RELEASE", "COOLDOWN", "INTERRUPTED" };
        static readonly Color Text = new Color(0.918f, 0.965f, 1f), Warn = new Color(1f, 0.70f, 0.28f), Critical = new Color(1f, 0.29f, 0.35f), Edge = new Color(0.37f, 0.84f, 1f);

        void Awake() => Init();

        public void Init()
        {
            model = modelSource as IHudModel;
            for (int i = 0; i < cardButtons.Length; i++)
            {
                int index = i;
                cardButtons[i].onClick.AddListener(() => model.RequestAcceptContract(model.GetContract(index).Id));
            }
            boardTab.onValueChanged.AddListener(on => { if (on) Show(true); });
            optionsTab.onValueChanged.AddListener(on => { if (on) Show(false); });
            if (inventory)
            {
                if (inventorySource) inventory.Bind((IInventoryModel)inventorySource);   // plain C# models bind through BindInventory
                inventoryTab.onValueChanged.AddListener(on => { if (on) OpenInventory(); });
            }
            closeButton.onClick.AddListener(CloseModal);

            float[] start = { HudSettings.Master, HudSettings.PanelOpacity, HudSettings.PromptOpacity, HudSettings.AccentOpacity, HudSettings.TextScale };
            for (int i = 0; i < sliders.Length; i++) sliders[i].SetValueWithoutNotify(start[i]);
            reducedMotion.SetIsOnWithoutNotify(HudSettings.ReducedMotion);
            cameraShake.SetIsOnWithoutNotify(HudSettings.CameraShake);
            effectsOnHud.SetIsOnWithoutNotify(HudSettings.EffectsOnHud);
            sliders[0].onValueChanged.AddListener(v => HudSettings.Master = v);
            sliders[1].onValueChanged.AddListener(v => HudSettings.PanelOpacity = v);
            sliders[2].onValueChanged.AddListener(v => HudSettings.PromptOpacity = v);
            sliders[3].onValueChanged.AddListener(v => HudSettings.AccentOpacity = v);
            sliders[4].onValueChanged.AddListener(v => HudSettings.TextScale = v);
            texts = GetComponentsInChildren<TMP_Text>(true);
            baseSizes = System.Array.ConvertAll(texts, t => t.fontSize);
            reducedMotion.onValueChanged.AddListener(v => HudSettings.ReducedMotion = v);
            cameraShake.onValueChanged.AddListener(v => HudSettings.CameraShake = v);
            effectsOnHud.onValueChanged.AddListener(v => HudSettings.EffectsOnHud = v);
            modalLayer.visible = false;
            if (compassTape)   // the authored rest position is the marker's true north; captured once so re-init cannot drift it
            {
                compassRestX = compassTape.anchoredPosition.x;
                compassRestY = compassTape.anchoredPosition.y;
                compassRestTaken = true;
            }
        }

        public void BindInventory(IInventoryModel source)
        {
            if (inventory) inventory.Bind(source);
        }

        void Update()
        {
            if (model == null) return;
            var kb = Keyboard.current;
            if (kb != null && kb.iKey.wasPressedThisFrame && inventory)
            { if (InventoryVisible) CloseModal(); else OpenInventory(); }
            else if (kb != null && kb.tabKey.wasPressedThisFrame) { if (modalLayer.visible && boardWindow.activeSelf) CloseModal(); else OpenModal(true); }
            else if (kb != null && kb.escapeKey.wasPressedThisFrame) { if (modalLayer.visible) CloseModal(); else OpenModal(false); }
            Tick();
        }

        public void Tick()
        {
            hp.Set(model.Hp01);
            guard.Set(model.GuardCurrent, model.GuardMax, model.GuardBroken);
            if (vignette)
            {
                bool lowHp = model.Hp01 <= 0.25f && HudSettings.EffectsOnHud;
                float pulse = HudSettings.ReducedMotion ? 0.6f : 0.45f + 0.25f * Mathf.Sin(Time.unscaledTime * 6f);
                vignette.color = lowHp ? new Color(0.55f, 0.02f, 0.06f, pulse) : new Color(0f, 0f, 0f, 0.35f);
            }

            int f = (int)model.Fire;
            fireIcon.sprite = fireSprites[f];
            fireState.text = FireNames[f];
            fireState.color = model.Fire == FireState.Interrupted ? Warn : model.Fire == FireState.Locked ? new Color(0.6f, 0.66f, 0.75f) : new Color(1f, 0.72f, 0.3f);
            chargeRing.fillAmount = model.Fire == FireState.Charging ? model.FireCharge01 : model.Fire == FireState.Release ? 1f : 0f;
            cooldownDisc.fillAmount = model.Fire == FireState.Cooldown ? model.FireCooldown01 : 0f;
            fireGlow.running = model.Fire == FireState.Ready || model.Fire == FireState.Charging;

            bool carrying = !string.IsNullOrEmpty(model.CarriedObjective);
            objectiveRoot.SetActive(carrying);
            if (carrying) objectiveText.text = string.IsNullOrEmpty(model.ReturnInstruction) ? model.CarriedObjective : model.ReturnInstruction;

            bool prompt = !string.IsNullOrEmpty(model.Prompt);
            promptRoot.SetActive(prompt);
            if (prompt) { promptText.text = model.Prompt; promptKey.text = model.PromptKey; }

            recoverRoot.SetActive(model.Recover01 >= 0f);
            if (model.Recover01 >= 0f) recoverRing.fillAmount = model.Recover01;

            UpdateAlerts();
            UpdateMap();
            // A1 weather: InCombat is a bool, so today it is the two poles of the crossfade. When
            // IHudModel.Engagement01 lands this expression becomes that value and nothing else changes.
            if (weather) weather.Tick(model.InCombat ? 1f : 0f);
            if (InventoryVisible && model.InCombat) CloseModal();
            if (inventoryTab) inventoryTab.interactable = !model.InCombat;
            if (modalLayer.visible) UpdateModal();
            // text size option: 0..1 maps to 0.8x..1.2x of the authored sizes (default 0.5 = 1x)
            float scale = Mathf.Lerp(0.8f, 1.2f, HudSettings.TextScale);
            if (!Mathf.Approximately(scale, appliedScale))
            {
                appliedScale = scale;
                for (int i = 0; i < texts.Length; i++) texts[i].fontSize = baseSizes[i] * scale;
            }
        }

        // Compass: the tape slides behind the fixed centre marker, and the heading it shows is the CAMERA's view
        // direction, not the fighter's facing, because the strip reads as "which way am I looking". DeltaAngle wraps
        // to +-180 so crossing north does not run the 3600 px (two revolution) tape off its travel.
        // Minimap: blips are world XZ projected against the geometry bounds into the disc. When the model reports no
        // map data the honest empty state shows instead of an invented map.
        void UpdateMap()
        {
            if (compassTape && compassRestTaken)
                compassTape.anchoredPosition = new Vector2(compassRestX - Mathf.DeltaAngle(0f, model.CompassHeadingDegrees) * compassPixelsPerDegree, compassRestY);

            if (!mapWindow) return;
            bool has = model.HasMapData;
            if (mapEmpty) mapEmpty.gameObject.SetActive(!has);

            int count = has ? Mathf.Max(0, model.MapBlipCount) : 0;
            while (blips.Count < count) AddBlip();
            for (int i = 0; i < blips.Count; i++) blips[i].gameObject.SetActive(i < count);
            if (count == 0) return;

            float minX, minZ, maxX, maxZ;
            model.GetMapBounds(out minX, out minZ, out maxX, out maxZ);
            float invX = maxX > minX ? 1f / (maxX - minX) : 0f, invZ = maxZ > minZ ? 1f / (maxZ - minZ) : 0f;
            float w = mapWindow.rect.width, h = mapWindow.rect.height;
            for (int i = 0; i < count; i++)
            {
                var blip = model.GetMapBlip(i);
                float u = (blip.X - minX) * invX - 0.5f;   // -0.5 .. 0.5 across the bounds
                float v = (blip.Z - minZ) * invZ - 0.5f;   // +Z is the map's up
                blips[i].rectTransform.anchoredPosition = new Vector2(u * w, v * h);
                blips[i].color = blip.Kind == MapBlipKind.Opponent ? blipOpponent
                    : blip.Kind == MapBlipKind.Self ? blipSelf : Edge;
            }
        }

        // Blips are built at runtime rather than authored: the count is model-driven, so an authored pool could only
        // ever be a guess at how much map there is.
        void AddBlip()
        {
            var go = new GameObject("Blip", typeof(RectTransform), typeof(Image));
            var rt = (RectTransform)go.transform;
            rt.SetParent(mapWindow, false);
            rt.sizeDelta = new Vector2(blipSize, blipSize);
            var img = go.GetComponent<Image>();
            img.sprite = blipSprite;
            img.raycastTarget = false;
            blips.Add(img);
        }

        void UpdateAlerts()
        {
            for (int i = 0; i < alertTimers.Length; i++)
            {
                if (alertTimers[i] > 0f && (alertTimers[i] -= Time.unscaledDeltaTime) <= 0f) alertSlots[i].SetActive(false);
            }
            while (model.TryDequeueAlert(out var alert))
            {
                ShowAlert(alert);
            }
        }

        void ShowAlert(HudAlert alert)
        {
            alertTexts[1].text = alertTexts[0].text; alertAccents[1].color = alertAccents[0].color;
            alertTimers[1] = alertTimers[0]; alertSlots[1].SetActive(alertSlots[0].activeSelf);
            alertTexts[0].text = alert.Text;
            alertAccents[0].color = alert.Level == AlertLevel.Critical ? Critical : alert.Level == AlertLevel.Warning ? Warn : Edge;
            alertTimers[0] = 3f; alertSlots[0].SetActive(true);
        }

        void UpdateModal()
        {
            wallet.text = model.Wallet.ToString();
            trustName.text = model.TrustName;
            trustBar.fillAmount = model.TrustProgress01;
            for (int i = 0; i < sliders.Length; i++) sliderValues[i].text = Mathf.RoundToInt(sliders[i].value * 100f) + "%";
            sliderValues[4].text = Mathf.RoundToInt(Mathf.Lerp(80f, 120f, sliders[4].value)) + "%";
            for (int i = 0; i < cardTitles.Length; i++)
            {
                bool has = i < model.ContractCount;
                cardButtons[i].transform.parent.gameObject.SetActive(has);
                if (!has) continue;
                var c = model.GetContract(i);
                cardTitles[i].text = c.Title;
                cardObjectives[i].text = c.Objective;
                cardServices[i].text = c.Service;
                cardButtons[i].interactable = c.Available && !c.Accepted;
                cardButtonLabels[i].text = c.Accepted ? "ACCEPTED" : "ACCEPT";
            }
        }

        public void OpenModal(bool board)
        {
            modalLayer.visible = true;
            (board ? boardTab : optionsTab).SetIsOnWithoutNotify(true);
            (board ? optionsTab : boardTab).SetIsOnWithoutNotify(false);
            if (inventoryTab) inventoryTab.SetIsOnWithoutNotify(false);
            Show(board);
        }

        public bool InventoryVisible => inventory && modalLayer.visible && inventory.gameObject.activeSelf;

        public void OpenInventory()
        {
            if (model.InCombat)
            {
                inventoryTab.SetIsOnWithoutNotify(false);
                boardTab.SetIsOnWithoutNotify(boardWindow.activeSelf);
                optionsTab.SetIsOnWithoutNotify(optionsWindow.activeSelf);
                ShowAlert(new HudAlert { Text = "Not during a fight", Level = AlertLevel.Info });
                return;
            }
            modalLayer.visible = true;
            boardTab.SetIsOnWithoutNotify(false); optionsTab.SetIsOnWithoutNotify(false);
            inventoryTab.SetIsOnWithoutNotify(true);
            boardWindow.SetActive(false); optionsWindow.SetActive(false);
            inventory.gameObject.SetActive(true);
            windowTitle.text = "GUILD SATCHEL";
            inventory.Refresh(); inventory.FocusFirst();
        }

        void Show(bool board)
        {
            boardWindow.SetActive(board);
            optionsWindow.SetActive(!board);
            if (inventory) inventory.gameObject.SetActive(false);
            windowTitle.text = board ? "GUILD BOARD" : "HUD OPTIONS";
            var first = board ? (Selectable)cardButtons[0] : sliders[0];
            if (EventSystem.current) EventSystem.current.SetSelectedGameObject(first.gameObject);
        }

        public void CloseModal()
        {
            modalLayer.visible = false;
            if (EventSystem.current) EventSystem.current.SetSelectedGameObject(null);
        }
    }
}
