# Proposal protocol (multi-agent batch, 2026-10-21)

Each proposal is ONE file `proposals/P_<ID>.py` (ID: uppercase letters/digits, e.g. `FVG1`). It must define:

```python
PREREG = dict(
    id="FVG1",                    # must match the file name
    family="fair-value-gap retest",
    desc="exact, complete trading rule in words",
    rationale="why this could exist (market microstructure / behaviour)",
    novelty="how it differs from the 24 methods in REPORT.md",
    data_informed=False,          # True if it was shaped by any result you have seen (e.g. inverting a failed method)
    H=24,                         # time exit in H1 bars (max 240)
    pad=0.25,                     # stop distance beyond `ext`, in ATR units (stop = ext - side*pad*atr)
)

def generate(df, name):
    """df: H1 OHLCV DataFrame (index = bar OPEN time; columns open, high, low, close, volume), name in
    {"USDJPY_H1","SOLUSDT_H1","XAUUSD_H1","XAGUSD_H1"}. Return a DataFrame indexed by signal-bar time t with columns:
      i    integer position of the signal bar in df (entry = OPEN of bar i+1)
      side +1 long / -1 short
      ext  stop anchor price (stop = ext - side*pad*atr); must be on the losing side of the entry
      atr  ATR known at the close of bar i (e.g. sweep_lib.atr(df).values[i])
    optional:
      tp   take-profit price (hit -> exit at tp)
      w    position size weight > 0, known at close of bar i (default 1). Weighted mean R is reported.
    CAUSALITY: everything for row i may use only bars 0..i (bar i is closed at signal time). No future bars, no
    full-sample statistics (rolling/expanding only), no centred windows, no .shift(-k)."""
```

Allowed helpers: `from sweep_lib import atr, levels, events` (all causal). You may resample H1 to D1/W1 inside
`generate` only if a higher-timeframe bar is used after it has CLOSED (e.g. groupby day, then `.shift(1)`).

## Commands (run from /home/user/fx_kev/sweep)
- `python3 eval_proposal.py check <ID>`: causality corruption test (10 events per instrument, all bars after t replaced
  with noise; signals at t must not change) + event counts per instrument and era. Shows NO profit or loss. Use as often as you like.
- `python3 eval_proposal.py register <ID>`: freezes the file (SHA-256) into `proposals/registry.jsonl`. After this the file
  must not change; evaluation refuses a modified or re-registered file.
- `python3 eval_proposal.py evaluate <ID>`: evaluates ONCE, pre-2026 only, appends to `proposals/results.jsonl`, prints JSON.
  A second call prints the stored result and does not recompute.

Designers: do NOT compute profit/loss, returns or win rates for your proposal or any variant before registering.
Costs (round trip): USDJPY 1 pip, SOL 0.10%, gold 0.025%, silver 0.05%. Stops are checked before targets inside a bar.

## Pass rule (same bar as ledger.py, fixed)
P1 pooled pre-2026 block-bootstrap 95% CI lower bound > 0 on USDJPY AND SOL; P2 mean > 0 in USDJPY eras 2011-15, 2016-20,
2021-25 and SOL 2021-25; P3 one-sided bootstrap p < 0.05 / N (N = all methods ever registered, at least 40); P5 mean > 0 in
both 2018-21 and 2022-25 on gold AND silver. 2026 (P4) is unsealed only by the lead, after adversarial review.
