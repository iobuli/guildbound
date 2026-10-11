# HUD layout contract (1920x1080 reference, CanvasScaler match 0.5)

Built by `Guildbound/UI/Build HUD` (Assets/Game/UI/Kit/Editor/HudBuilder.cs). Frames: `python -I Tools/build_hud_kit.py` (exact sizes and 9-slice borders in Sprites/kit.json, 2x textures at 200 PPU). Optional crystal perimeter art: Assets/Game/UI/PanelKit/Sprites (Codex), drawn over the plates, fillCenter off, no raycasts.

Layers: L0 accents, L1 plates, L2 meters, L3 icons, L4 text, L5 prompts, L6 alerts, L7 modal. Each is a CanvasGroup + HudLayer with floors 0 / .875 / .85 / .85 / .85 / .875 / .90 / 1 (plates baked at 0.80 alpha -> >= 0.70 effective behind text).

| Content host | Anchor | Position (x, y from anchor) | Size |
|---|---|---|---|
| Combat panel | top-left | 32, 32 | 600 x 264 |
| - portrait slot | panel | 20, 18 | 120 x 120 |
| - HP bar (frame / fill) | panel | 250, 26 / 255, 31 | 326 x 28 / 316 x 18 |
| - guard segments (5) | panel | 250, 70 | 60 x 14 each, pitch 66.5 |
| - fire slot / icon / sweep | panel | 30, 150 / 44, 164 / 50, 170 | 100 / 72 / 60 |
| - FIRE label / state | panel | 150, 168 / 150, 198 | 220 x 30 / 300 x 36 |
| Objective pill | top-left | 32, 308 | 440 x 56 |
| Prompt (L5) | bottom-centre | 0, 120 up | 460 x 56 |
| Recover ring (L5) | bottom-centre | 0, 196 up | ring 88, label 240 x 30 |
| Alerts x2 (L6) | top-right | -32, 32; pitch 76 | 440 x 64 |
| Modal window (L7) | centre | 0, 0 | 1120 x 700 |
| - header / tabs / close | window | 24, 20 / 690 & 840, 28 / 990, 28 | 1072 x 52 / 140 x 36 / 96 x 36 |
| - trust / wallet blocks | window | 24, 88 / 576, 88 | 520 x 100 |
| - contract cards x2 | window | 24 + 548 i, 200 | 524 x 470 (art slot 480 x 160 at 22, 28; accept 220 x 48 at 152, 402) |
| - option rows x8 | window | 24, 96 + 68 i | 1072 x 60 (slider 500 at 420; value at 940) |

Centre third (640..1280 x 360..720) stays empty in combat. Input: Tab = board, Esc = options/close; every control is a uGUI Selectable (keyboard/gamepad navigation via InputSystemUIInputModule).
