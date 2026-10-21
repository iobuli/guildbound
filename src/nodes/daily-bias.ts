import { defineNode } from '../core/node.js';
import { BiasPort, BiasTf, SfpPort, type SfpSignalT } from '../core/ports.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import { bodyOf, rng } from '../core/ta.js';
import type { Candle, Check } from '../core/types.js';
import type { z } from 'zod';

type Sig = { kind: 'pin' | 'engulfing' | 'sfp' | 'idf'; dir: 'long' | 'short' };

function detectAt(c: Candle[], k: number, sfp: SfpSignalT[]): Sig[] {
  const out: Sig[] = [], b = c[k]!, r = rng(b);
  if (r > 0 && bodyOf(b) <= P.pinBodyMax * r) {
    if ((Math.min(b.o, b.c) - b.l) / r >= P.pinWickMin) out.push({ kind: 'pin', dir: 'long' });
    if ((b.h - Math.max(b.o, b.c)) / r >= P.pinWickMin) out.push({ kind: 'pin', dir: 'short' });
  }
  if (k >= 1) {
    const p = c[k - 1]!;
    if (p.c < p.o && b.c > b.o && b.c >= p.o && b.o <= p.c && bodyOf(b) > bodyOf(p)) out.push({ kind: 'engulfing', dir: 'long' });
    if (p.c > p.o && b.c < b.o && b.c <= p.o && b.o >= p.c && bodyOf(b) > bodyOf(p)) out.push({ kind: 'engulfing', dir: 'short' });
  }
  if (k >= 2) {
    const ins = c[k - 1]!, mom = c[k - 2]!;
    if (ins.h <= mom.h && ins.l >= mom.l) {
      const upBreak = b.h > ins.h, dnBreak = b.l < ins.l;
      if (upBreak && !dnBreak && b.c < ins.h) out.push({ kind: 'idf', dir: 'short' });
      if (dnBreak && !upBreak && b.c > ins.l) out.push({ kind: 'idf', dir: 'long' });
    }
  }
  for (const s of sfp) if (s.valid && s.sweepIdx === k) out.push({ kind: 'sfp', dir: s.dir });
  return out;
}

function biasFor(c: Candle[], sfp: SfpSignalT[]): z.infer<typeof BiasTf> {
  const last = c.length - 1;
  for (let age = 0; age <= P.freshBiasDays && last - age >= 0; age++) {
    const sigs = detectAt(c, last - age, sfp);
    if (!sigs.length) continue;
    const longs = sigs.some((s) => s.dir === 'long'), shorts = sigs.some((s) => s.dir === 'short');
    const signals = sigs.map((s) => ({ ...s, ageBars: age }));
    if (longs && shorts) return { dir: 'neutral', signals, cancelled: true, ageBars: age, fresh: true };
    return { dir: longs ? 'long' : 'short', signals, cancelled: false, ageBars: age, fresh: age <= P.freshBiasDays };
  }
  return { dir: 'neutral', signals: [], cancelled: false, ageBars: null, fresh: false };
}

export default defineNode({
  id: 'daily-bias',
  title: 'Higher-timeframe bias (D1 / W1)',
  stage: 'D',
  summary: 'Reads closed daily and weekly candles for a pinbar, engulfing, SFP or inside-day failure, cancels opposing signals on the same bar, and expires the bias after a couple of days.',
  decideAt: 'prior_bars',
  step: { n: 3.1, label: 'Confluence factor: daily bias (a bar pattern, not a trend)' },
  inputs: { sfp: SfpPort },
  outputs: { bias: BiasPort },
  rules: [
    rule('daily-bias.types', 'Daily bias comes from a pinbar, an engulfing candle, an SFP, or (in the later lessons) an inside-day failure. The 2012-era webinar names only the first three.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('lesson', 'curated/taking-a-trade-daily-biases.md', '2024-10-18'), src('transcript', 'levels webinar part 1, user-pasted transcript @37:07')],
    }),
    rule('daily-bias.crossbar-cancels', 'Two opposing biases on one bar (a bullish engulfing with a bearish SFP) cancel out and the market is neutral.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('webinar', 'curated/notes-images.md', '2024-09-08')],
    }),
    rule('daily-bias.expires', 'A daily-engulfing bias stops counting after a couple of days if no entry is given.', {
      codable: true, sources: [src('webinar', 'curated/notes-images.md', '2024-09-10')],
    }),
    rule('daily-bias.weekly-and-daily-then-h1', 'W1/D1 give the bias, H1 gives the execution, and he never looks lower.', {
      sources: [src('tweet', 'curated tweets', '2026-09-24'), src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('daily-bias.only-20-percent', 'Only about 20% of his trades have a daily bias. The other 80% go straight to H1 and are quick in-and-out trades at 0.8-1R on average.', {
      kind: 'heuristic', sources: [src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('daily-bias.against-it', 'You can trade against the daily bias, but the levels must be good and you must get out at the first sign of trouble. An H1 short is normally skipped if the day looks set to close as a bullish engulfing, unless the target is very close.', {
      sources: [src('webinar', 'curated/trade-management.md', '2024-10-18'), src('webinar', 'user-pasted notes', '2024-09-25')],
    }),
    rule('daily-bias.reverse-trading', 'Start from where price is going and why, then look for an entry that fits that bias.', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-what-constitutes-a-daily-bias.md', '2026-02-07')],
    }),
    rule('daily-bias.candle-definitions', 'The exact candle definitions (pin wick share, engulfing test) are not given in the sources. This build uses generic textbook thresholds.', {
      status: 'inferred', kind: 'heuristic', codable: true, sources: [src('lesson', 'curated/taking-a-trade-daily-biases.md', '2024-10-18')],
    }),
    rule('daily-bias.unclosed-anticipation', 'In his journal he sometimes acts on a daily or weekly candle that has not closed yet ("looks like it might close as a bullish engulfing").', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-passed-atr.md', '2024-12-21'), src('journal', 'curated/lxpb-trades-what-constitutes-a-daily-bias.md', '2026-02-07')],
    }),
    rule('daily-bias.closed-bars-only', 'This build only reads CLOSED daily and weekly bars, so an anticipated but unclosed candle is never used.', {
      status: 'inferred', codable: true, sources: [src('backtest', 'user-supplied forensic page', '2026-09-13')],
    }),
    rule('daily-bias.bar-pattern-not-trend', 'The bias must be a bar pattern on the daily chart, never just the trend: a pinbar, a swing failure (swing low broken and price closes above, or the reverse) or an engulfing. Silver example: a bullish pinbar that was also a swing failure gave "a strong directional bias" because breaking the range low and closing back inside usually takes price to the other side of the range.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @35:16'), src('transcript', 'levels webinar part 1, user-pasted transcript @36:03'), src('transcript', 'levels webinar part 1, user-pasted transcript @37:47')],
    }),
  ],
  conflicts: [{
    a: 'daily-bias.unclosed-anticipation', b: 'daily-bias.closed-bars-only',
    note: 'The discretionary journal anticipates unclosed daily candles; the causal rule forbids it. A systematic version cannot act on a candle that has not printed.',
  }],
  evaluate: (inp, v, setup) => {
    const d1 = biasFor(v.d1, inp.sfp.d1), w1 = biasFor(v.w1, []);
    const htf = d1.dir !== 'neutral' ? d1.dir : w1.dir;
    const checks: Check[] = [];
    if (htf !== 'neutral') checks.push({ rule: 'daily-bias.against-it', pass: htf === setup.dir ? true : null, detail: htf === setup.dir ? `trade is with the ${htf} bias` : `trade is AGAINST the ${htf} bias: needs good levels and a quick exit` });
    return { out: { bias: { d1, w1, htf } }, checks };
  },
});
