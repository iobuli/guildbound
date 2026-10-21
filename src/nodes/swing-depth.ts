import { defineNode } from '../core/node.js';
import { AtrPort, SwingsPort, type SwingInfoT } from '../core/ports.js';
import { fractals, swingInOut } from '../core/ta.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Candle } from '../core/types.js';

function swingsFor(tf: 'h1' | 'd1', c: Candle[], atrD1: number, keep: number): SwingInfoT[] {
  const out: SwingInfoT[] = [];
  for (const f of fractals(c, P.fractalN)) {
    if (f.i < c.length - keep) continue;
    const m = swingInOut(c, f);
    const depthAtr = atrD1 > 0 ? m.depth / atrD1 : 0;
    out.push({ tf, i: f.i, t: c[f.i]!.t, side: f.side, price: f.price, swingIn: m.swingIn, swingOut: m.swingOut, depth: m.depth, depthAtr, deep: depthAtr >= P.deepSwingAtr[tf], sweptAt: m.sweptAt });
  }
  return out;
}

export default defineNode({
  id: 'swing-depth',
  title: 'Swing depth (swing in / swing out)',
  stage: 'L',
  summary: 'Lists swing highs and lows and measures how "deep" each is, so only significant swings can be swept. Depth is the smaller of the swing-in and swing-out moves, compared with the daily ATR.',
  decideAt: 'prior_bars',
  step: { n: 0.1, label: 'Swing depth' },
  inputs: { atr: AtrPort },
  outputs: { swings: SwingsPort },
  rules: [
    rule('swing-depth.in-out-definition', 'Draw a line at the swing price, run it left until it hits a candle and right until the sweep. The lowest (highest) point of each section is the swing in / swing out. Measure the SMALLER side.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-h1-sweep.md', '2024-10-22'), src('webinar', 'curated/advanced-sfp.md', '2024-09-24')],
    }),
    rule('swing-depth.deep-threshold', 'An H1 swing is deep at 1x ATR or more; a D1 swing needs 3x the current day\'s ATR. Anything smaller is ignored when it is swept.', {
      codable: true, sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24'), src('lesson', 'curated/taking-a-trade-h1-sweep.md', '2024-10-22')],
      note: 'Worked examples: 57.5 against an ATR of 66.8 is NOT deep; 1018 against 568 pipettes is.',
    }),
    rule('swing-depth.where-do-people-get-bearish', 'Ask where people will get bearish (bullish): only the main swing counts, not a tiny low sitting under it.', {
      kind: 'heuristic', sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24')],
    }),
    rule('swing-depth.shallow-swing-above-deep', 'A deep swing can be ignored if another deep swing sits right above it, since that is easy extra liquidity. A recent, prominent high close by can also become the place people get bullish instead.', {
      status: 'inferred', kind: 'heuristic', sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24')],
      note: 'The note-taker marked this SPECULATION.',
    }),
  ],
  conflicts: [{
    a: 'swing-depth.in-out-definition', b: 'swing-depth.deep-threshold',
    note: 'The notes say an H1 swing must be "1x ATR" without saying which ATR; the lesson examples measure it against the DAILY ATR. This build uses the D1 ATR for both (x1 for H1, x3 for D1).',
  }],
  evaluate: (inp, v) => ({ out: { swings: { h1: swingsFor('h1', v.h1, inp.atr.d1, 400), d1: swingsFor('d1', v.d1, inp.atr.d1, 250) } } }),
});
