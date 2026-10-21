import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { runBacktest, placebos, KEYS, type Trade, type RKey, type Scan, type BtConfig } from './engine.js';
import { clusterMean, clusterDiff, benjaminiHochberg, mean, sd, f } from './stats.js';
import { shuffledCandles } from './nullmodel.js';
import { scaleForPeriod } from '../core/scale.js';
import { H } from '../core/series.js';
import { INSTRUMENTS } from '../core/instruments.js';
import type { Candle } from '../core/types.js';

export const TFS = { m30: H / 2, h1: H, h4: 4 * H } as const;
export type Tf = keyof typeof TFS;
export const loadGold = (tf: string): Candle[] => JSON.parse(readFileSync(new URL(`../../data/XAUUSD_${tf}.json`, import.meta.url), 'utf8'));
export const IS_END = 2018; // in-sample 2010-2018, out-of-sample 2019-2026
export const isIS = (t: Trade) => t.year <= IS_END, isOOS = (t: Trade) => t.year > IS_END;
const rows = (ts: Trade[], k: RKey) => ts.filter((t) => t.ok && t.r[k] !== undefined).map((t) => ({ r: t.r[k]!, day: t.day }));

export function goldCfg(c: Candle[], tf: Tf, extra: Partial<BtConfig> = {}) {
  const start = c.findIndex((b) => b.t >= Date.UTC(2010, 5, 1));
  const g = INSTRUMENTS.XAUUSD!;
  return { start, end: 1e9, period: TFS[tf], weekAnchor: g.weekAnchor, inst: g.instrument, spreadBps: g.spreadBps, slipR: g.slipR, ...extra };
}
export function runGold(c: Candle[], tf: Tf, extra: Partial<BtConfig> = {}, withPlacebo = false) {
  const bt = runBacktest(c, goldCfg(c, tf, extra));
  const pl = withPlacebo ? placebos(c, bt.trades, new Map<number, Scan>(bt.scans.map((s) => [s.j, s])), bt.cfg) : [];
  return { ...bt, placebo: pl };
}

/** One position at a time: take a fill only if the previous accepted trade (for this target) has already exited. Dollars at $500 risk per R, no compounding. */
export function ledger(ts: Trade[], k: RKey, years: number) {
  const seq = ts.filter((t) => t.ok && t.r[k] !== undefined).sort((a, b) => a.i - b.i);
  let free = -1; const taken: Trade[] = [];
  for (const t of seq) if (t.i > free) { taken.push(t); free = t.x[k]!; }
  const daily = new Map<number, number>();
  for (const t of taken) daily.set(t.day, (daily.get(t.day) ?? 0) + t.r[k]! * 500);
  const ds = [...daily.values()], tradingDays = Math.round(years * 252);
  const m = ds.reduce((a, b) => a + b, 0) / tradingDays, v = ds.reduce((a, b) => a + (b - m) ** 2, 0) / tradingDays - 0;
  const sharpe = Math.sqrt(Math.max(1e-12, ds.reduce((a, b) => a + b * b, 0) / tradingDays - m * m)) > 0 ? (m / Math.sqrt(Math.max(1e-12, ds.reduce((a, b) => a + b * b, 0) / tradingDays - m * m))) * Math.sqrt(252) : 0;
  void v;
  let eq = 0, peak = 0, dd = 0; for (const t of taken) { eq += t.r[k]!; peak = Math.max(peak, eq); dd = Math.max(dd, peak - eq); }
  return { n: taken.length, win: taken.filter((t) => t.r[k]! > 0).length / Math.max(1, taken.length), meanR: mean(taken.map((t) => t.r[k]!)), perYear: taken.reduce((a, t) => a + t.r[k]! * 500, 0) / years, sharpe, maxDdR: dd };
}

function line(name: string, ts: Trade[], keys: RKey[] = ['1', '2', '3', '4', 'fta']) {
  const cells = keys.map((k) => { const m = clusterMean(rows(ts, k), 250); return Number.isFinite(m.mean) ? `${f(m.mean, 2)} [${f(m.lo, 2)},${f(m.hi, 2)}]` : 'n/a'; });
  return `${name.padEnd(28)} n=${String(ts.filter((t) => t.ok).length).padStart(6)}  ` + cells.map((c, i) => `${keys[i]}R ${c}`).join('  ');
}

async function main() {
  const tf = (process.argv.find((a) => a.startsWith('--tf='))?.slice(5) ?? 'h1') as Tf;
  const c = loadGold(tf), t0 = Date.now(), out: string[] = [];
  const log = (s = '') => { console.log(s); out.push(s); };
  const restore = scaleForPeriod(TFS[tf]);
  const real = runGold(c, tf, {}, true), ok = real.trades.filter((t) => t.ok);
  const years = (c.at(-1)!.t - c[real.cfg.start]!.t) / (365.25 * 86_400_000);
  log(`# XAUUSD ${tf.toUpperCase()} base, ${new Date(c[real.cfg.start]!.t).toISOString().slice(0, 10)} to ${new Date(c.at(-1)!.t).toISOString().slice(0, 10)} (${years.toFixed(1)} years)`);
  log(`${real.scans.length} scans, ${real.trades.length} fills (${ok.length} valid stop). Costs: 1.2 bps spread + 0.10R slippage. Mean net R per trade [95% CI, bootstrapped over days]. In-sample 2010-${IS_END}, out-of-sample ${IS_END + 1}+.\n`);
  log('## 1. Everything the system would fill (overlapping fills allowed)');
  log(line('all fills', ok)); log(line('  in-sample', ok.filter(isIS))); log(line('  out-of-sample', ok.filter(isOOS)));
  log(line('  longs', ok.filter((t) => t.dir === 'long'))); log(line('  shorts', ok.filter((t) => t.dir === 'short')));
  log('\n## 2. One position at a time, $500 risk per R, no compounding (the way your backtest page counts)');
  for (const k of ['2', '3', '4', 'fta'] as RKey[]) { const l = ledger(ok, k, years), lo = ledger(ok.filter(isOOS), k, years - (IS_END + 1 - 2010 - 5 / 12)); log(`${k.padEnd(3)}R target: ${String(l.n).padStart(5)} trades, win ${f(100 * l.win, 1)}%, mean ${f(l.meanR, 3)}R, $${f(l.perYear, 0)}/yr, Sharpe ${f(l.sharpe, 2)}, max DD ${f(l.maxDdR, 1)}R   | OOS only: $${f(lo.perYear, 0)}/yr Sharpe ${f(lo.sharpe, 2)}`); }
  log('\n## 3. Against random orders (same side, distance, risk, window)');
  log(line('real levels', ok)); log(line('placebo', real.placebo));
  log(line('  real, OOS', ok.filter(isOOS))); log(line('  placebo, OOS', real.placebo.filter(isOOS)));
  log('real minus placebo: ' + KEYS.filter((k) => k !== 'fta').map((k) => { const d = clusterDiff(rows(ok, k), rows(real.placebo, k)); return `${k}R ${f(d.diff, 3)} (p=${f(d.p, 2)})`; }).join('   '));
  log('longs vs placebo longs, shorts vs placebo shorts at 4R: ' + (['long', 'short'] as const).map((d) => { const x = clusterDiff(rows(ok.filter((t) => t.dir === d), '4'), rows(real.placebo.filter((t) => t.dir === d), '4')); return `${d} ${f(x.diff, 3)} (p=${f(x.p, 2)})`; }).join('   '));
  log('\n## 4. By the system\'s verdict and tier');
  log(line('GO', ok.filter((t) => t.go))); log(line('NO-GO', ok.filter((t) => !t.go)));
  log(line('  GO in-sample', ok.filter((t) => t.go && isIS(t)))); log(line('  GO out-of-sample', ok.filter((t) => t.go && isOOS(t))));
  for (const tier of ['blind', 'needs-one-confluence', 'needs-two-confluences']) log(line(`tier ${tier}`, ok.filter((t) => t.tier === tier)));
  const gd = clusterDiff(rows(ok.filter((t) => t.go), '4'), rows(ok.filter((t) => !t.go), '4'));
  log(`GO minus NO-GO at 4R: ${f(gd.diff, 3)}R (p=${f(gd.p, 2)})`);
  log('\n## 5. By year: mean net R at 4R, real vs placebo (all fills)');
  const ys = [...new Set(ok.map((t) => t.year))].sort();
  for (const y of ys) { const a = ok.filter((t) => t.year === y), b = real.placebo.filter((t) => t.year === y); log(`${y}  real ${f(mean(rows(a, '4').map((x) => x.r)), 2).padStart(6)} (n=${String(a.length).padStart(5)})   placebo ${f(mean(rows(b, '4').map((x) => x.r)), 2).padStart(6)} (n=${String(b.length).padStart(5)})   GO ${f(mean(rows(a.filter((t) => t.go), '4').map((x) => x.r)), 2).padStart(6)} (n=${a.filter((t) => t.go).length})`); }
  log('\n## 6. Each rule alone: PASS vs FAIL at 4R, with in-sample / out-of-sample consistency');
  const ruleIds = new Set<string>(); for (const t of ok) for (const r of Object.keys(t.checks)) ruleIds.add(r);
  const rt: any[] = [];
  for (const r of ruleIds) {
    const Pp = ok.filter((t) => t.checks[r] === true), Ff = ok.filter((t) => t.checks[r] === false);
    if (Pp.length < 60 || Ff.length < 60) continue;
    const dd = clusterDiff(rows(Pp, '4'), rows(Ff, '4'), 500);
    const sub = (fn: (t: Trade) => boolean) => mean(rows(Pp.filter(fn), '4').map((x) => x.r)) - mean(rows(Ff.filter(fn), '4').map((x) => x.r));
    rt.push({ rule: r, nP: Pp.length, nF: Ff.length, mP: mean(rows(Pp, '4').map((x) => x.r)), mF: mean(rows(Ff, '4').map((x) => x.r)), diff: dd.diff, p: dd.p, isD: sub(isIS), oosD: sub(isOOS) });
  }
  const q = benjaminiHochberg(rt.map((x) => x.p)); rt.forEach((x, k) => (x.q = q[k]));
  for (const x of rt.sort((a, b) => a.p - b.p)) log(`${x.rule.padEnd(40)} pass n=${String(x.nP).padStart(5)} ${f(x.mP, 2)}R | fail n=${String(x.nF).padStart(5)} ${f(x.mF, 2)}R | diff ${f(x.diff, 2).padStart(6)} p=${f(x.p, 3)} q=${f(x.q, 3)}  IS ${f(x.isD, 2)} OOS ${f(x.oosD, 2)}`);
  if (!process.argv.includes('--no-null')) {
    log('\n## 7. The same pipeline on gold with its memory destroyed (block-shuffled by day), 2 seeds');
    for (const sd0 of [101, 202]) {
      const sh = shuffledCandles(c, sd0, Math.round(24 * H / TFS[tf]));
      const n = runGold(sh, tf, {}), o = n.trades.filter((t) => t.ok);
      log(line(`shuffled #${sd0} all fills`, o)); log(line(`shuffled #${sd0} GO`, o.filter((t) => t.go)));
      const l4 = ledger(o, '4', years); log(`   one-at-a-time 4R: ${l4.n} trades, mean ${f(l4.meanR, 3)}R, $${f(l4.perYear, 0)}/yr, Sharpe ${f(l4.sharpe, 2)}`);
    }
  }
  restore();
  log(`\nseconds ${((Date.now() - t0) / 1000).toFixed(0)}`);
  mkdirSync(new URL('../../out/', import.meta.url), { recursive: true });
  writeFileSync(new URL(`../../out/gold_${tf}.md`, import.meta.url), '```\n' + out.join('\n') + '\n```\n');
  writeFileSync(new URL(`../../out/gold_${tf}_rules.json`, import.meta.url), JSON.stringify(rt, null, 1));
}
if (process.argv[1]?.endsWith('gold.ts')) await main();
