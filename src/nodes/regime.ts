import { defineNode } from '../core/node.js';
import { AtrPort, RegimePort } from '../core/ports.js';
import { rule, src } from '../core/rule.js';

const B = (why: string) => src('backtest', `builder's descriptor: ${why}`);

/** Wilder ATR(20) at every closed day, so today's ATR can be ranked against the past year. */
function atrSeries(d: readonly { h: number; l: number; c: number }[], n = 20): number[] {
  const out: number[] = []; let a = 0;
  for (let i = 1; i < d.length; i++) {
    const tr = Math.max(d[i]!.h - d[i]!.l, Math.abs(d[i]!.h - d[i - 1]!.c), Math.abs(d[i]!.l - d[i - 1]!.c));
    a = i <= n ? (a * (i - 1) + tr) / i : (a * (n - 1) + tr) / n;
    out.push(a);
  }
  return out;
}

export default defineNode({
  id: 'regime',
  title: 'Market regime',
  stage: 'X',
  summary: 'Describes the market the levels sit in: daily and weekly trend, where price is in its recent range, how volatile it is against its own past year, and how far it is below its high. Context for the reading only; it gates nothing.',
  decideAt: 'prior_bars',
  step: { n: 0.2, label: 'Regime: trend, volatility, range position (context only)' },
  inputs: { atr: AtrPort },
  outputs: { regime: RegimePort },
  rules: [
    rule('regime.trend-from-the-daily-mean', 'Trend is the 10-day change of the 20-day mean close, measured in daily ATRs: up at +1 ATR or more, down at -1 or less, otherwise range. The weekly trend compares the last weekly close with the 10-week mean.', {
      status: 'inferred', kind: 'heuristic', codable: true, sources: [B('the sources describe HTF context and bias by bar patterns but give no trend definition; this one is only for describing the market')],
    }),
    rule('regime.volatility-against-its-own-past', 'Volatility is the daily ATR as a percentage of price, ranked against the past 250 days: below the 33rd percentile is low, above the 67th is high.', {
      status: 'inferred', kind: 'heuristic', codable: true, sources: [B('ranks volatility so that crypto and gold read on the same scale')],
    }),
    rule('regime.context-only', 'Regime never vetoes or adds a confluence. It feeds the interpreter so a reading can say what kind of market a level is in.', {
      status: 'inferred', codable: true, sources: [B('kept out of the decision path until a pre-registered test says it helps')],
    }),
  ],
  evaluate: (inp, v) => {
    const d = v.d1, w = v.w1, n = d.length, atr = inp.atr.d1;
    const blank = { trend: 'range' as const, slopeAtr: 0, weekTrend: 'range' as const, rangePos: 0.5, volPct: 0.5, volState: 'normal' as const, atrPct: 0, drawdown: 0, days: n };
    if (n < 35 || !(atr > 0)) return { out: { regime: blank } };
    const mean = (a: number, b: number) => { let s = 0; for (let i = a; i < b; i++) s += d[i]!.c; return s / (b - a); };
    const slopeAtr = (mean(n - 20, n) - mean(n - 30, n - 10)) / atr;
    const trend: 'up' | 'down' | 'range' = slopeAtr >= 1 ? 'up' : slopeAtr <= -1 ? 'down' : 'range';
    let hi = -Infinity, lo = Infinity; for (let i = Math.max(0, n - 60); i < n; i++) { hi = Math.max(hi, d[i]!.h); lo = Math.min(lo, d[i]!.l); }
    const last = d[n - 1]!.c, rangePos = hi > lo ? (last - lo) / (hi - lo) : 0.5;
    const series = atrSeries(d).map((a, k) => a / d[k + 1]!.c), cur = series[series.length - 1]!, past = series.slice(-250);
    const volPct = past.filter((x) => x < cur).length / Math.max(1, past.length);
    let top = -Infinity; for (const b of d) top = Math.max(top, b.h);
    let weekTrend: 'up' | 'down' | 'range' = 'range';
    if (w.length >= 11) { let s = 0; for (let i = w.length - 10; i < w.length; i++) s += w[i]!.c; const m = s / 10, wc = w[w.length - 1]!.c; weekTrend = wc > m * 1.02 ? 'up' : wc < m * 0.98 ? 'down' : 'range'; }
    return { out: { regime: { trend, slopeAtr, weekTrend, rangePos, volPct, volState: (volPct < 0.33 ? 'low' : volPct > 0.67 ? 'high' : 'normal') as 'low' | 'normal' | 'high', atrPct: atr / last, drawdown: last / top - 1, days: n } } };
  },
});
