# ATR yardsticks

`atr` · stage **X** · decides at **prior_bars** · computed

Measures volatility on D1 and H1. Almost every other node sizes things in ATR, so this is the only place ATR is computed.

## Ports

**Inputs**
- none

**Outputs**
- `atr`: d1: number, h1: number, h1_24: number

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **atr.d1-is-the-yardstick**<br>The daily ATR is the yardstick for what counts as a deep swing, how close a fib must be, and whether the market has already "passed ATR". | hard | stated | no | lesson: curated/taking-a-trade-h1-sweep.md (2024-10-22)<br>webinar: curated/advanced-sfp.md (2024-09-24) |
| **atr.h1-24-for-sfp-stop**<br>The H1 SFP stop uses the 24-period ATR taken at the close of the sweep candle. | hard | stated | yes | webinar: user-pasted SFP notes (2024-09-24) |
| **atr.m5-uses-12**<br>M5 SFPs use a 12-period ATR instead.<br>*No M5 data in this build, so not computed.* | hard | stated | no | webinar: curated/levels.md (2024-09-28) |
| **atr.d1-period-20**<br>The daily ATR is a 20-period ATR, because there are about 20 trading days in a month: it averages how far the market moves in a day over the last month. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @43:34 |
| **atr.use-the-day-before**<br>Read the ATR of the day BEFORE the day in question (hover over the previous daily bar), not the day that triggers the trade.<br>*Closed daily bars only, so the forming day is never included. This also keeps it causal.* | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @44:21 |
| **atr.h1-length-unspecified**<br>No source gives an H1 ATR length. This build uses Wilder 14 as a scale only (zone width, swing size). The 24-period value is stated for the SFP stop. | heuristic | inferred | yes | webinar: curated/advanced-sfp.md |
