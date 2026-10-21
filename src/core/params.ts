/**
 * Every number the nodes use, with where it came from.
 * stated   = the trader gave the number
 * example  = read off a worked example (a boundary, not a promise)
 * inferred = the builder's choice to make an idea computable; change freely
 */
export const P = {
  lookback: { h1: 800, d1: 400, w1: 120 }, // closed bars a node can see
  fractalN: 3, // inferred: bars either side for a swing point
  deepSwingAtr: { h1: 1, d1: 3 }, // stated (swing-in / swing-out lesson), measured against the D1 ATR
  atrN: 14, // inferred: H1 ATR length, used only as a scale
  atrD1N: 20, // stated: 20-period daily ATR, because there are 20 trading days in a month
  atrSfpH1: 24, // stated: 24-period ATR for the H1 SFP stop
  levelScaleD1: 0, // 0: zone/reaction/space are multiples of the H1 ATR (this build's default); 1: multiples of the DAILY ATR (the user's SOL Trade Check spec)
  zoneHalfAtr: 0.25, // inferred: half-width of a level zone in H1 ATR
  touchReactionAtr: 1.0, // inferred: move away needed for a touch to count as respected
  touchReactionBars: 12, // inferred
  touchClusterBars: 6, // inferred: touches this close are one touch ("only 1 reaction")
  eraLookbackBars: 600, // inferred
  chopCrossings: 4, // inferred: level crossings that mean "price is chopping through it"
  chopWindow: 40, // inferred: ...within this many base bars
  freshBiasDays: 2, // stated: a D1 engulfing stops counting "after a couple of days"
  confluenceNeeded: 3, // stated: every S/R trade needs 3 confluences
  fibLevels: [0.382, 0.5, 0.618] as number[], // example: 38%, 50%, 61.8% all appear
  fibMaxRetrace: 0.618, // stated: pulled back past 61.8% and the fib is removed
  fibProximityOfD1Atr: 0.1, // stated: fib counts only within 10% of the day's ATR
  atrPassed: 0.9, // inferred: "close enough to ATR" counts
  retestMinBars: 11, // example: 5 bars "not enough", 11 "just about enough"
  retestMinExcursionAtr: 0.5, // backtest-reported space rule: 0.5 x ATR clear before a retest counts
  rallyAwayFraction: 0.5, // stated: about half way to target without filling = swerved
  maxSwerves: 4, // example: "4 swerves should have caused him to cancel"
  sfpMaxVisits: 3, // stated: 4th time back in the area = no
  sfpValidH1Bars: 48, // stated: swing levels stay valid about 2 days
  sfpValidD1Bars: 3, // inferred
  sfpMinBarAtr: 0.5, // inferred: a 6-pip Asian bar "isn't likely to mean anything"
  sfpWideBarAtr: 0.8, // inferred: "wide ranging bar"
  sfpCloseNearExtreme: 0.2, // stated: closes within 20% of the extreme
  sfpWaitFractionToStop: 0.5, // stated: price moves ~50% toward the theoretical stop
  pinBodyMax: 0.3, // inferred (generic pinbar)
  pinWickMin: 0.6, // inferred (generic pinbar)
  minRr: 0.8, // stated: takes trades from 0.8R
  minRrHighProb: 0.5, // stated: sometimes 0.5R in high-probability setups
  evolvingRrClose: 0.5, // stated: evolving R below 0.5 = consider closing
  ftaMinSwingAtr: 0.5, // inferred: a swing smaller than this is not a "trouble area"
  stopBufferAtr: 0.05, // inferred: "just beyond", not 1 tick
  stopBlockerAtr: 0.3, // inferred: a level this close beyond the stop means "stopped into a level"
  crossPairBufferAtr: 0.25, // inferred: crosses squeeze more, so wider stop
  minStopPct: 0.0005, // backtest-reported: 0.05% minimum stop
  newsWindowMin: 15, // stated: no fills 15 minutes either side of red news
  newsPullPips: 20, // stated: pull the limit if price is within 10-20 pips
  newsKeepPips: 60, // example: ~60 pips away on big news, leave it
  newsReinsertMin: 5, // stated: give the news five minutes, then put the order back
  newsPullAtr: 1.0, // inferred: ATR-based stand-in for pips on non-FX
  newsKeepAtr: 3.0, // inferred
  retestMinBarsHard: 2, // stated: never the immediate retest (the very next bar), even with a daily bias
  momentumBars: 6, // inferred
  momentumAtr: 1.5, // inferred: net move into the level that counts as "hard"
  scanClusterAtr: 0.5, // inferred: swing prices closer than this are one level
  scanRangePct: 0.2, // inferred: only levels within 20% of price
  scanMax: 30,
};
