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

## Automated check (drives the real player with scripted input, 44 checks)
	D:\Godot\Godot_v4.7.2-stable_win64_console.exe --headless --path . -- --selftest

Run it headless. In a visible window, clicking or alt-tabbing releases the scripted keys and can fail checks.

## Credits
Character models: "Universal Base Characters" by Quaternius, CC0 (public domain) - https://quaternius.itch.io/universal-base-characters
Animations: "Universal Animation Library" 1 and 2 (Standard) by Quaternius, CC0 - https://quaternius.itch.io/universal-animation-library , https://quaternius.itch.io/universal-animation-library-2

## Animation contact sheet (visual check of every movement state)
	D:\Godot\Godot_v4.7.2-stable_win64_console.exe --path . -- --animshots
Writes docs/anim_sheet.png. Needs a window; don't touch the mouse while it runs.
