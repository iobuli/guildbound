# Level scanner

`level-scan` · stage **L** · decides at **prior_bars** · computed

Marks the levels on the chart: clusters the swing highs and lows, drops any that price has not yet broken, and for each one on the right side of the market counts the swing touches in its previous role and says which tier it falls in.

## Ports

**Inputs**
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number
- `swings` (from swing-depth.swings): h1: object[], d1: object[]

**Outputs**
- `levels`: price: number, rows: object[]

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **level-scan.mark-levels-first**<br>Start by marking prior support/resistance flips on the weekly, daily and hourly charts, then look at where price currently is and which of those levels sit on the right side of it. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @2:44<br>transcript: levels webinar part 1, user-pasted transcript @5:25<br>transcript: levels webinar part 1, user-pasted transcript @1:02:30 |
| **level-scan.triage-by-touches**<br>Grade every level by its number of swing touches, which fixes the plan for it: 3 or more is traded blind, 2 needs one confluence, 1 needs two. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @23:35<br>transcript: levels webinar part 1, user-pasted transcript @24:25<br>transcript: levels webinar part 1, user-pasted transcript @1:14:52 |
| **level-scan.four-touch-support-is-not-a-buy**<br>A level price has hit four times as support is NOT bought: the stops of breakout traders and of failing longs pile up below it. If it breaks, it becomes the great sell on the rally back.<br>*The same-direction case is the subject of a second webinar that is not in the sources provided.* | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @8:39<br>transcript: levels webinar part 1, user-pasted transcript @12:24<br>transcript: levels webinar part 1, user-pasted transcript @1:03:09 |
| **level-scan.clustering-is-the-builders**<br>The sources mark levels by eye. This scanner clusters swing prices within half an H1 ATR and keeps those within 20% of price. Those two numbers are the builder's. | heuristic | inferred | yes | transcript: levels webinar part 1, user-pasted transcript @2:44 |
