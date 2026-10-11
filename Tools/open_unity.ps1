# Opens the Guildbound project in Unity 6.3 with the MCP for Unity bridge started in stdio mode, so Claude Code,
# Codex (unityMCP) and Aura (aura-unity + mcp-for-unity) can all reach the live editor without clicking Start Session.
# Usage: powershell -NoProfile -File Tools/open_unity.ps1
$project = Split-Path -Parent $PSScriptRoot
if (Get-Process -Name Unity -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like 'guildbound*' }) {
    Write-Output 'Unity already has the project open.'
    exit 0
}
Start-Process -FilePath 'D:\Unity\Editors\6000.3.26f1\Editor\Unity.exe' -ArgumentList @('-projectPath', "`"$project`"", '-executeMethod', 'MCPForUnity.Editor.McpCiBoot.StartStdioForCi')
Write-Output 'Unity starting with the MCP bridge (stdio).'
