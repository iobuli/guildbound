# Guildbound (working title)

Design docs live in `docs/`. Two code bases share the same `data/*.json`:

| What | Where | Run |
|---|---|---|
| Python reference sim (stat engine, character sheet) | `sim/`, `tests/` | `python -m sim.demo` / `python -m unittest discover tests` |
| Godot 4.7 controller test arena | `game/`, `project.godot` | see below |

## Play the test arena
Godot is installed at `D:\Godot`.

	D:\Godot\Godot_v4.7.2-stable_win64.exe --path "D:\New folder (4)\guildbound"

Controls are printed in the in-game HUD (F3 hides it). Esc frees the mouse.

## Automated check (drives the real player with scripted input)
	D:\Godot\Godot_v4.7.2-stable_win64_console.exe --headless --path . -- --selftest

Run it headless. In a visible window, clicking or alt-tabbing releases the scripted keys and can fail checks.
Linux / cloud: download `Godot_v4.7.2-stable_linux.x86_64.zip` from the Godot releases, run `--headless --path . --import`
once, then the same `-- --selftest` (add `--only=move,melee,...` for a subset, `--combatlog` for swing traces).

## Credits
Character models: "Universal Base Characters" by Quaternius, CC0 (public domain) - https://quaternius.itch.io/universal-base-characters
Animations: Mixamo (Adobe) packs, retargeted onto the base body by tools/retarget.gd - free to use in the game, not to redistribute on their own (keep this repo private).
Fist set: ACCAD Motion Lab, The Ohio State University, CC BY 3.0 (Male2 martial arts takes).
Weapons: KayKit (Kay Lousberg), CC0. Icons: 496 RPG icons by Henrique Lazarini, CC0. Sounds: see assets/audio/CREDITS.txt.

## Clip contact sheet (visual check of library clips on the base body)
	D:\Godot\Godot_v4.7.2-stable_win64_console.exe --path . --script res://tools/clip_sheet.gd -- SS_idle SS_slash FIST_jab
Writes docs/clip_sheet.png. Needs a window (Linux: xvfb-run ... --rendering-driver opengl3).
