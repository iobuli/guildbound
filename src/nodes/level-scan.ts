import { defineNode } from '../core/node.js';
import { AtrPort, LevelScanPort, SwingsPort } from '../core/ports.js';
import { analyzeLevel, tierOf } from '../core/levelmath.js';
import { P } from '../core/params.js';
import { fractals } from '../core/ta.js';
import { rule, src } from '../core/rule.js';

const T = (t: string) => src('transcript', `levels webinar part 1, user-pasted transcript @${t}`);

export default defineNode({
  id: 'level-scan',
  title: 'Level scanner',
  stage: 'L',
  summary: 'Marks the levels on the chart: clusters the swing highs and lows, drops any that price has not yet broken, and for each one on the right side of the market counts the swing touches in its previous role and says which tier it falls in.',
  decideAt: 'prior_bars',
  step: { n: 0.5, label: 'Mark the levels on the chart and count their swing touches' },
  inputs: { atr: AtrPort, swings: SwingsPort },
  outputs: { levels: LevelScanPort },
  rules: [
    rule('level-scan.mark-levels-first', 'Start by marking prior support/resistance flips on the weekly, daily and hourly charts, then look at where price currently is and which of those levels sit on the right side of it.', {
      sources: [T('2:44'), T('5:25'), T('1:02:30')],
    }),
    rule('level-scan.triage-by-touches', 'Grade every level by its number of swing touches, which fixes the plan for it: 3 or more is traded blind, 2 needs one confluence, 1 needs two.', {
      codable: true, sources: [T('23:35'), T('24:25'), T('1:14:52')],
    }),
    rule('level-scan.four-touch-support-is-not-a-buy', 'A level price has hit four times as support is NOT bought: the stops of breakout traders and of failing longs pile up below it. If it breaks, it becomes the great sell on the rally back.', {
      sources: [T('8:39'), T('12:24'), T('1:03:09')], note: 'The same-direction case is the subject of a second webinar that is not in the sources provided.',
    }),
    rule('level-scan.clustering-is-the-builders', 'The sources mark levels by eye. This scanner clusters swing prices within half an H1 ATR and keeps those within 20% of price. Those two numbers are the builder\'s.', {
      codable: true, status: 'inferred', kind: 'heuristic', sources: [T('2:44')],
    }),
  ],
  evaluate: (inp, v) => {
    const c = v.h1, price = c.at(-1)?.c;
    if (price === undefined || !(inp.atr.h1 > 0)) return { out: { levels: { price: 0, rows: [] } } };
    const raw = P.scanSource === 'fractals' ? fractals(c, 1).map((f) => f.price) : [...inp.swings.h1, ...inp.swings.d1].map((s) => s.price);
    const pts = raw.filter((p) => Math.abs(p / price - 1) <= P.scanRangePct).sort((a, b) => a - b);
    const scale = P.levelScaleD1 ? inp.atr.d1 : inp.atr.h1;
    const gap = P.scanClusterAtr * scale, groups: number[][] = [];
    for (const p of pts) { const g = groups[groups.length - 1]; if (g && p - g[g.length - 1]! <= gap) g.push(p); else groups.push([p]); }
    const rows = groups.map((g) => {
      const centre = g[Math.floor(g.length / 2)]!, dir = centre < price ? ('long' as const) : ('short' as const);
      const a = analyzeLevel(c, centre, dir, scale);
      return a.found ? { price: centre, dir, side: (dir === 'long' ? 'below' : 'above') as 'below' | 'above', distAtr: Math.abs(centre - price) / inp.atr.h1, touches: a.touches, tier: tierOf(a.touches), lxpbPrice: a.lxpbPrice, barsSinceBreak: a.barsSinceBreak, spanBars: a.spanBars, tradable: a.tradable, chopCrossings: a.chopCrossings } : null;
    }).filter((r): r is NonNullable<typeof r> => r !== null && r.touches >= 1).sort((a, b) => a.distAtr - b.distAtr).slice(0, P.scanMax);
    return { out: { levels: { price, rows } } };
  },
});
