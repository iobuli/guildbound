# Memory

Durable facts, decisions and lessons. Keep it short; details live in the linked notes.

## Data
- Markets ingested (see `markets/`): SOLUSDT (Binance spot H1, 2020-08 to 2026-10), ETHUSD (broker M1/M5/M15/M30/H4, 2018-01 to 2026-10, no H1 file:
  H1 assembled), XAUUSD (broker M30/H1/H4/D1, 2009-10 to 2026-07). All three clocks are UTC (ETH and SOL checked against Binance; gold from its weekly reopen).
- ETH at this broker was closed at weekends until 2020 and trades daily since. Its tick-volume scale changes about 110x between years.
- The user's SOL daily CSV is a different feed from Binance (closes match on 14% of days); it is a cross-check only.
- Trade Check page candles match Binance until 2023-03, then a different feed.

## Fixed in v2 (2026-10-21)
- Daily bars were UTC calendar days: gold had a 2-3h Sunday stub on 16.5% of days, inside the 20-day ATR. Now NY-close days for FX/metals,
  UTC days for crypto, and stubs after a weekend close are merged into Monday.
- Two level parsers (graph vs study) became one, switched only by presets.
- The old permutation p-value (clusterDiff) was far too conservative whenever groups share days (matched placebos, rule pass/fail splits):
  earlier "no rule is significant" results were partly that. Replaced by a day-cluster bootstrap (clusterDiffBoot).
- Fills were never gated by the graph's verdict; every trade now carries GO, space and touches so gated results are reported directly.
- Intrabar order was always the worst case; finer bars now decide when they exist (ETH M1-M30, gold M30), with the conservative result kept beside it.

## Findings before v2 (to be re-checked under v2)
- No net edge on SOL or gold (H4/H1/M30) after costs; real levels beat random orders by about +0.1R but only reach breakeven gross.
- The user's Trade Check keep/reject rule is exactly "the fill bar closed through the level" (0 of 35 kept, 8 of 8 rejected): lookahead.
- The user's One Hour `far` (space) includes the decision bar's own high/low on 79 of 79 decisions.
- Leads (not confirmed): space measured from prior bars (+0.09R over placebo on SOL, n.s.); One Hour T1 excursion (rank corr +0.28 with outcome, n=79).

## Decisions
- Local git only; no heartbeat; presets are the only way to change parameters; leads from SOL are confirmed on ETH and gold.

## Lessons
- Check a claim against the data before reporting it (the "harness ignores space" claim was wrong: space only gated GO, not fills).
- Do not guess another engine's parameters: recover them from its data (One Hour's touch rule was recovered: 3-bar swings within 0.1 ATR, 12+ bars apart).
