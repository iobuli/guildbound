import { P } from './params.js';

/** The parameters that are counted in base bars, with their H1 values. */
const BAR_PARAMS = ['sfpValidH1Bars', 'touchClusterBars', 'touchReactionBars', 'eraLookbackBars', 'retestMinBars', 'momentumBars', 'chopWindow'] as const;
const LOOKBACK = { h1: 800, d1: 400, w1: 120 };

/**
 * Re-expresses every bar-counted parameter for a base timeframe of `periodMs` (H1 = 3.6e6) so the same amount of TIME is meant:
 * 11 H1 bars of space becomes 22 M30 bars or 3 H4 bars. Returns a function that restores the H1 values.
 */
export function scaleForPeriod(periodMs: number): () => void {
  const f = 3_600_000 / periodMs, saved: Record<string, number> = {}, savedLb = { ...P.lookback };
  for (const k of BAR_PARAMS) { saved[k] = P[k]; P[k] = Math.max(1, Math.round(P[k] * f)); }
  P.lookback = { h1: Math.round(LOOKBACK.h1 * f), d1: LOOKBACK.d1, w1: LOOKBACK.w1 };
  return () => { for (const k of BAR_PARAMS) P[k] = saved[k]!; P.lookback = savedLb; };
}
