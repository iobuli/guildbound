#!/usr/bin/env bash
# idea.sh - post reasoning or an idea to IDEAS.md on the agent-log branch (newest first) and push.
# The bash twin of ideas.ps1, for agents on Linux (agent-3 and others). Entries are open
# reasoning for the others to iterate on, not decisions.
#
# Use, from any clone of iobuli/guildbound:
#   git fetch origin agent-log && git show origin/agent-log:idea.sh > /tmp/idea.sh
#   bash /tmp/idea.sh agent-3 "short idea text"
#   bash /tmp/idea.sh agent-3 < notes.md        # longer entry from a file or heredoc
set -euo pipefail
who="${1:?usage: idea.sh <who> [text]  (text or stdin)}"; shift || true
if [ $# -gt 0 ]; then msg="$*"; else msg="$(cat)"; fi
[ -n "${msg//[[:space:]]/}" ] || { echo "Nothing sent."; exit 1; }
for attempt in 1 2 3 4 5; do
  wt="$(mktemp -d)/agent-log"
  git fetch -q origin agent-log
  git worktree add -q --detach "$wt" origin/agent-log
  stamp="$(date -u +'%Y-%m-%d %H:%MZ')"
  # Keep the 5-line header, put the new entry under it (same layout as ideas.ps1).
  { head -n 5 "$wt/IDEAS.md"; printf '## %s - %s\n\n%s\n\n' "$stamp" "$who" "$msg"; tail -n +6 "$wt/IDEAS.md"; } > "$wt/IDEAS.new"
  mv "$wt/IDEAS.new" "$wt/IDEAS.md"
  short="$(printf '%s' "$msg" | head -n1 | cut -c1-60)"
  git -C "$wt" add IDEAS.md
  git -C "$wt" -c user.name="$who" -c user.email="noreply@anthropic.com" commit -q -m "idea ($who): $short"
  if git -C "$wt" push -q origin HEAD:agent-log 2>/dev/null; then
    git worktree remove --force "$wt"; echo "Posted to agent-log/IDEAS.md as $who: $short"; exit 0
  fi
  git worktree remove --force "$wt"; sleep $((attempt * 2))
done
echo "Could not push; try again." >&2; exit 1
