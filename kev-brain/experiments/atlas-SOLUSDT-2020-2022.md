# Atlas reading: SOLUSDT 2020-2022 (observer notes)

Written by Claude after reading `out/atlas/SOLUSDT_2020-2022.json` (`kev atlas SOLUSDT --from=2020 --to=2022`, preset kev-default),
2026-10-21. kev replayed 19,847 hours, 2020-09-25 to 2022-12-31, deciding each hour from closed bars only. It marked
467 distinct levels; each was then followed to the end of its life. Tags: **[causal]** = knowable at the time;
**[hindsight]** = uses what happened later. Hindsight describes the market. It is not evidence that anything could be traded.

## Phase notes

### 2020-09-25 to 2020-12-23 falling: 3.28 to 1.03 (-69%)
SOL bled out of its launch hype. kev's regime node mostly said "range" (1,289 of 2,149 hours) and "down" for 764 hours.
- Levels [hindsight]:
  - kev marked 42 levels, most of them resistance flips (26 vs 16).
  - 55% of them were never revisited, and only half of first retests held.
  - A falling market leaves levels behind rather than returning to them.
- Daily bias signals were coin flips (4 of 9 next-day hits) [hindsight outcome of causal signals].
- Fills at kev's levels beat random orders by +0.06R, with net about -0.03R [causal].

### 2020-12-23 to 2021-05-18 rising: 1.03 to 58.38 (+5,567%)
The parabolic run. kev called the regime "up" for 2,561 of 3,520 hours [causal].
- It marked 135 levels, mostly support flips (83).
- First retests came fast (median 16 hours after first sight) and held 74% of the time [hindsight].
- This is where fills beat random orders most: +0.11R, CI 0.05 to 0.18 [causal].
  - The net was still only about breakeven (+0.01R).
  - The structure helped, but costs and the stop rule ate the edge.
- Bearish daily patterns failed (mean -0.87 ATR three days later), while bullish ones paid (+0.68).
  - The bias node took both at face value.
- The strongest level of the window formed here: support near 40.7, held 6 of 7 tests from late April until the May crash.

### 2021-05-18 to 2021-07-20 falling: 58.38 to 22.10 (-62%)
The May crash and its aftermath.
- Every one of the 1,536 hours was labelled "range" by the regime node. Volatility exploded, so a fall measured in daily ATRs looked flat.
  - This is a real weakness of the builder's trend descriptor (see Limits).
- Only 18 levels were marked, but 83% of them were tested.
- Support near 20.4-20.9 formed in April and was tested 4-5 times over the next 19 months.
  - It held every time.
  - It finally broke on 2022-11-09, the FTX collapse [hindsight].
- Fills vs random: +0.03R, CI spans 0 [causal].

### 2021-07-20 to 2021-11-06 rising: 22.10 to 259.90 (+1,076%)
The second leg up to the all-time high.
- kev saw "up" for 1,316 hours and "range" for 1,270 [causal].
- It marked 90 levels; first retests held 73% of the time [hindsight].
- Bullish daily patterns had the best follow-through of the window (+1.64 ATR after three days), but there were only 7 of them.
- Fills beat random by +0.07R, net -0.07R [causal].
- Support near 150.6 held 5 of 6 tests in September.

### 2021-11-06 to 2022-12-31 falling: 259.90 to 9.66 (-96%, to the end of the window)
A 14-month bear market: Luna in May, the summer range, then FTX in November.
- The regime node called it "range" for 6,096 hours and "down" for only 3,360 [causal].
- kev marked 189 levels, mostly resistance flips (108).
- Retests were slow (median 61 hours) and held only 55% of the time [hindsight].
- Daily bias was useless both ways: long signals -0.27 ATR and short signals -0.12 ATR after three days.
- The 2022 range had its own durable floors:
  - 98.2 held 4 tests in March to April;
  - 48 held 4 of 5 in May;
  - 30.6 held 4 of 5 from September to October;
  - 27.6 held 5 of 5 from June until 2022-11-08 [hindsight].
- Fills beat random by +0.04R, CI 0.00 to 0.06, with net about -0.03R across 1,491 fills [causal].

## What I learned
1. **The structure is real but small, in every phase [causal].** Fills at kev's levels beat random orders at the same distance and time in all five phases: +0.03 to +0.11R, +0.06R overall (CI 0.04 to 0.08). The edge is largest in the strongest trend (+0.11R in the 2021 run). It never survives costs: net is -0.08R to +0.01R by phase.
2. **Levels in this market are short-lived [hindsight].**
   - Of 467 levels, 435 were eventually closed through, and the median life is about 1 day; the longest 10% last 7 weeks or more.
   - Almost half (224) were never revisited at all.
   - A level kev marks on SOL is usually a passing waypoint, not a wall.
3. **"With-trend levels hold better" is mostly hindsight.**
   - Measured against the hindsight phase direction, with-trend first retests held 68% and counter-trend 53%.
   - Measured against what kev knew at the time (its regime trend), "trend agrees with the level" had an AUC of 0.44 for predicting a level that holds 3+ tests: no better than a coin.
   - The phase direction is only known afterwards.
4. **Touch count when kev first sees a level does not predict its future [hindsight outcome, causal feature].**
   - First-retest hold rate is 63% for 1 touch, 59% for 2, 71% for 3 and 62% for 4+.
   - The levels that later held 3+ tests had fewer touches at first sight (1.4 vs 2.0; AUC 0.38).
   - This matches the transcript's warning that a much-touched support is not a buy, and the cross-market H2 result (no touch effect).
5. **Nothing kev knows at first sight separates the strong levels.** Only 22 of 243 tested levels (9%) held 3+ tests. The best first-sight feature was low volatility (AUC 0.58), which is weak.
6. **Daily bias depends on context it does not see [hindsight outcome].**
   - Bullish patterns paid in rising phases (+0.68 and +1.64 ATR after three days).
   - Bearish patterns failed in the 2021 run (-0.87).
   - In the 2022 bear both directions lost.
   - As a standalone signal it is close to noise. It may only be useful alongside a trend context kev does not have yet.
7. **The regime descriptor needs work.** Measuring trend in daily ATRs labels crashes as "range": every hour of the May to July 2021 crash, and 60% of the 2022 bear. A percentage- or drawdown-based trend would read these phases better. I have not changed it: changing it would be tuning on this window.

## Limits of this reading
- One market, one cycle, one parser (kev-default). The phase boundaries are hindsight (8-ATR daily swings).
- The last phase ends at the window edge, not at the low.
- "Tests" and "holds" use kev's own zone (0.1 daily ATR at first sight) and a 0.5 ATR bounce. Different but reasonable rules would move the percentages, not the direction of the findings above.
- Next, if wanted: the same atlas on ETH 2020-2022 and gold, then pre-register any lead (for example a drawdown-based regime, or bias combined with regime) before testing it causally.
