# Local Unity and Blender connections

The current setup uses two separate bridges. Aura connects to the live Unity editor; Blender MCP connects to Blender's add-on socket. Blender is not routed through Aura.

Aura is registered for Claude in this project's local scope and for Codex. Server command: `D:\aura\AuraForUnity\python\python.exe`, arguments `-m aura_unity_mcp.server`. Both `com.ramenvr.aura-unity` 1.4.1 and `com.coplaydev.unity-mcp` 10.3.0 are installed. The editor uses the shared stdio connection, not the earlier HTTP configuration. `Tools/open_unity.ps1` opens the current project with the bridge started; do not launch another editor when one already has the project open.

Codex can call the installed Aura server in the current session with:

```powershell
& 'D:/aura/AuraForUnity/python/python.exe' Tools/aura_call.py get_unity_editor_state
& 'D:/aura/AuraForUnity/python/python.exe' Tools/aura_call.py check_compile_errors
```

For another tool, inspect its actual schema with `Tools/aura_call.py list`, then provide a JSON argument file. This wrapper calls MCP tools directly, not an autonomous Aura chat agent. It selects the workspace containing Tools as the project root. Screenshot output can be saved through the optional third argument. Do not log account configuration or image base64 data.

Blender MCP is registered with the existing `uvx.exe`, arguments `--from mcp-for-blender==2.1.9 mcp-for-blender`, and `DISABLE_TELEMETRY=true`. Blender 5.2's existing `blender_mcp.py` add-on is enabled. Its idle default-scene helper was closed to release memory after connection verification. Start a Blender session and its add-on server when doing the next art task. Paid model-generation integrations remain disabled.

Verified locally on 2026-10-11: Blender get_scene_info returned the actual default scene; Aura listed 99 tools, found Guildbound through list_unity_project_roots, located HudPlaytest through query_project_assets, reported no compile errors, and rebuilt the integrated HUD through the existing editor. These connection checks do not establish a gameplay pass. The subsequent HUD input sequence exited Play Mode before asserting behavior, so runtime verification remains pending in the Tandem handover.

Use the authorized Aura trial and existing local tools. No purchases, premium generation, subscription changes or Training Data changes were made. Configuration backups are outside the repository under LOCALAPPDATA/FreeGuild/mcp-setup-backups.

Coordinate scene ownership in TANDEM_REASONING.log and collab/CHANNEL.md before switching scenes. Release an editor claim promptly after the announced check. Avoid source imports during another agent's short Play Mode sequence. Read the shared local Unity guides under `.Aura/Skills/`; they are currently git-ignored and are unavailable in a fresh clone without reinstalling them.
