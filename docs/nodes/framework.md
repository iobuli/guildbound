# Direction / Objection / Location / Execution

`framework` · stage **D** · decides at **prior_bars** · rule card (nothing to compute)

The structure the trader's own journal uses for every trade. The graph's stages follow it: D (bias), O (what is in the way), L (the level), E (the order).

## Ports

**Inputs**
- none

**Outputs**
- none

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **framework.doel**<br>Newer journal entries are written as Direction (the bias and why), Objection (what stands in the way, such as a target inside a weekly level), Location (the level the trade is built on) and Execution (how the order is placed, usually the H1 LXPB). | hard | observed | no | journal: curated/lxpb-trades-engulfing.md (2026-05-03)<br>journal: curated/lxpb-trades-pattern-failure.md (2026-06-24) |
| **framework.stacked-lxpb**<br>The passed-ATR trades stack LXPBs across weekly, daily and hourly together with "past ATR". The more timeframes share the same level, the stronger the setup. | heuristic | observed | no | journal: curated/lxpb-trades-passed-atr.md (2024-12-16)<br>journal: curated/lxpb-trades-passed-atr.md (2024-12-21) |
| **framework.follow-your-rules**<br>Rules to live by: 1. always follow your rules; 2. always follow your rules; 3. always follow your rules; 4. know when to break the rules. | heuristic | stated | no | webinar: user-pasted notes (2024-09-24) |
| **framework.eighty-twenty**<br>About 20% of trades have a daily bias (use D1 for target, H1 for entry and stop); 80% go straight to H1 for target, entry and stop. | hard | stated | no | webinar: user-pasted notes (2024-09-24) |
| **framework.order-of-operations**<br>The order the method is applied in: 1. mark levels (W1/D1/H1). 2. Which side of the market is each level on (buy below, sell above). 3. Is it an inverse-last level that price respected last time. 4. Count swing touches. 5. Tier: 3+ blind, 2 needs one confluence, 1 needs two (bias, fib, ATR). 6. Space and momentum, not the immediate retest. 7. Limit order, stop where you would still be bullish, target at the first trouble area. 8. Pull the order if missed and price ran more than half way to target.<br>*Assembled from several places in the transcript; the trader gives the tiers as a summary twice but never lists the whole sequence in one go.* | hard | inferred | no | transcript: levels webinar part 1, user-pasted transcript @4:43<br>transcript: levels webinar part 1, user-pasted transcript @7:47<br>transcript: levels webinar part 1, user-pasted transcript @23:27<br>transcript: levels webinar part 1, user-pasted transcript @25:30<br>transcript: levels webinar part 1, user-pasted transcript @53:21<br>transcript: levels webinar part 1, user-pasted transcript @1:00:04 |
| **framework.part-two-is-missing**<br>This webinar covers only levels where price is going the OPPOSITE way to the last participants (the inverse). A second webinar covers levels where you go the SAME way (buying support, which includes the SFP) and gives a step-by-step "market blueprint" like a flow chart. That second part is not in the sources provided. | hard | observed | no | transcript: levels webinar part 1, user-pasted transcript @1:22:08<br>transcript: levels webinar part 1, user-pasted transcript @1:22:50<br>transcript: levels webinar part 1, user-pasted transcript @2:11 |
| **framework.limit-orders**<br>Nearly 99% of his trades are limit orders, so he does not need to sit at the screen; a "blind" trade just means a resting limit order with a stop and a target. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @23:44<br>transcript: levels webinar part 1, user-pasted transcript @1:15:25 |
