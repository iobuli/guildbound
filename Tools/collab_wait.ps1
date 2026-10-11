# Blocks until the OTHER agent appends a new entry to collab/CHANNEL.md, then prints the new entries.
# Usage: powershell -NoProfile -File tools/collab_wait.ps1 -Me codex   (or -Me claude)
# Exit codes: 0 = new message printed, 2 = user STOP posted, 3 = timed out (just run it again).
# "Seen" state lives in collab/.seen_<me> (the other agent's heading count), so nothing posted
# between two waits is missed.
param(
    [Parameter(Mandatory = $true)][ValidateSet('claude', 'codex')][string]$Me,
    [int]$TimeoutMin = 9,
    [int]$PollSec = 10
)

$root = Split-Path -Parent $PSScriptRoot
$channel = Join-Path $root 'collab/CHANNEL.md'
$state = Join-Path $root "collab/.seen_$Me"
$other = if ($Me -eq 'claude') { 'codex' } else { 'claude' }
$headPattern = "^## \[$other\]"

function Get-Lines { Get-Content -LiteralPath $channel -Encoding UTF8 }
function Get-Count([string[]]$lines) { @($lines | Where-Object { $_ -match $headPattern }).Count }

$lines = Get-Lines
$seen = if (Test-Path $state) { [int](Get-Content $state) } else { Get-Count $lines }
Set-Content -LiteralPath $state -Value $seen  # first run: start from now, and persist so later runs miss nothing
$deadline = (Get-Date).AddMinutes($TimeoutMin)

while ((Get-Date) -lt $deadline) {
    $lines = Get-Lines
    if ($lines | Where-Object { $_ -match '^## \[user\].*\bSTOP\b' }) {
        Write-Output 'STOP posted by user in collab/CHANNEL.md. Finish the current step, post status, do not wait again.'
        exit 2
    }
    $count = Get-Count $lines
    if ($count -gt $seen) {
        # Print every entry from the other agent after the first $seen of them.
        $n = 0; $printing = $false
        foreach ($line in $lines) {
            if ($line -match '^## \[') {
                $printing = $false
                if ($line -match $headPattern) { $n++; if ($n -gt $seen) { $printing = $true } }
            }
            if ($printing) { Write-Output $line }
        }
        Set-Content -LiteralPath $state -Value $count
        exit 0
    }
    Start-Sleep -Seconds $PollSec
}
Write-Output "No new [$other] entry within $TimeoutMin min. Run again to keep waiting."
exit 3
