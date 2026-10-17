# Guildbound GUI: our own LVGL-style toolkit, terminal HUD, gibberish NPC speech

## Context
The boss wants the game's full GUI built first, polished until the vibe is right. The look: elegant, transparent, turquoise-green terminal text. The layout is single-player and Minecraft-like, with a RuneScape-style round minimap. NPCs "speak" gibberish that the game auto-translates in text, so we don't need voice acting. Later, a terminal-style tool will map the 28 Rokoko mocap clips into moves.

**Why not `lv_binding_rust`:**
- The `lvgl` crate's last release was 0.6.2 in April 2023, and its README calls it "still in development".
- It only supports pointer input, which rules out typing.
- There are no Windows build instructions; it needs clang plus compiled C code.
- It renders on the CPU into a framebuffer.

The boss asked for our own toolkit instead. LVGL is MIT-licensed, so we take its *design* and implement it in Rust on top of Bevy UI, which is already GPU-rendered and already used by the bag, F1 and F4 panels.

The LVGL ideas we take:
- objects with styles per *state* (default, hovered, pressed, focused, checked, disabled)
- themes
- `lv_anim`-style tweens with easing curves
- label long-modes
- bars and arcs
- focus groups for keyboard and gamepad

Bevy 0.19.1 already supplies every primitive this needs (checked in the registry sources):
- `BoxShadow` for glow, `BackgroundGradient`, `TextShadow`, `BorderRadius`, `Outline`
- `UiTransform` for rotation and scale, `UiMaterial` for custom WGSL
- `ImageNode`, `ViewportNode`, `Camera::world_to_viewport`
- `bevy_input_focus` directional navigation

## Layout
```
+--------------------------------------------------------------------+
| > +3 Log                                                            |
| > Opponent: "Vashen tul, oro kessa?"                               |
| > -> "Halt, traveller. Where to?"   (log fades when idle)           |
|                          "Vashen tul..."  <- overhead gibberish     |
|                               [E] Talk                              |
|                                               +-- bag (5x4) ------+ |
|  .-- map --.        HP  ##########----        |  terminal grid    | |
| (  topo     )       ST  #######-------        |  hover = details  | |
|  '---N-----'  [1][2][3][4][5][6][7][8][9][0] [BAG]---------------+ |
+--------------------------------------------------------------------+
```
- **Bottom-left:** a round minimap with a compass, a player arrow, and dots for NPCs (yellow) and items (red). It rotates with the camera, as in RuneScape.
- **Left, above the map:** the terminal log, single-player. It shows pickups, "Bag full", NPC lines and system messages, and fades when idle, like Minecraft chat.
- **Bottom-centre:** HP and stamina bars, plus mana once magic lands. Below them is a 10-slot hotbar showing the player's `Loadout` (keys 1-0 already switch slots). The **bag icon** sits at the far right.
- **Bottom-right:** the bag panel opens here, from the bag icon or `I`.

## Step 1: `crates/gbui` (new crate, Bevy only, no game types, testable headless like `sim`)
- `theme.rs` holds the `Theme` resource, loaded from `assets/ui/theme.json` with hot reload so the vibe can be tuned live:
  - palette: accent turquoise around #2EE6C5, accent-2 green around #7CFFB2, text, dim, danger
  - background alpha, border width, radius, glow, font sizes, font
  - per-class, per-state overrides; `UiState` is derived from `Interaction` and focus each frame and resolved into colours, borders and shadows
- `anim.rs` has the `Tween` component:
  - targets: opacity, offset, scale, fill
  - easing like `lv_anim_path`: linear, ease-out, overshoot, bounce
  - delay, repeat, playback
  - one system applies all tweens
- `term.rs` holds the terminal text pieces:
  - `Typewriter`: reveals text at a set rate with a `▌` cursor
  - `Decode`: scrambles glyphs from a source text into a target text; this is the "translation" effect
  - `TermLog`: bounded lines, prefixes, per-line fade, idle fade
- `widgets.rs` holds the builders:
  - `panel`: glass background, 1 px accent border, corner brackets, glow
  - `bar`: fill plus a ghost trail that shows damage
  - `slot`: key label and selected state
  - `tooltip` and `icon_button`
- `shaders/`:
  - `glass.wgsl`: scanlines, faint noise and vignette on panels
  - `ring.wgsl`: circular mask, ring and map rotation/offset uniforms for the minimap
- Font: download **JetBrains Mono** (OFL, includes block and box-drawing glyphs) to `assets/fonts/jetbrains-mono/` with its `OFL.txt`.
- Tests: easing curves, typewriter and decode progress, theme parse plus state resolution.

## Step 2: HUD in the game
`play/hud.rs` becomes `play/hud/`.

- `hud/mod.rs`: the root layout and the restyled interaction prompt (`[E] Put on armor`).
- `hud/vitals.rs`: reads `Fighter.hp/max_hp` and `stamina/max_stamina` (`play/mod.rs:243`) and drives the bars with a damage trail.
- `hud/hotbar.rs`:
  - shows `Loadout.slots` and `Loadout.current` (read-only; `input.rs:118-157` already switches slots)
  - the bag icon sends a `ToggleBag` message, which `inventory::toggle` consumes, so there's still one writer of `BagOpen`
- `hud/log.rs`: `HudLog` messages go into a gbui `TermLog`. The inventory toasts (`inventory.rs`) become log lines.
- `hud/minimap.rs`:
  - a topographic map is drawn once at startup from `sim::terrain::ground` (`crates/sim/src/terrain.rs:43`): turquoise contour lines and height shading on a transparent background, saved into an `Image`
  - `ring.wgsl` masks it to a circle and rotates/offsets it by the player's position and the camera yaw
  - the compass N marker rotates, and dots come from `Npc` and item positions
  - this is cheaper than a second camera and fits the terminal look
- `inventory.rs`: keep the data logic (collect, reject, `Stack`s). Its panel is rebuilt with gbui `panel` and `slot`, anchored bottom-right.
- `camera.rs` `capture_cursor` stays as it is; it already reads `BagOpen`.

## Step 3: gibberish speech (`play/dialogue.rs`)
- `assets/dialogue/*.json`: speaker, language, and the lines written in English.
- `gibberish(text, lang)` is deterministic:
  - each word is hashed (FNV) into syllables from the language's syllable table, so the same word always gives the same sounds
  - word length, capitals and punctuation are kept
- The `Npc` ("Opponent", `world.rs:1007`) gets an `Interactable { kind: "talk" }`. On `UseEffect` it says its next line:
  - an overhead gibberish bubble, positioned each frame with `world_to_viewport`, which fades out
  - a log line `Opponent: <gibberish>` that decodes into the English
- Per-syllable blip sounds through `fx::Sfx` come later.
- Tests: determinism, capitals and punctuation, one language never maps two words to the same sound-word within a line.

## Step 4: after the boss signs off the vibe, the mocap console (maps the Rokoko clips)
- **Opening the console:** `` ` `` opens a gbui terminal with an input line.
- **Commands:** `clips`, `play rokoko/Fireballs`, `seek`, `speed`, `in`/`out` marks, and `save <moveset> <attack>`.
  - `save` writes `ClipRef { track, start, end, speed }` windows (`play/moveset.rs:26`) into `assets/anim/movesets/*.json`.
- **Preview:** a stage mannequin with no `Mover` plays the clip. Its layers are written by the console, the same pattern as the armour stand (`armor.rs` `pose_stands`).

## Bookkeeping
- `docs/HANDOFF.md`: module map rows (gbui, hud/*, dialogue) and ownership rows:
  - HUD nodes: `hud`
  - `HudLog`: emitted by anyone, shown by `hud::log`
  - `ToggleBag`: sent by `hud` / input, consumed by `inventory`
  - speech/dialogue state: `dialogue`
  - The stale `D:\gb_assets_archive` line gets fixed.
- No commits unless the boss says so.

## Verification
- `cargo test -p gbui` and `cargo test -p guildbound`, all passing.
- `cargo build -p guildbound` as the bar.
- **Playtest pause after Step 2 (vibe):** the boss runs `play.cmd`.
  - Tune `assets/ui/theme.json` live with hot reload.
  - Check that the bars react to damage, the hotbar follows 1-0, the bag opens from the icon and `I`, and the minimap rotates with the camera with correct dots.
- **Playtest after Step 3:** talk to the Opponent (E). Gibberish shows overhead, and the log decodes it into English.
