// `kev audit-kev`: checks of kev itself (not of the user's pages). Each prints numbers; fixes are made only where a check confirms a problem.
import { writeFileSync } from 'node:fs';
import { openMarket } from '../core/market-io.js';
import { withPreset } from '../core/presets.js';
import { graph } from '../kev.js';
import { SetupSchema } from '../core/types.js';
import { interpret } from '../interpret.js';
import { fate, mergeLevels } from './atlas.js';
import { placebos, runBacktest, type Scan } from '../backtest/engine.js';
import { BRAIN, log } from '../brain/brain.js';

const DAY = 86_400_000, pct = (x: number) => `${(100 * x).toFixed(0)}%`;
const med = (xs: number[]) => { const s = xs.filter(Number.isFinite).sort((a, b) => a - b); return s.length ? s[Math.floor(s.length / 2)]! : NaN; };

export function auditKev(log2: (s: string) => void = () => {}) {
  const m = openMarket('SOLUSDT'), c = m.h1, feed = m.feed(), L: string[] = ['# Audit of kev itself', '', `Run ${new Date().toISOString().slice(0, 10)} on SOLUSDT by \`kev audit-kev\`.`, ''];
  const idx = (t: number) => c.findIndex((b) => b.t >= t);
  // A. regime trend labels in known windows
  L.push('## A. Regime trend: ATR-based vs percent-based', '', '| window | days | ATR trend up/down/range | percent trend up/down/range |', '|---|---|---|---|');
  for (const [name, a, b] of [['2021 run (Jan-May 10)', '2021-01-01', '2021-05-10'], ['May-Jul 2021 crash', '2021-05-12', '2021-07-20'], ['2022 bear', '2022-01-01', '2022-12-31'], ['2023 recovery', '2023-01-01', '2023-12-31']] as const) {
    const cnt = { atr: { up: 0, down: 0, range: 0 }, pct: { up: 0, down: 0, range: 0 } }; let n = 0;
    for (let t = Date.parse(a) + 12 * 3_600_000; t < Date.parse(b); t += DAY) {
      const r = graph.run(feed, t, SetupSchema.parse({ dir: 'long' }), { instrument: m.instrument }).states.regime?.out.regime as { trend: 'up' | 'down' | 'range'; trendPct: 'up' | 'down' | 'range' } | undefined;
      if (!r) continue; n++; cnt.atr[r.trend]++; cnt.pct[r.trendPct]++;
    }
    L.push(`| ${name} | ${n} | ${cnt.atr.up}/${cnt.atr.down}/${cnt.atr.range} | ${cnt.pct.up}/${cnt.pct.down}/${cnt.pct.range} |`);
  }
  log2('A done');
  // B. level identity tolerance vs fate (2022 replay)
  const i0 = idx(Date.parse('2022-01-01')), i1 = idx(Date.parse('2023-01-01')) - 1;
  const steps = withPreset('kev-default', () => { const s = []; for (let i = i0; i <= i1; i++) s.push({ t: c[i]!.t, r: interpret(feed, c[i]!.t, { instrument: m.instrument, price: c[i]!.o }) }); return s; });
  const tIdx = new Map(c.map((b, i) => [b.t, i]));
  L.push('', '## B. Does the "levels live about a day" finding depend on how levels are merged? (2022)', '', '| merge tolerance | levels | never tested | median life (days) | first test held |', '|---|---|---|---|---|');
  for (const tol of [0.1, 0.25, 0.5]) {
    const ids = mergeLevels(steps, tol); for (const x of ids) fate(c, tIdx.get(x.first)!, Math.min(c.length - 1, i1 + 24 * 90), x);
    const tested = ids.filter((x) => x.tests > 0);
    L.push(`| ${tol} daily ATR | ${ids.length} | ${pct(1 - tested.length / ids.length)} | ${med(ids.filter((x) => x.diedAt).map((x) => (x.diedAt! - x.first) / DAY)).toFixed(1)} | ${pct(tested.filter((x) => x.held > 0).length / tested.length)} |`);
  }
  log2('B done');
  // C. gap fills, costs, placebo matching (2022 harness run)
  const bt = withPreset('kev-default', () => runBacktest(c, { start: i0, end: i1, session: m.session, inst: m.instrument, spreadBps: 0, slipR: 0.1, entryAtLevel: true }));
  const pl = placebos(c, bt.trades, new Map<number, Scan>(bt.scans.map((s) => [s.j, s])), bt.cfg, 2);
  const gaps = bt.trades.filter((t) => Math.abs(t.fill - t.px) > 1e-9);
  const feeR = bt.trades.filter((t) => t.atrD1 > 0).map((t) => (0.002 * t.fill) / (0.5 * t.atrD1));
  const sameDir = pl.every((p) => bt.trades.some((t) => t.dir === p.dir));
  L.push('', '## C. Fills, costs, placebos (2022 harness run)', '',
    `- Fills: ${bt.trades.length}. Filled at a better price than the limit because the bar opened beyond it (gap): ${gaps.length} (${pct(gaps.length / bt.trades.length)}). That is how a resting limit fills; placebos use the same rule.`,
    `- Binance taker fees (0.1% a side) in R at a 0.5-ATR stop: median ${med(feeR).toFixed(3)}R, 90th percentile ${[...feeR].sort((a, b) => a - b)[Math.floor(0.9 * feeR.length)]!.toFixed(3)}R. The flat 0.10R cost covers ${pct(feeR.filter((x) => x <= 0.1).length / feeR.length)} of fills.`,
    `- Placebos: ${pl.length} for ${bt.trades.length} fills; every placebo has a real counterpart on the same side: ${sameDir}.`, '');
  const out = L.join('\n');
  writeFileSync(`${BRAIN}experiments/2026-10-21-kev-audit.md`, out);
  log('audit-kev written to experiments/2026-10-21-kev-audit.md');
  return out;
}
