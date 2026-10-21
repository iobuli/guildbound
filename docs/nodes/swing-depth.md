# Swing depth (swing in / swing out)

`swing-depth` · stage **L** · decides at **prior_bars** · computed

Lists swing highs and lows and measures how "deep" each is, so only significant swings can be swept. Depth is the smaller of the swing-in and swing-out moves, compared with the daily ATR.

## Ports

**Inputs**
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number

**Outputs**
- `swings`: h1: object[], d1: object[]

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **swing-depth.in-out-definition**<br>Draw a line at the swing price, run it left until it hits a candle and right until the sweep. The lowest (highest) point of each section is the swing in / swing out. Measure the SMALLER side. | hard | stated | yes | lesson: curated/taking-a-trade-h1-sweep.md (2024-10-22)<br>webinar: curated/advanced-sfp.md (2024-09-24) |
| **swing-depth.deep-threshold**<br>An H1 swing is deep at 1x ATR or more; a D1 swing needs 3x the current day's ATR. Anything smaller is ignored when it is swept.<br>*Worked examples: 57.5 against an ATR of 66.8 is NOT deep; 1018 against 568 pipettes is.* | hard | stated | yes | webinar: curated/advanced-sfp.md (2024-09-24)<br>lesson: curated/taking-a-trade-h1-sweep.md (2024-10-22) |
| **swing-depth.where-do-people-get-bearish**<br>Ask where people will get bearish (bullish): only the main swing counts, not a tiny low sitting under it. | heuristic | stated | no | webinar: curated/advanced-sfp.md (2024-09-24) |
| **swing-depth.shallow-swing-above-deep**<br>A deep swing can be ignored if another deep swing sits right above it, since that is easy extra liquidity. A recent, prominent high close by can also become the place people get bullish instead.<br>*The note-taker marked this SPECULATION.* | heuristic | inferred | no | webinar: curated/advanced-sfp.md (2024-09-24) |

## Conflicts

- `swing-depth.in-out-definition` vs `swing-depth.deep-threshold`: The notes say an H1 swing must be "1x ATR" without saying which ATR; the lesson examples measure it against the DAILY ATR. This build uses the D1 ATR for both (x1 for H1, x3 for D1).
