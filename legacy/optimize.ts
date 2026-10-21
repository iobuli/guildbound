import { readFileSync, writeFileSync } from 'node:fs';
import { runBacktest, type Trade } from './engine.js';
import { mean, f } from './stats.js';
import { shuffledCandles } from './nullmodel.js';
import { P } from '../core/params.js';
import type { Candle } from '../core/types.js';

const sol: Candle[] = JSON.parse(readFileSync(new URL('../../data/SOLUSDT_1h.json', import.meta.url), 'utf8'));
const START = sol.findIndex((b) => b.t >= Date.UTC(2021, 0, 1));
const GRID = { zoneHalfAtr: [0.15, 0.25, 0.4], touchReactionAtr: [0.75, 1.0, 1.5], retestMinExcursionAtr: [0.5, 1.0] } as const;
const base = { zoneHalfAtr: P.zoneHalfAtr, touchReactionAtr: P.touchReactionAtr, retestMinExcursionAtr: P.retestMinExcursionAtr };
const configs: Record<string, number>[] = [];
for (const z of GRID.zoneHalfAtr) for (const r of GRID.touchReactionAtr) for (const e of GRID.retestMinExcursionAtr) configs.push({ zoneHalfAtr: z, touchReactionAtr: r, retestMinExcursionAtr: e });

const R = (ts: Trade[], pick: (t: Trade) => boolean) => { const x = ts.filter((t) => t.ok && pick(t) && t.r['2'] !== undefined).map((t) => t.r['2']!); return { n: x.length, m: mean(x) }; };
function sweep(label: string, candles: Candle[]) {
  const res = configs.map((cfg) => {
    Object.assign(P, cfg);
    const { trades } = runBacktest(candles, { start: START, end: 1e9 });
    return { cfg, goIS: R(trades, (t) => t.go && t.year <= 2023), goOOS: R(trades, (t) => t.go && t.year >= 2024), allIS: R(trades, (t) => t.year <= 2023), allOOS: R(trades, (t) => t.year >= 2024) };
  });
  Object.assign(P, base);
  console.log(`\n== ${label} ==  (mean net R at 2R; GO trades = every veto clears)`);
  for (const r of res) console.log(`zone ${r.cfg.zoneHalfAtr!.toFixed(2)} react ${r.cfg.touchReactionAtr!.toFixed(2)} space ${r.cfg.retestMinExcursionAtr!.toFixed(1)} | GO IS n=${String(r.goIS.n).padStart(4)} ${f(r.goIS.m, 2).padStart(5)}  OOS n=${String(r.goOOS.n).padStart(4)} ${f(r.goOOS.m, 2).padStart(5)} | ALL IS ${f(r.allIS.m, 2).padStart(5)} OOS ${f(r.allOOS.m, 2).padStart(5)}`);
  const elig = res.filter((r) => r.goIS.n >= 200);
  const best = [...elig].sort((a, b) => b.goIS.m - a.goIS.m)[0]!;
  console.log(`-> best in-sample (n>=200): zone ${best.cfg.zoneHalfAtr} react ${best.cfg.touchReactionAtr} space ${best.cfg.retestMinExcursionAtr}: IS ${f(best.goIS.m, 3)} -> OOS ${f(best.goOOS.m, 3)} (n=${best.goOOS.n}); default config OOS ${f(res.find((r) => r.cfg.zoneHalfAtr === base.zoneHalfAtr && r.cfg.touchReactionAtr === base.touchReactionAtr && r.cfg.retestMinExcursionAtr === base.retestMinExcursionAtr)!.goOOS.m, 3)}`);
  return { res, best };
}
const real = sweep('SOL (real)', sol);
const nul = sweep('SOL block-shuffled (memory destroyed)', shuffledCandles(sol, 101));
writeFileSync(new URL('../../out/optimize.json', import.meta.url), JSON.stringify({ real, nul }, null, 1));
