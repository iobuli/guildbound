import { runGold, loadGold, ledger, isIS, isOOS, type Tf } from './gold.js';
import { clusterMean, f } from './stats.js';
import type { Trade, RKey } from './engine.js';
import type { BtConfig } from './engine.js';

const tf = (process.argv.find((a) => a.startsWith('--tf='))?.slice(5) ?? 'h1') as Tf;
const c = loadGold(tf);
const years = (c.at(-1)!.t - Date.UTC(2010, 5, 1)) / (365.25 * 86_400_000), oosY = years - 8.6;
const VARS: [string, Partial<BtConfig>][] = [
  ['baseline (all tiers)', {}],
  ['held-exit (all tiers)', { heldExit: true }],
  ['3+ touches only', { minTier: 3 }],
  ['3+ touches + held-exit', { minTier: 3, heldExit: true }],
  ['2+ touches + held-exit', { minTier: 2, heldExit: true }],
];
for (const [name, extra] of VARS) {
  const bt = runGold(c, tf, extra, true), ok = bt.trades.filter((t) => t.ok), pl = bt.placebo;
  const m = (ts: Trade[], k: RKey) => { const x = clusterMean(ts.filter((t) => t.r[k] !== undefined).map((t) => ({ r: t.r[k]!, day: t.day })), 200); return `${f(x.mean, 3)} [${f(x.lo, 2)},${f(x.hi, 2)}]`; };
  console.log(`\n== ${tf.toUpperCase()} ${name}: n=${ok.length}`);
  console.log(`   mean net R   1R ${m(ok, '1')}   2R ${m(ok, '2')}   4R ${m(ok, '4')}   | placebo 4R ${m(pl, '4')}   | OOS 4R ${m(ok.filter(isOOS), '4')}`);
  for (const k of ['2', '4'] as RKey[]) { const l = ledger(ok, k, years), li = ledger(ok.filter(isIS), k, 8.6), lo = ledger(ok.filter(isOOS), k, oosY); console.log(`   one-at-a-time ${k}R: ${String(l.n).padStart(5)} trades, win ${f(100 * l.win, 1)}%, mean ${f(l.meanR, 3)}R, $${f(l.perYear, 0)}/yr Sharpe ${f(l.sharpe, 2)} | IS $${f(li.perYear, 0)} S ${f(li.sharpe, 2)} | OOS $${f(lo.perYear, 0)} S ${f(lo.sharpe, 2)}`); }
}
