# Small Unity HUD playtest

Open menu `Guildbound/UI/Play HUD Sandbox` in Unity 6000.3.26f1. The scene is `Assets/Game/UI/PanelKit/HudPlaytest.unity`; it uses Claude's primary HUD and the generated blue perimeter sprites. State is sandbox data, not gameplay.

- H: sample hit, minus 15 HP; interrupts recovery/charge.
- G: spend one guard segment; zero shows guard break.
- Hold Q, then release: charge, release and cooldown.
- R: sample three-second recovery.
- I: inventory with twelve slots, item selection and details. Click the appearance piece, then EQUIP/UNEQUIP to change its sample equipped flag.
- C: toggle the sample combat state. Entering combat closes inventory; I during combat shows "Not during a fight".
- E or Tab: guild board; click its real contract controls.
- Escape: open options or close the current modal. Try opacity, text size, reduced motion and effects.
- F5: reset sample HP/guard/fire/recovery.
- F10: quit the standalone player. Use Unity's Play button to stop Editor Play Mode.

An earlier Windows sandbox player built locally with zero build errors. Its path is recorded in [playtest-path.txt](playtest-path.txt); it stays outside git. The current integrated inventory was rebuilt in the live Unity editor through Aura, and has not been rebuilt into that player.

Frames generated with the built-in image_gen tool from the supplied HUD board. Saved image: [frames-sheet-v1.png](frames-sheet-v1.png). Exact generation prompt: [generation-prompt-v1.txt](generation-prompt-v1.txt). Measured source cuts, borders and content sizes: [panel-kit-manifest.json](panel-kit-manifest.json). Frame resizing preview: [FreeGuild-Panel-Kit.html](../../../FreeGuild-Panel-Kit.html).

Verified in Unity: original panel importer/build exits zero, content dimensions, saved pointer-click and submit callbacks, slider callback and backing opacity floor. Integrated inventory compile and primary HUD/editor scene rebuild passed through Aura. Edit-mode capture: Documentation/build/hud/hud_inventory.png. Live input verification remains pending: two Play Mode attempts exited before the sequence could assert inventory behavior. This is not a runtime pass.

The integrated page uses the primary HUD Canvas and shared L7 modal. Four occupied preview categories follow AMEND-INV-01: objective, preparation, appearance equipment and keepsake. Preview data does not implement production recovery consumption, character appearance or saves. Aura takes over the real rules/model and HUD refinement per the boss's latest Tandem mandate.
