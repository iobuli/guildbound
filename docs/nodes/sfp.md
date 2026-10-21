# Swing failure pattern (SFP)

`sfp` · stage **L** · decides at **prior_bars** · computed

Finds sweeps of deep swings that close back inside, applies the visit-count, size and invalidation conditions, and says how the entry and stop should be set.

## Ports

**Inputs**
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number
- `swings` (from swing-depth.swings): h1: object[], d1: object[]

**Outputs**
- `sfp`: h1: object[], d1: object[]

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **sfp.definition**<br>An SFP is essentially a false break of a high or low, but with strict criteria for what it must look like and when it is traded. | hard | stated | no | tweet: curated tweets (2026-05-22) |
| **sfp.deep-swing-only**<br>Only sweeps of significant (deep) swings count. Sweeping a shallow swing is ignored. | hard | stated | yes | webinar: curated/advanced-sfp.md (2024-09-24)<br>lesson: curated/taking-a-trade-h1-sweep.md (2024-10-22) |
| **sfp.first-sweep-only**<br>Only the first sweep of a level is tradeable. The second or any later sweep is invalid. A fresh sweep in a new area is fine. | hard | stated | yes | lesson: curated/taking-a-trade-h1-sweep.md (2024-10-22) |
| **sfp.visit-count**<br>Count visits to the area: the 1st creates the swing, the 2nd needs a sweep, the 3rd needs a sweep AND a MACD divergence, the 4th is a no. Tests on the daily count toward the hourly.<br>*The MACD divergence is flagged (needsDivergence) but not computed here.* | hard | stated | yes | webinar: curated/advanced-sfp.md (2024-09-24)<br>webinar: curated/advanced-sfp.md (2024-09-26) |
| **sfp.time-and-space**<br>There must be time and space between the original swing and the SFP, as with an S/R retest. Swing levels stay valid for about 2 days. | hard | stated | yes | webinar: user-pasted SFP notes (2024-09-24) |
| **sfp.enter-at-close**<br>On an H1 sweep that closes back inside, sell (buy) at the current price the moment the candle closes. | hard | stated | yes | lesson: curated/taking-a-trade-h1-sweep.md (2024-10-22) |
| **sfp.strong-close-wait**<br>If the sweep bar is wide and closes within 20% of its extreme, do not enter at the close. Draw the ATR stop; if price moves about half way toward it and turns back, place a stop order at the original SFP close. | hard | stated | yes | webinar: curated/advanced-sfp.md (2024-09-24)<br>webinar: user-pasted SFP notes (2024-09-24) |
| **sfp.stop-atr24**<br>Stop = the sweep extreme plus the 24-period ATR. The "advanced" entry uses the sweep extreme itself. | hard | stated | yes | webinar: user-pasted SFP notes (2024-09-24) |
| **sfp.invalidation**<br>If a candle closes beyond the SFP candle itself, the SFP is invalid and an open trade is exited at once. | hard | stated | yes | webinar: user-pasted SFP notes (2024-09-24) |
| **sfp.small-bars-meaningless**<br>A sweep candle that is tiny (a 6-pip Asian-session bar) is unlikely to mean anything.<br>*The 0.5 ATR cut-off is the builder's; the source gives only the example.* | hard | inferred | yes | webinar: curated/advanced-sfp.md (2024-09-24) |
| **sfp.distrust-session-close**<br>On the FTSE he does not trust SFPs forming at the close of the market (volatility from closing positions). Less of a concern in forex. | hard | stated | no | webinar: curated/advanced-sfp.md (2024-09-24) |
| **sfp.ramp-into-sweep**<br>A short is not ideal if price ramps up bullishly into the sweep. Better if the SFP candle breaks back under prior support, for example as a bearish engulfing. | preference | observed | no | webinar: curated/advanced-sfp.md (2025-01-21) |
| **sfp.opposite-sfp-closes-trades**<br>An H1 SFP against an open trade is a reason to close it, but give price an hour or two to show its hand. | hard | stated | no | webinar: user-pasted SFP notes (2024-09-24) |
| **sfp.enter-at-the-swept-level**<br>Newer practice: enter at the swept level itself or the nearest naked level, not blindly at the SFP close. "Stopped out right on the low that was SFP'd, which is where you should have been looking to enter." | hard | observed | no | tweet: curated tweets (2026-07-01)<br>tweet: curated tweets (2022-10-21) |
| **sfp.average-rr**<br>Average RR on SFP trades is about 1.2 with a high hit rate; he takes them from 0.8R and sometimes 0.5R when probability is high.<br>*Hit rate (~90%) is a self-reported figure, not tested here.* | heuristic | stated | no | webinar: user-pasted SFP notes (2024-09-24) |
| **sfp.not-at-inverse-levels**<br>An SFP is not relevant at a level that is the inverse last: an SFP presumes the level was support (resistance) last, and that is the other kind of trade. If you get an SFP at a level, you are in the same-direction case. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @56:53 |

## Conflicts

- `sfp.enter-at-close` vs `sfp.enter-at-the-swept-level`: The 2024 course teaches selling at the SFP candle close. The 2022 and 2026 posts prefer entering at the swept level / nearest naked level. Both are kept; the entry node reports whichever the setup style asks for.
