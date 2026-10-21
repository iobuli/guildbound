# M5 failed-breakdown reversal (Bund)

`ltf-reversal` · stage **E** · decides at **prior_bars** · rule card (nothing to compute)

A lower-timeframe reversal he trades a lot on the M5 Bund. It is not an SFP.

## Ports

**Inputs**
- none

**Outputs**
- none

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **ltf-reversal.pattern**<br>Price closes through the lows three times but immediately goes bid each time. "If the market is making fresh lows it should be a weak market, but it is immediately going bid." Weakness keeps getting bought. | hard | stated | no | webinar: curated/notes-images.md (2024-09-25) |
| **ltf-reversal.needs-divergence**<br>It needs MACD divergence, the same as the sweep technique. | hard | stated | no | webinar: curated/notes-images.md (2024-09-25) |
| **ltf-reversal.entry**<br>Entry is a market buy at the first reversal candle after the third breakdown, with the stop just under the lows. No ATR or thrust candle is needed. | hard | stated | no | webinar: curated/notes-images.md (2024-09-25) |
| **ltf-reversal.m5-habits**<br>M5 levels are marked and traded the same way as H1; M5 SFPs use a 12-period ATR; he does not hold M5 through news; the M5 stop trail can differ (a head-and-shoulders shoulder instead of the thrust candle). | hard | observed | no | webinar: curated/levels.md (2024-09-28)<br>webinar: curated/trade-management.md (2024-10-18) |
