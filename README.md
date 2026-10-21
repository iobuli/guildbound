# FX / crypto / metals research with Kev (backup branch)

Backup of the trading-research workspace. Orphan branch: it shares no history with the game (`build/bevy`).

## Layout
- `fx_kev/` - all code, results and logs.
  - `fx_kev/sweep/REPORT.md` - the main log: 24 pre-registered level / liquidity-sweep / Kev methods, none passed.
  - `fx_kev/sweep/ledger.py` - every method's frozen definition, written before it was evaluated.
  - `fx_kev/sweep/results_v2.jsonl` - one result line per method (never overwritten).
  - `fx_kev/sweep/proposals/` - multi-agent batch (2026-10-21): one file per proposal, `registry.jsonl` (file hashes frozen at registration), `results.jsonl`.
  - `fx_kev/sweep/eval_proposal.py` - harness: check / register / evaluate / unseal.
  - `fx_kev/sweep/k4_*` - Kev-0.8B fine-tune on pre-2021 sweep outcomes (training data, eval sample, eval script).
  - `fx_kev/blind/` - brief, results and frozen rules from the blind discovery agent (its input data is re-derivable from `data/`).
  - Earlier work: `fx_kev/*.py` (Ichimoku grids, pattern study, exit study, hierarchical Kev).
- `data/raw_uploads/` - the original uploaded files (tab-separated, no header: time, open, high, low, close, volume).
- `data/holdout_2026_SEALED/` - all bars from 2026-01-01 on, split off and NOT used for any design. Only open it to test a
  method that has already passed every pre-2026 check (see the pass rule in `fx_kev/sweep/ledger.py`).

## Re-create the working layout
The scripts expect `/home/user/fx_kev/blind/data/<NAME>.csv` (bars before 2026, comma-separated with a header) and the
held-out files under the scratchpad path set in `fx_kev/sweep/sweep_lib.py` (`HOLD`). Split `data/raw_uploads` at 2026-01-01
the same way (metals are resampled from M15 to H1) or point `DATA` / `HOLD` in `sweep_lib.py` at your own copies.
