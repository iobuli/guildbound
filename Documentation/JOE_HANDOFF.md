# Joe - FreeGuild / Guildbound handoff

Updated 2026-10-11. This is a snapshot; read the latest team log before editing.

## Project and direction

The live project is **Unity only** at `D:\New folder (4)\guildbound`, branch `build/m0`. The old Godot project is archived in `_OldGodot/` as reference. Current installed stack: Unity **6000.3.26f1 LTS**, URP 17.3.0, new Input System 1.20.1, Animation Rigging 1.4.1, Cinemachine 3.1.7. ProjectVersion.txt and Packages/manifest.json are the actual stack; older notes naming 6000.5 are stale.

The prototype is grounded physical combat, a procedural animation layer, elemental progression, and one guild hall hub. Owned Asset Store models, clips, environment pieces and effects are the base, modified and combined like Lego. Damage comes from resolved contact in the combat rules. The motor is the only mover; animation never moves the fighter independently.

Read in this order: AGENTS.md, TANDEM_CODEX.manifest.md / TANDEM_CLAUDE.manifest.md, the latest entries in TANDEM_REASONING.log, then Documentation/HANDOFF.md. The latest BEGIN_MASTER_GDD block in Tandem is the design of record. Documentation/MASTER.md is older background. Do not run the legacy Game.cs, Fighter.cs or Assets/Game/Editor/Build.cs scene generator.

## Team and current ownership

- Boss sets direction. **Aura has full task, design and review authority**, by the boss's instruction. Both developers take assignments and submit results to Aura.
- Claude owns rules, architecture, fixtures, verification and commits.
- Codex owns creative presentation, character animation, game feel and VFX. Current delivery for Aura review: Armature binding, lean, hips-to-chest turn lag, verified-contact recoil and contextual SparringCamera. Leg IK is parked by Aura.
- Aura owns the HUD/GUI rework, production inventory presentation and Sparring scene creation.
- Joe's task is not assigned yet. Read the log and take an assignment from Aura before overlapping another agent.

Coordinate through TANDEM_REASONING.log and collab/CHANNEL.md, with Done / Review / Needs / Next entries. Announce scene edits first. Post `PLAY <agent> <reason>` before a Play session and `PLAY FREE` after; another agent must not inject input into that session. Work stays uncommitted until Aura reviews and Claude verifies/commits. Do not run batch Unity or another editor against the open project. A direct boss message takes priority.

## Built and verified

The latest recorded suite at this snapshot is **56/56 EditMode tests passing**, independently rerun by Aura, compile clean. W1 is committed as 37deeca. This covers combat, progress, inventory and world-geometry rules; it does not establish the complete hall/route playable loop.

- `Assets/Game/Combat`: deterministic 1/120 s combat simulation; movement, alternating light strikes, heavy, guard/break, step, throw, plain/charged cast and recovery. Combat event logs tested across 30/60/120 fps.
- `Assets/Game/Progress`: contracts, attempts, wallet, trust, services, attunement, deliver-once rewards and save snapshots. A gold-path fixture exercises GDD steps 2-9 on real rules. Snapshot includes owned garments and worn piece.
- `Assets/Game/Rules`: real twelve-slot InventoryModel. Objective reads GuildState's single carried objective; preparation stack cap 2; appearance-only equipment; display-only keepsakes. Preparation is spent on completed recovery, not the request.
- `Assets/Game/Play`: SparringDriver, FighterView, two opponent patterns, CombatHudModel, PlayInventoryContext and SparringCamera. Inventory preparation count is wired to recovery. Fighter visuals now use the owned Armature, with the contextual shoulder camera delivered for review. Driver-only input ramp is compiled; new momentum/facing/combo rules are Claude's next assigned slice.
- `Assets/Game/UI/Kit`: layered single-Canvas HUD with real uGUI buttons, sliders and toggles. Codex's twelve-slot inventory page and stronger generated crystal frame pass are committed as the baseline in 070b783. Aura is refining the layout and binding it to the real model.
- Art/motion labs: owned-body Blender experiments, palette, rig validation, actual animation-key captures and generated empty crystal panel surfaces. Existing supplied icons are retained. The chosen combat base is `Assets/Brawler Animations Sample/Models/Armature/Armature.fbx`. Codex extends Aura's existing HeroAnimator and maps 17 usable states. Both source punches are left-handed: Light2 mirrors one; Heavy uses a punch matching the rule's fist. Six targeted import overrides and 1,098 pose samples verify the Avatar, fall/rise and zero object-root drift. See `Documentation/build/animation/REVIEW.md` for reproducible checks and actual results.

## In progress and limits

Sparring.unity is built, wired and smoke-tested, using the owned tavern kit. The full hall/route playable loop is still to build. Codex's animation/motion slice is now in Assets, uncommitted for Aura review. Preview checks preserve all 72 transform positions/scales at 30/60/120 fps; controlled World-to-rig checks pass 13 samples, including the corrected front/back reactions.

The live inventory reads 1 / 12 slots and refuses opening during combat in Claude's smoke. Board contract/wallet/trust members now read the real GuildState; map bounds and two world-space blips are supplied by CombatHudModel. Aura owns the remaining UI verification/rebuild. Codex's cleaned real-input sequence proved F -> Heavy -> Heavy1 and captured both rigs, then Play exited before the rest finished: full real-input and repeated-Play acceptance remain open. Never report the aborted sequence as passed. Cast/recover clips, paired throws, contact fitting and VFX remain presentation work.

Claude's latest review also found two integration gaps: the plain-C# InventoryModel needs a public HUD bind called after Awake, rather than assignment to a MonoBehaviour inspector field; inventory snapshot fields exist, but restoring a complete live inventory from a save is still M1 work. Passing snapshot fixtures does not establish that round trip.

Current SparringDriver controls: WASD move, Shift run, LMB light, F heavy, RMB guard, Ctrl step, R throw, Q cast/hold, V recover, MMB lock, Enter reset, P partner pattern. HUD uses I inventory, Tab board, Esc options/close. The isolated HUD sandbox has different sample keys; see its PLAYTEST.md.

## MCP setup while you download

These connections are already configured and verified on this machine. On a different machine, adapt executable paths and install the corresponding tools; do not paste over existing client configuration.

| Connection | Installed server | Editor side |
|---|---|---|
| Aura -> Unity | `D:\aura\AuraForUnity\python\python.exe -m aura_unity_mcp.server` | Aura plugin 1.4.1 plus the live shared stdio bridge |
| Unity MCP -> Unity | Existing uvx; `--link-mode copy --from mcpforunityserver==10.3.0 mcp-for-unity --transport stdio` | MCP for Unity 10.3.0 |
| Blender MCP -> Blender | Existing uvx; `--from mcp-for-blender==2.1.9 mcp-for-blender`, `DISABLE_TELEMETRY=true` | Enabled blender_mcp add-on in Blender 5.2; add-on server must be running |

Aura and Blender are separate connections. MCP installation does not install or supply the game's owned asset packs. Those packs and local mesh derivatives are git-ignored, so a new clone needs the owned content restored.

Use `Tools/open_unity.ps1` to open this project with Unity's bridge started. It checks for an already open project. The active transport is **stdio**, not the old HTTP 8080 configuration. Codex and Claude registrations are already in place; reconnect/restart the client if it has not loaded the new servers.

For a direct connection check in the current workspace:

```powershell
& 'D:/aura/AuraForUnity/python/python.exe' Tools/aura_call.py get_unity_editor_state
& 'D:/aura/AuraForUnity/python/python.exe' Tools/aura_call.py check_compile_errors
```

Tools/aura_call.py selects this workspace and calls Aura MCP directly. `list` returns actual tool schemas. Detailed connection notes: [MCP_SETUP.md](MCP_SETUP.md). Blender's idle helper was closed after its successful connection test; start a Blender session for the next art task.

Use the authorized trial and existing local tools. No purchases, premium image/model generation, subscription changes or Training Data changes. The earlier Unity Play crash was associated with system memory exhaustion; Claude enabled DisableDomainReload. Runtime state must reset appropriately between plays. Avoid duplicate heavy app sessions.

Useful artifacts: `Documentation/build/CLIP_TABLE.md`, `Documentation/build/hud/hud_inventory.png`, `Documentation/concepts/hud/panel-kit/PLAYTEST.md`, `Documentation/MCP_SETUP.md`. Some large local concept viewers/render sheets are not in git. Latest ownership and status always come from Tandem.
