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

## Findings v2 (2026-10-21, pre-registered, see findings.md)
- Fills at kev's levels beat matched random orders in every market: SOL +0.09R, ETH +0.05R, gold +0.06R (p ~0.003), but every market is net negative
  after costs (-0.07 to -0.13R at stop 0.5 daily ATR, 2R).
- H1 space CONFIRMED on fresh data: fills with 0.5+ ATR of space (prior bars only) beat their placebos by +0.17 SOL, +0.15 ETH, +0.10 gold, in both halves.
  Net it is still about breakeven on SOL (+0.03R, CI spans 0) and negative on ETH and gold. Part of the lift is random orders doing worse in those windows.
- H2: touch count adds nothing, under either touch rule.
- H3: the "fill bar did not close through" filter lifts RANDOM orders from about -0.31R to about 0R in all three markets: it is how the user's pages got their edge.
- H4: worst-case intrabar rules understate structural-stop results by ~0.06R; negligible for 0.5-ATR stops.
- H5: gold's Sunday stubs made the daily ATR 16% too small and added 18% more fills, but did not change the conclusion. Crypto: NY-close vs UTC days makes no difference.
- SOL "GO at fill" looked good (+0.20R, n 208, descriptive) but does not replicate (ETH -0.10, gold -0.26).

## Atlas SOLUSDT 2020-2022 (2026-10-21; experiments/atlas-SOLUSDT-2020-2022.md)
- 467 levels kev marked; median life about 1 day; 48% never revisited; first retests held 63% [hindsight].
- Fills beat random orders in all 5 phases (+0.03 to +0.11R, best in the 2021 run) but net is about 0 or below in every phase [causal].
- Nothing known at first sight picks the levels that later hold 3+ tests (AUC 0.38-0.58); more touches at first sight is slightly WORSE.
- "With-trend levels hold better" (68% vs 53%) only works with hindsight phase direction; the causal regime trend gives AUC 0.44.
- The regime node's ATR-normalised trend calls crashes "range" (all of May-Jul 2021, 60% of 2022): a known weakness, not tuned.

## Audit + paper book (2026-10-21; experiments/2026-10-21-kev-audit.md, 2026-10-21-paper-book.md)
- Audit: added causal percent trend `trendPct` (regime node) since the ATR trend missed crashes; atlas last leg fixed; engine placebos now keep the real trade's GO label (they had go:false, so any GO-gated placebo book was empty).
- Paper book, $100k, 0.5% risk, rules frozen on 2020-2022 (D GO chosen by deflated Sharpe, all 4 candidates lost): 2023-now $83,079, Sharpe -0.79, maxDD -24%; placebo book $65,610. Beats random by ~0.25R/trade, still loses; gold -0.40R is most of it.
- Hurst ~0.55 / VR(24h) ~1 in every market-year: no exploitable regime persistence at H1.
- Iteration 1 (walk-forward self-selection, 50 configs/market, DSR): at DSR>=0.9 it never trades; at 0.5 only SOL GO trades, $98,118 vs random $95,813. In-sample SOL +0.2-0.47R decays to about 0 out of sample; ETH and gold have nothing. Live choice: sit out.
- Pattern lab 2020-2022 (12,327 H1 events; experiments/2026-10-21-pattern-lab.md): T3 fills, sweeps/SFP, MSB and sweep->MSB have 24h lift ~0 (|lift| <= 0.04 ATR, all CIs span 0); 70 refinements (vol, session, 50-day trend), none pass; quarterly lift flips sign. Faint leads: fading the 50-day move (+-0.05 ATR), below costs. kev's limit-order edge is the limit fill itself, not post-close information.
- Nucleus (SOL 1m 2020-11..2022, 6 TFs x n-n-n lines n=9/26/52; experiments/2026-10-21-nucleus.md): JS/Python bit-identical, no lookahead (tests). Trace: nothing tells where the hour closes; weak contrarian rho (~-0.02, AUC ~0.48) in 2022 only, mostly 1m-15m and slopes (part bid-ask bounce); 4h/1D never above the noise band; no state has q<0.05.
- Binance 1m archive fills no-trade minutes with flat bars at the last close (v=0; 10.5% of 2020 minutes): they set the open (sometimes high/low) of replayed 2020 bars, which is why 209 hours differ from h1.json. Skipping v=0 minutes for open/high/low gives 20,919/20,921 matches. Not yet applied to the nucleus.
- Nucleus review to-dos (no lookahead found; the trace's features are unaffected): 1D n=52 shift reads 0 for 26 valid days in Oct-Nov 2020 (needs a sentinel or J>=2n-1 in valid); the warm-up should count 106 existing days; closed-line cmid/cpos not exposed yet; equal mids are ordered shorter-first, which favours the bull stack (10-16% of minutes are ties; make it symmetric).
- kev-lab/ (Python stdlib server + JS page) reads out/signals/*.json from `kev signals`; tests in kev-lab/tests.

## Findings before v2 (superseded where the v2 runs disagree)
- No net edge on SOL or gold (H4/H1/M30) after costs; real levels beat random orders by about +0.1R but only reach breakeven gross.
- The user's Trade Check keep/reject rule is exactly "the fill bar closed through the level" (0 of 35 kept, 8 of 8 rejected): lookahead.
- The user's One Hour `far` (space) includes the decision bar's own high/low on 79 of 79 decisions.
- Leads (not confirmed): space measured from prior bars (+0.09R over placebo on SOL, n.s.); One Hour T1 excursion (rank corr +0.28 with outcome, n=79).

## Decisions
- Local git only; no heartbeat; presets are the only way to change parameters; leads from SOL are confirmed on ETH and gold.

## Lessons
- Check a claim against the data before reporting it (the "harness ignores space" claim was wrong: space only gated GO, not fills).
- Do not guess another engine's parameters: recover them from its data (One Hour's touch rule was recovered: 3-bar swings within 0.1 ATR, 12+ bars apart).
