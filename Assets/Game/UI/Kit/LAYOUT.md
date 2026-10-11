# HUD layout contract (1920x1080 reference, CanvasScaler match 0.5)

Built by `Guildbound/UI/Build HUD` (Assets/Game/UI/Kit/Editor/HudBuilder.cs). Frames: `python -I Tools/build_hud_kit.py` (exact sizes and 9-slice borders in Sprites/kit.json, 2x textures at 200 PPU). Optional crystal perimeter art: Assets/Game/UI/PanelKit/Sprites (Codex), drawn over the plates, fillCenter off, no raycasts.

Layers: L0 accents, L1 plates, L2 meters, L3 icons, L4 text, L5 prompts, L6 alerts, L7 modal. Each is a CanvasGroup + HudLayer with floors 0 / .875 / .85 / .85 / .85 / .875 / .90 / 1 (plates baked at 0.80 alpha -> >= 0.70 effective behind text).

A1 weather (`Runtime/HudWeather.cs`, built by `HudBuilder.AddWeather`): density follows game state instead of a slider the player has to manage. TOWN parks the combat panel at .35 and leaves the objective surface open at 1.0; APPROACH is .80 / 1.0; FIGHT is 1.0 on the panel and .45 on the objective, so the left column carries the contract in town and the fight carries HP, guard and fire. Compass and minimap ride at 1 / 1 / .80; alerts, prompt and recover never fade. The weather multiplies INTO the layers as one CanvasGroup per surface on the cluster nodes - effective alpha is surface x layer - because surfaces share layers: the objective ledger and the combat panel both draw on L1, so a layer-wide fade would dim HP with it. Layer floors are unchanged, so whatever stays on screen stays legible, and the centre third stays empty in all three phases. Hold LeftAlt to freeze the weather where it stands; any surface pinned in `hud.weather.pin.<surface>` ignores the weather entirely. Build captures: `hud_town.png` and `hud_combat.png` are the two poles of the same frame.

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
| Compass plate (L1, RectMask2D) | top-centre | 0, 42 down | 601 x 71, static compass_strip |
| - heading tape (slides) | plate | rest x -1612 | 3600 x 71, 5 px per degree, 2 revolutions |
| - cardinals N..NW, twice | tape | 225 i, 22 | 225 x 26 |
| - centre marker (L3) | plate | centred | 28 x 28, fixed |
| Minimap frame | top-right | -20, 56 down | 256 x 256 (square, the art is a circle) |
| - mappable window (L2, Mask) | frame | 20, 20 | 216 x 216, disc_mask backdrop |
| - empty state (L4) | frame | 20, 113 | 216 x 30, "NO MAP DATA" |
| Alerts x2 (L6) | top-right | -20, 341 down; pitch 76 | 440 x 64 |
| Modal window (L7) | centre | 0, 0 | 1120 x 700 |
| - header / board, inventory, options tabs / close | window | 24, 20 / 454, 604, 754 at y 28 / 990, 28 | 1072 x 52 / 140 x 36 / 96 x 36 |
| - trust / wallet blocks | window | 24, 88 / 576, 88 | 520 x 100 |
| - contract cards x2 | window | 24 + 548 i, 200 | 524 x 470 (art slot 480 x 160 at 22, 28; accept 220 x 48 at 152, 402) |
| - option rows x8 | window | 24, 96 + 68 i | 1072 x 60 (slider 500 at 420; value at 940) |
| - inventory page | window | 0, 0 | 1120 x 700; shares header and L7 CanvasGroup |
| - inventory capacity | inventory | 44, 92 | 540 x 36 |
| - inventory slots x12 | inventory | 44 + 136 column, 138 + 160 row | 118 x 138, four columns and three rows |
| - slot icon / count | slot | 21, 20 / 14, 104 | 76 x 76 / 90 x 26 |
| - inventory item details | inventory | 618, 108 | 456 x 544 |
| - detail icon / name / category | details | 148, 22 / 32, 202 / 32, 266 | 160 x 160 / 392 x 64 / 392 x 30 |
| - description / action / refusal reason | details | 32, 304 / 32, 436 / 32, 500 | 392 x 120 / 392 x 56 / 392 x 28 |

Centre third (640..1280 x 360..720) stays empty in combat. The board's top edge is now in place: compass tape centred at y 42..113 and the round minimap at the top right, with alerts moved down to y 341 so the corner belongs to the map.

World map status: the compass and minimap are laid out but NOT yet bound, because IHudModel does not carry HasMapData / CompassHeadingDegrees / GetMapBounds / MapBlipCount / GetMapBlip (interface posted to Claude 19:30). Until that lands the tape sits at north and the minimap shows its "NO MAP DATA" state - deliberately, since a minimap that invents terrain reads as a bug rather than a placeholder. Binding is a handful of lines in HudView.Tick: slide compassTape by -heading * 5 px per degree wrapped to +-180, then project each world-space blip into mapWindow against GetMapBounds. The compass cardinals are parented to the tape rather than to L4 so they travel with it on scroll; HudView still applies the text-size option to them because it gathers every TMP_Text under the canvas.

Input: I = inventory outside combat, Tab = board, Esc = options/close; every control is a uGUI Selectable (keyboard/gamepad navigation via InputSystemUIInputModule). Combat rejects inventory with an L6 "Not during a fight" alert and closes an already open inventory when combat begins.

Generated crystal frame PPU multipliers are multiplied by .65 for stronger corner silhouettes. Frame centres are transparent and decorative Images do not receive raycasts. InventoryView binds through IInventoryModel; the builder scene supplies InventoryPreviewModel. Preview equip toggles a display flag only. Production recovery, appearance changes and persistence still need the rules model.
