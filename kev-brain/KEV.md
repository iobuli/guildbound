# KEV: how this research system works and how Claude works in it

kev turns a trading method (levels, swing touches, breaks, retests, daily bias, ATR) into code that can be tested honestly on any
market. This vault is its memory. The code is the method; the vault says what we know about it, where each idea came from, and what
the data showed.

## Rules that are never bent
1. **Causal only.** A decision uses closed bars and, at a fill, the traded price. Never the bar being decided, never a close that has not printed.
   Fills happen only at prices that traded. If a stop and a target share a bar, the stop wins unless finer bars show the order.
2. **Pre-register before new data.** Before testing a market or a change, write the hypothesis, the outcome, the test and the
   decision rule in `experiments/`. Leads found on one market are confirmed on another, not on the same one.
3. **Every result has a control.** Real orders are compared with matched random orders (same side, distance, window and risk), with
   day-cluster bootstrap intervals, and Benjamini-Hochberg across the questions asked together. Report effect sizes, not just p.
4. **No tuning on out-of-sample data.** The in-sample/out-of-sample split lives in each market's profile and is fixed before running.
5. **Report what failed.** A null result goes in `findings.md` like any other.
6. **Provenance.** Every rule in the code names its source (transcript timestamp, lesson, journal, tweet, or backtest), its status
   (stated / observed / inferred / backtest-reported / backtest-rejected), and whether code tests it.

## Where knowledge goes
| what | where |
|---|---|
| the user's goals and preferences | `USER.md` |
| durable facts, decisions, lessons (short) | `MEMORY.md` |
| what happened in a session | `daily/YYYY-MM-DD.md` (append only; runs log themselves) |
| a rule of the method | a node's `rules` in `src/nodes/*.ts`; `method/` notes are regenerated from code |
| a new source (text, page, transcript) | `sources/` (drop it in `inbox/` and run `kev brain process`) |
| a new price file | `inbox/`, or `kev ingest SYMBOL files...` |
| an experiment | `experiments/` before running; results appended after |
| what is known so far | `findings.md` (generated from experiment results) |

## Session protocol
- At the start the SessionStart hook prints this file, USER.md, MEMORY.md, the last two daily logs and the findings summary.
- Before answering a question about past work or the method, search: `kev brain search "..."` (or the `kev_brain_search` MCP tool).
- At the end of a task, append a short summary to today's daily log (`kev brain log "..."`), and update MEMORY.md if a durable fact changed.
- The only parameter changes are named presets (`src/core/presets.ts`). Nothing else writes the parameters.

## Commands
`npx tsx src/cli.ts` with `ingest`, `markets`, `backtest`, `study`, `audit-pages`, `inspect`, `brain build|search|context|log|process`.
