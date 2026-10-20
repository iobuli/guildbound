# Guildbound second brain: the map

Shared knowledge base for every agent on Guildbound (Rust, Bevy 0.19, repo iobuli/guildbound).
Use it to answer questions about the project. Do not invent info; if something isn't here or in
the code, say "not found".

## Navigation
- `MEMORY.md`      : long-term core facts (the user, the rules, the stack). Read first.
- `decisions.md`   : log of key decisions: the what and the why. Append only.
- `LEARNINGS.md`   : lessons and past mistakes. Append only.
- `projects/`      : one file per workstream (event): status, owner, branch, next step.
- `notes/`         : facts, one topic per file (systems, tools, how-tos).
- `IDEAS.md`       : the user's ideas (`ps <text>` from their PC). Read every turn.
- `LOG.md`         : per-change entries with files and lines (`post.sh`).
- `CONTEXT.md`, `WORKPLAN.md`, `GUILDHOUSE.md`: older long-form docs; facts move into the
  folders above over time.

## Protocol
- One file per topic. Names: lowercase with hyphens (`monk-deflect.md`, not `notes-2.md`).
- Answer from these files and the code; cite the file. If missing, say "not found".
- Append new facts to the existing file for that topic; do not create duplicates.
- A decision → one line in `decisions.md` (date, who, what, why). A lesson → `LEARNINGS.md`.
- A project's status changes → update its file in `projects/` in the same push as the code.
