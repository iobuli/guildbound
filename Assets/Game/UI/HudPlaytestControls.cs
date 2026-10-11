using System;
using System.IO;
using Guildbound.Contracts;
using TMPro;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Guildbound.UI
{
    // Local HUD sandbox only. Values exercise IHudModel presentation, not gameplay.
    public sealed class HudPlaytestControls : MonoBehaviour
    {
        public HudPreviewModel model;
        public HudView view;
        public TMP_Text instructions;
        float castTime = -1;
        float recoveryTime = -1;
        float openedAt;
        bool captured;

        void Start()
        {
            ResetSamples();
            openedAt = Time.unscaledTime;
            instructions.text = "HUD SANDBOX   I: inventory   C: combat test   H: hit   G: guard   Hold Q: cast   R: recovery   E/Tab: board   Esc: options   F5: reset";
            view.OpenInventory();
            Debug.Log("FreeGuild HUD sandbox started. H/G/Q/R sample controls, E/Tab board, Esc options.");
        }

        void ResetSamples()
        {
            model.demo = false;
            model.inCombat = false;
            model.hp = .8f;
            model.guardCurrent = 4;
            model.guardBroken = false;
            model.fire = FireState.Ready;
            model.charge = model.cooldown = 0;
            model.recover = -1;
            model.carried = "Sealed guild marker";
            castTime = recoveryTime = -1;
        }

        void Update()
        {
            var keys = Keyboard.current;
            if (keys == null) return;
            if (keys.f10Key.wasPressedThisFrame) Application.Quit();
            if (keys.f5Key.wasPressedThisFrame) ResetSamples();
            if (keys.cKey.wasPressedThisFrame)
            {
                model.inCombat = !model.inCombat;
                model.Push(model.inCombat ? "Combat test on / inventory blocked" : "Combat test off / inventory available", AlertLevel.Info);
            }
            if (keys.eKey.wasPressedThisFrame) view.OpenModal(true);
            if (!view.modalLayer.visible)
            {
                if (keys.hKey.wasPressedThisFrame)
                {
                    model.hp = Mathf.Max(0, model.hp - .15f);
                    recoveryTime = -1; model.recover = -1;
                    if (model.fire == FireState.Charging) { model.fire = FireState.Interrupted; castTime = .6f; }
                    model.Push("Sample hit / -15 HP", model.hp <= .25f ? AlertLevel.Critical : AlertLevel.Info);
                }
                if (keys.gKey.wasPressedThisFrame)
                {
                    model.guardCurrent = Mathf.Max(0, model.guardCurrent - 1);
                    model.guardBroken = model.guardCurrent == 0;
                    if (model.guardBroken) model.Push("Guard broken / sample feedback", AlertLevel.Critical);
                }
                if (keys.qKey.wasPressedThisFrame && model.fire == FireState.Ready)
                { recoveryTime = -1; model.recover = -1; model.fire = FireState.Charging; model.charge = 0; }
                if (model.fire == FireState.Charging)
                {
                    model.charge = Mathf.Clamp01(model.charge + Time.unscaledDeltaTime / 1.2f);
                    if (keys.qKey.wasReleasedThisFrame) { model.fire = FireState.Release; castTime = .3f; }
                }
                if (keys.rKey.wasPressedThisFrame && model.fire == FireState.Ready)
                { recoveryTime = 0; model.recover = 0; model.Push("Sample recovery / H or Q interrupts", AlertLevel.Info); }
            }
            if (castTime >= 0)
            {
                castTime -= Time.unscaledDeltaTime;
                if (model.fire == FireState.Cooldown) model.cooldown = Mathf.Clamp01(castTime / 3);
                if (castTime <= 0)
                {
                    if (model.fire == FireState.Release) { model.fire = FireState.Cooldown; model.cooldown = 1; castTime = 3; }
                    else { model.fire = FireState.Ready; model.cooldown = model.charge = 0; castTime = -1; }
                }
            }
            if (recoveryTime >= 0)
            {
                recoveryTime += Time.unscaledDeltaTime;
                model.recover = Mathf.Clamp01(recoveryTime / 3);
                if (recoveryTime >= 3)
                { model.hp = 1; model.recover = -1; recoveryTime = -1; model.Push("Recovery complete / sample HP restored", AlertLevel.Info); }
            }
            if (!captured && Time.unscaledTime - openedAt > 1.5f)
            {
                var args = Environment.GetCommandLineArgs();
                int index = Array.IndexOf(args, "--capture-hud");
                if (index >= 0 && index + 1 < args.Length) ScreenCapture.CaptureScreenshot(args[index + 1]);
                else if (Application.isEditor) ScreenCapture.CaptureScreenshot(Path.Combine(Application.dataPath,
                    "../Documentation/concepts/qa/hud-playtest-editor.png"));
                captured = true;
            }
        }
    }
}
