#!/usr/bin/env bash
# Post an entry to the agent-log branch: which agent, which commits, which files and lines.
# Usage: post.sh <agent> <from>..<to> "<what it does>" ["<config / how to try it>"]
# Run from inside a clone of iobuli/guildbound (any branch). Needs push access.
set -euo pipefail
agent=${1:?agent name}; range=${2:?from..to}; note=${3:?note}; config=${4:-}
repo=$(git rev-parse --show-toplevel)
cd "$repo"
git fetch -q origin agent-log

# Changed line ranges per file in the new version, from the zero-context diff.
ranges() {
  git diff -U0 "$1^" "$1" -- | awk '
    /^\+\+\+ b\// { if (f != "") print f ": " r; f = substr($0, 7); r = ""; n = 0; next }
    /^\+\+\+ \/dev\/null/ { if (f != "") print f ": " r; f = ""; next }
    /^@@/ {
      split($3, a, ","); s = substr(a[1], 2) + 0; c = (a[2] == "" ? 1 : a[2] + 0)
      n++
      if (n <= 12) r = r (r == "" ? "" : " ") (c == 0 ? "-" s : "+" s (c > 1 ? "-" s + c - 1 : ""))
      else if (n == 13) r = r " …"
    }
    END { if (f != "") print f ": " r }'
}

entry=$(mktemp)
{
  echo "## $(date -u +%Y-%m-%dT%H:%MZ) · $agent"
  echo
  branches=$(git branch -r --contains "${range#*..}" 2>/dev/null | grep -v agent-log | sed 's/^ *origin\///' | tr '\n' ' ')
  echo "**Branch:** ${branches:-(not pushed)}  ·  **Range:** \`$range\`"
  echo
  echo "**What:** $note"
  [ -n "$config" ] && { echo; echo "**Config / try it:** $config"; }
  echo
  for c in $(git rev-list --reverse "$range"); do
    echo "- \`$(git rev-parse --short "$c")\` $(git log -1 --format=%s "$c")"
    ranges "$c" | sed 's/^/    - /'
  done
  echo
} > "$entry"

work=$(mktemp -d)
git worktree add -q "$work" origin/agent-log --detach
trap 'git worktree remove --force "$work" >/dev/null 2>&1 || true; rm -f "$entry"' EXIT
for attempt in 1 2 3 4 5; do
  ( cd "$work"
    git fetch -q origin agent-log && git reset -q --hard origin/agent-log
    { head -n 2 LOG.md; cat "$entry"; tail -n +3 LOG.md; } > LOG.new && mv LOG.new LOG.md
    git add LOG.md
    git -c user.name="$agent" -c user.email="noreply@anthropic.com" commit -qm "log: $agent $range"
    git push -q origin HEAD:agent-log ) && { echo "posted to agent-log"; cat "$entry"; exit 0; }
  sleep $((attempt * 2))
done
echo "could not push the log entry" >&2
exit 1
