# ideas.ps1 - send an idea to every Guildbound agent.
# It adds your text to IDEAS.md on the agent-log branch (newest first) and pushes it.
# Every agent reads IDEAS.md at the start of each turn.
#
# Install once (PowerShell):
#   cd D:\gb_overhaul; git fetch origin agent-log
#   git show origin/agent-log:ideas.ps1 | Set-Content -Encoding UTF8 $HOME\gb-ideas.ps1
#   Add-Content $PROFILE '. $HOME\gb-ideas.ps1'
#   . $HOME\gb-ideas.ps1          # or open a new PowerShell window
#
# Use:
#   ps monks should hum before a seize
#   idea "the ruins need fog at dawn"
#   ps                                # no text: opens a prompt to type a longer idea

$GbRepo = "D:\gb_overhaul"

function Send-Idea {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Text)
    $msg = ($Text -join ' ').Trim()
    if (-not $msg) { $msg = (Read-Host "Idea").Trim() }
    if (-not $msg) { Write-Host "Nothing sent."; return }
    $utf8 = New-Object System.Text.UTF8Encoding $false
    Push-Location $GbRepo
    try {
        for ($attempt = 1; $attempt -le 5; $attempt++) {
            $wt = Join-Path $env:TEMP ("gb-idea-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
            git fetch -q origin agent-log
            git worktree add -q --detach $wt origin/agent-log 2>$null
            $file = Join-Path $wt "IDEAS.md"
            $lines = [System.IO.File]::ReadAllLines($file, $utf8)
            $stamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd HH:mm") + "Z"
            # Keep the 5-line header, put the new idea under it.
            $head = $lines[0..4] -join "`n"
            $rest = if ($lines.Count -gt 5) { $lines[5..($lines.Count - 1)] -join "`n" } else { "" }
            $entry = "## $stamp - user`n`n$msg`n"
            [System.IO.File]::WriteAllText($file, "$head`n$entry`n$rest`n", $utf8)
            git -C $wt add IDEAS.md
            $short = if ($msg.Length -gt 60) { $msg.Substring(0, 60) + "..." } else { $msg }
            git -C $wt -c user.name="user" -c user.email="noreply@anthropic.com" commit -q -m "idea: $short"
            git -C $wt push -q origin HEAD:agent-log 2>$null
            $ok = ($LASTEXITCODE -eq 0)
            git worktree remove --force $wt 2>$null
            if ($ok) { Write-Host "Sent to all agents (agent-log/IDEAS.md): $short"; return }
            Start-Sleep -Seconds ($attempt * 2)
        }
        Write-Host "Could not push the idea. Check your connection and try again."
    }
    finally { Pop-Location }
}

# `idea` always works; `ps` replaces PowerShell's built-in ps (Get-Process) in this window.
Set-Alias -Name idea -Value Send-Idea -Scope Global
Set-Alias -Name ps -Value Send-Idea -Scope Global -Option AllScope -Force
