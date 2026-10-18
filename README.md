# agent-log: the text-only update board

This branch holds no game code. It is where every agent working on Guildbound says what it
changed, so the others (and you) can see it at a glance without reading diffs.

- `LOG.md`: newest entry first. One entry per change (a commit or a short run of commits).
- `post.sh`: writes an entry from git itself (files and exact line ranges), commits it here and
  pushes. Run it after each change you push.

## Agents

| Name    | Who                                   | Works on                                             |
|---------|---------------------------------------|------------------------------------------------------|
| agent-1 | wardrobe / cast session               | hero bodies, garments, textures, appearance.json     |
| agent-2 | combat / motion session (this one)    | Monk, discipline, combat, AI, climbing, mocap, hands |
| helper  | agent-2's helper (worktree)           | bag (satchel), crafting, debug stats panel           |

## Posting an entry

From any clone of `iobuli/guildbound`, after pushing your work:

```bash
git fetch origin agent-log
git show origin/agent-log:post.sh > /tmp/post.sh
bash /tmp/post.sh <agent> <from>..<to> "<what it does>" ["<how to configure / try it>"]
# e.g.
bash /tmp/post.sh agent-2 26c5552..e9ecf30 "Monk class and counters" "Class panel key 4; GB_CLASS=monk; GB_RANK=6"
```

The script lists, for every commit in the range: hash, branch, subject, and each changed file
with its changed line ranges in the new version (`+12-30` means lines 12..30 now; `-` alone means
lines removed there). Then your note and config. It retries the push if another agent posted at
the same time.

## Reading

```bash
git fetch origin agent-log && git show origin/agent-log:LOG.md | head -80
```

or open `LOG.md` on GitHub, branch `agent-log`.

## Rules

1. Post after every push to a work branch (`overhaul-2026-10-16`, `feat-*`).
2. Say which branch others should pull or fetch, and anything they must do first.
3. Keep notes short. The line ranges carry the detail.
