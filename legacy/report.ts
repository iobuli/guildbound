import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { runBacktest, placebos, KEYS, type Trade, type RKey, type Scan } from './engine.js';
import { clusterMean, clusterDiff, benjaminiHochberg, mean, f } from './stats.js';
import { shuffledCandles } from './nullmodel.js';
import type { Candle } from '../core/types.js';

const sol: Candle[] = JSON.parse(readFileSync(new URL('../../data/SOLUSDT_1h.json', import.meta.url), 'utf8'));
const START = sol.findIndex((b) => b.t >= Date.UTC(2021, 0, 1));
const IS = (t: Trade) => t.year <= 2023, OOS = (t: Trade) => t.year >= 2024;
const rows = (ts: Trade[], k: RKey) => ts.filter((t) => t.ok && t.r[k] !== undefined).map((t) => ({ r: t.r[k]!, day: t.day }));

export function runAll(candles: Candle[], seed = 7) {
  const bt = runBacktest(candles, { start: START, end: 1e9, seed });
  const scanMap = new Map<number, Scan>(bt.scans.map((s) => [s.j, s]));
  const pl = placebos(candles, bt.trades, scanMap, bt.cfg);
  return { ...bt, placebo: pl };
}

function line(name: string, ts: Trade[], keys: RKey[] = ['1', '2', '3', '4', 'fta']) {
  const cells = keys.map((k) => { const m = clusterMean(rows(ts, k), 300); return Number.isFinite(m.mean) ? `${f(m.mean, 2)} [${f(m.lo, 2)},${f(m.hi, 2)}]` : 'n/a'; });
  return `${name.padEnd(30)} n=${String(ts.filter((t) => t.ok).length).padStart(5)}  ` + cells.map((c, i) => `${keys[i]}R ${c}`).join('  ');
}

const out: string[] = [];
const log = (s = '') => { console.log(s); out.push(s); };
const t0 = Date.now();
const real = runAll(sol);
const ok = real.trades.filter((t) => t.ok);
log(`# Backtest of the node graph on SOL/USDT H1, ${new Date(sol[START]!.t).toISOString().slice(0, 10)} to ${new Date(sol.at(-1)!.t).toISOString().slice(0, 10)}`);
log(`Resting limit orders at the last high/low before each flipped level's breakout; filled on touch; stop from the stop node; costs 0.10R per trade; stop wins ties.`);
log(`Mean net R per trade [95% CI, bootstrapped over days]. ${real.scans.length} scans, ${real.trades.length} fills (${ok.length} with a valid stop), ${((Date.now() - t0) / 1000).toFixed(0)}s.\n`);

log('## 1. Everything the system would fill');
log(line('all fills', ok)); log(line('  in-sample 2021-23', ok.filter(IS))); log(line('  out-of-sample 2024+', ok.filter(OOS)));
log('\n## 2. Against random orders (same side, same distance, same risk, same window)');
log(line('real levels (all tiers)', ok)); log(line('placebo', real.placebo));
log(line('real levels, OOS', ok.filter(OOS))); log(line('placebo, OOS', real.placebo.filter(OOS)));
const cmp = KEYS.map((k) => { const d = clusterDiff(rows(ok, k), rows(real.placebo, k)); return `${k}R diff ${f(d.diff, 3)} (p=${f(d.p, 2)})`; });
log('real minus placebo: ' + cmp.join('   '));

log('\n## 3. By the system\'s own verdict and tier');
log(line('GO (every veto clears)', ok.filter((t) => t.go))); log(line('NO-GO', ok.filter((t) => !t.go)));
log(line('  GO, in-sample', ok.filter((t) => t.go && IS(t)))); log(line('  GO, out-of-sample', ok.filter((t) => t.go && OOS(t))));
for (const tier of ['blind', 'needs-one-confluence', 'needs-two-confluences']) log(line(`tier ${tier}`, ok.filter((t) => t.tier === tier)));
const go2 = rows(ok.filter((t) => t.go), '2'), no2 = rows(ok.filter((t) => !t.go), '2'), d = clusterDiff(go2, no2);
log(`GO minus NO-GO at 2R: ${f(d.diff, 3)} R per trade (p=${f(d.p, 2)})`);

log('\n## 4. Each rule on its own: fills that PASS it vs FAIL it, at a 2R target');
const rules = new Set<string>(); for (const t of ok) for (const r of Object.keys(t.checks)) rules.add(r);
const rt: { rule: string; nP: number; nF: number; mP: number; mF: number; diff: number; p: number; isDiff: number; oosDiff: number }[] = [];
for (const r of rules) {
  const P = ok.filter((t) => t.checks[r] === true), F = ok.filter((t) => t.checks[r] === false);
  if (P.length < 40 || F.length < 40) continue;
  const dd = clusterDiff(rows(P, '2'), rows(F, '2'));
  const sub = (fn: (t: Trade) => boolean) => mean(rows(P.filter(fn), '2').map((x) => x.r)) - mean(rows(F.filter(fn), '2').map((x) => x.r));
  rt.push({ rule: r, nP: P.length, nF: F.length, mP: mean(rows(P, '2').map((x) => x.r)), mF: mean(rows(F, '2').map((x) => x.r)), diff: dd.diff, p: dd.p, isDiff: sub(IS), oosDiff: sub(OOS) });
}
const q = benjaminiHochberg(rt.map((x) => x.p));
rt.forEach((x, k) => ((x as any).q = q[k]));
for (const x of rt.sort((a, b) => a.p - b.p)) log(`${x.rule.padEnd(44)} pass n=${String(x.nP).padStart(4)} ${f(x.mP, 2)}R | fail n=${String(x.nF).padStart(4)} ${f(x.mF, 2)}R | diff ${f(x.diff, 2).padStart(6)}  p=${f(x.p, 2)} q=${f((x as any).q, 2)}  IS ${f(x.isDiff, 2)} OOS ${f(x.oosDiff, 2)}`);

log('\n## 5. The same pipeline on SOL with its memory destroyed (block-shuffled), 2 seeds');
const nulls = [101, 202].map((sd) => runAll(shuffledCandles(sol, sd), sd));
nulls.forEach((n, k) => { const o = n.trades.filter((t) => t.ok); log(line(`shuffled #${k + 1} all fills`, o)); log(line(`shuffled #${k + 1} GO`, o.filter((t) => t.go))); });
log(`seconds total ${((Date.now() - t0) / 1000).toFixed(0)}`);

mkdirSync(new URL('../../out/', import.meta.url), { recursive: true });
writeFileSync(new URL('../../out/backtest.md', import.meta.url), '```\n' + out.join('\n') + '\n```\n');
writeFileSync(new URL('../../out/backtest_rules.json', import.meta.url), JSON.stringify(rt, null, 1));
