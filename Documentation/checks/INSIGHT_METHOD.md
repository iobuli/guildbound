# Insight method: how Claude keeps Codex current

Goal: every useful thing Claude learns (a measurement, a risk, a correction, a decision needed) reaches Codex in one predictable shape, is answered explicitly, and cannot silently go stale. One channel only: `TANDEM_REASONING.log`.

## 1. The unit: an insight
Seven fields, all required. Written with `insight.py add`.

| Field | Meaning |
|---|---|
| id | `I-001`... assigned by the tool |
| type | FINDING (measured/observed), RISK (could hurt later), DECISION (someone must choose), CORRECTION (earlier claim was wrong), HANDOFF (work moves to Codex) |
| title | one sentence, the claim itself |
| evidence | a path, a number or a quoted line Codex can open (no evidence = it is an opinion, mark `hypothesis`) |
| impact | which GDD section, kit entry, asset or check it touches |
| ask | the exact action or answer wanted from Codex |
| confidence | `verified` (measured/seen), `inferred` (reasoned from evidence), `hypothesis` (a guess to test) |

Status flow: `open` -> `accepted` | `countered` | `closed`. An open insight is re-listed in every digest until it is answered.

## 2. The cycle (every work step)
1. **Capture**: when a step produces something Codex must know, run `insight.py add ...` before moving on. Corrections of my own earlier claims are always logged as CORRECTION, never edited away.
2. **Check**: run `design_check.py` so the digest carries a current TOTAL line.
3. **Digest + post**: `insight.py post --title "<topic>"` appends one `## [claude] ... | INSIGHT | ...` entry to the log containing: (1) open insights, (2) my log entries since Codex's last entry, (3) the check totals, (4) recent commits, (5) reply instructions.
4. **Reply**: Codex answers per id on one line (`I-004 ACCEPT`, `I-005 COUNTER: reason + evidence`, `I-006 CLOSE: where it is already done`).
5. **Record**: Claude runs `insight.py ack <id> accepted|countered|closed --note "..."`. A COUNTER becomes a normal DUEL if it is not settled in one exchange; unresolved after that goes to the boss.

## 3. Rules
- Cadence: one digest per work step; batch at most 5 new insights per digest; more means the step was too big.
- Never bury an insight in prose. If it is not in the ledger it has not been delivered.
- Honesty marks: say what a number does NOT prove (e.g. a Blender render is not Unity proof; an empty test measures nothing).
- A CORRECTION cites the earlier entry it supersedes.
- Codex's counter-insights use the same shape with `[codex]` as the agent; Claude answers them the same way.
- The ledger (`insights.json`) is append-and-update; the log stays append-only (check C16). Check C17 warns when more than 5 insights stay open.
- Anything that needs the boss is a DECISION-type insight with the two options stated, and is also surfaced in the STATE block under `decisions_pending_boss`.

## 4. Quick commands
```
python -I Documentation/checks/insight.py add --type RISK --title "..." --evidence "..." --impact "GDD S9" --ask "..." --confidence inferred
python -I Documentation/checks/insight.py list --open
python -I Documentation/checks/insight.py digest      # preview, writes nothing to the log
python -I Documentation/checks/insight.py post --title "after HUD review"
python -I Documentation/checks/insight.py ack I-004 accepted --note "agreed, applied in AMEND-UI-01"
```
