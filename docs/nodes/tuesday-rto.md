# Tuesday weekly-open retest

`tuesday-rto` · stage **E** · decides at **prior_bars** · computed

A self-contained setup: on a Tuesday, if the weekly open has not been touched, trade an H1 pinbar toward it with the pin as the stop.

## Ports

**Inputs**
- none

**Outputs**
- `tuesday`: valid: boolean, dir: object, entry: number,null, stop: number,null, target: number,null, rr: number,null, reasons: string[]

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **tuesday-rto.rules**<br>Tuesday only. The weekly open must not have been touched since 22:00 on Monday (and not so far on Tuesday). Buy or sell with the weekly open as the target. | hard | stated | yes | lesson: curated/taking-a-trade-tuesday-rto.md (2025-01-10) |
| **tuesday-rto.getting-in**<br>Enter on a multiple-tap level or an H1 pinbar. Weekly open above price: buy a bullish pin. Weekly open below: sell a bearish pin.<br>*Only the pinbar route is coded; the multiple-tap-level route needs a level and is left to the level node.* | hard | stated | yes | lesson: curated/taking-a-trade-tuesday-rto.md (2025-01-10) |
| **tuesday-rto.orders**<br>Buy the top of the pin or sell its low. The stop goes on the other side of the pinbar. Only take trades that are at least 1R. | hard | stated | yes | lesson: curated/taking-a-trade-tuesday-rto.md (2025-01-10) |
| **tuesday-rto.holds-past-tuesday**<br>It does not matter if the trade runs into Wednesday; only the entry day counts. | hard | stated | no | lesson: curated/taking-a-trade-tuesday-rto.md (2025-01-10) |
| **tuesday-rto.weekly-open-definition**<br>The sources do not say which clock defines the weekly open. This build uses the first base bar of the trading week (Monday 00:00 for crypto, Sunday evening for metals and FX). | hard | inferred | yes | lesson: curated/taking-a-trade-tuesday-rto.md (2025-01-10) |
