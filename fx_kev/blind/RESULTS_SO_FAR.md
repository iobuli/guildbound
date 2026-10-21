# Results so far (facts only, data up to 2026-09; the agent's folder contains data up to 2025-12-31)

## SOLUSDT H1, Ichimoku grid (train 50%/val 25%/test 25%, 0.1% round trip)
Best train/val config (Tenkan 5, Kijun 40, SpanB 52, price above cloud, future cloud bullish, long-only, 4xATR stop, 48-bar max hold):
train +1088% (Sharpe 2.65), val +238% (2.58), test -13.6% (-0.13). Defaults 9/26/52 on test: -42.8% unfiltered, -39.1% with all filters.
Buy&hold: train +589%, val +637%, test -28%.

## USDJPY Ichimoku grid (cost about 1 pip)
D1: chosen config test +3.7% (13 trades, Sharpe 0.44); default 9/26/52 unfiltered L+S test +27% (27 trades).
H1: chosen (12,26,52, price above cloud, long-only, 4xATR stop, 48h) test +10.6%, 214 trades, Sharpe 1.03, maxDD 6.1%; default unfiltered L+S test +12.7%.

## Pattern study, 4,558 H1 TK crosses 2011-2026 (R = profit in H1 ATR units)
Mean R train/val/test: 0.165 / 0.387 / 0.305. Win about 45%. Winners: mean MFE 6.8 ATR, median time to peak 37h. Losers: mean MAE 4.3, median peak 5h (stop at 4 ATR).
Longs R 0.34/0.55/0.51; shorts -0.01/0.23/0.09. Session, weekday, RSI/ATR/cloud-distance quartiles: inconsistent sign across splits.
Future-cloud filter: train +0.50 vs -0.09, test +0.01 vs +0.56 (reversed). Sunday-open entries positive in all splits but only 145 trades.

## Exit rules (net mean R per trade, long-only, train/val/test)
baseline 48h/4ATR stop 0.27/0.47/0.46; hold 96h 0.50/0.65/0.59; trail 4 ATR after +2 ATR 0.53/0.57/0.82; early exit at 6h if underwater 0.19/0.40/0.23.
Wider/trailing beat tight/early in all splits; early-failure exits hurt.

## Kev-0.8B (zero-shot, text snapshots of D1/H1/M15 state), 600 crosses, 2022-09 to 2026
AUC by level 0.52-0.57 (about chance). Spearman(Kev score, realized R) between -0.28 and +0.09 across splits; not monotonic; sizing by Kev percent
did not improve Sharpe on test (95% CI of gain contains 0 or is negative).

## Lookahead audit
Corrupting all bars after t left the text snapshots unchanged (12/12); an injected one-bar leak was caught.
