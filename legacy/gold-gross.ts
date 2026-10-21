import { runGold, loadGold, ledger, isIS, isOOS, type Tf } from './gold.js';
import { clusterMean, clusterDiff, f, mean } from './stats.js';
import { placebos, type RKey, type Scan } from './engine.js';

const tf = (process.argv.find((a) => a.startsWith('--tf='))?.slice(5) ?? 'h1') as Tf;
const c = loadGold(tf);
const costs: [string, { slipR: number; spreadBps: number }][] = [['zero costs', { slipR: 0, spreadBps: 0 }], ['spread only (1.2 bps)', { slipR: 0, spreadBps: 1.2 }], ['full (1.2 bps + 0.10R)', { slipR: 0.1, spreadBps: 1.2 }]];
const years = (c.at(-1)!.t - Date.UTC(2010, 5, 1)) / (365.25 * 86_400_000);
for (const [name, cost] of costs) {
  const bt = runGold(c, tf, cost, true), ok = bt.trades.filter((t) => t.ok);
  const row = (k: RKey, ts = ok) => { const m = clusterMean(ts.filter((t) => t.r[k] !== undefined).map((t) => ({ r: t.r[k]!, day: t.day })), 200); return `${f(m.mean, 3)} [${f(m.lo, 2)},${f(m.hi, 2)}]`; };
  console.log(`\n== ${tf.toUpperCase()} ${name}: n=${ok.length}`);
  console.log(`   all fills   2R ${row('2')}   4R ${row('4')}   FTA ${row('fta')}`);
  console.log(`   GO          2R ${row('2', ok.filter((t) => t.go))}   4R ${row('4', ok.filter((t) => t.go))}   FTA ${row('fta', ok.filter((t) => t.go))}`);
  console.log(`   placebo     2R ${row('2', bt.placebo)}   4R ${row('4', bt.placebo)}`);
  const l = ledger(ok, '4', years), lo = ledger(ok.filter(isOOS), '4', years - 8.6), li = ledger(ok.filter(isIS), '4', 8.6);
  console.log(`   one-at-a-time 4R: ${l.n} trades, win ${f(100 * l.win, 1)}%, mean ${f(l.meanR, 3)}R, $${f(l.perYear, 0)}/yr, Sharpe ${f(l.sharpe, 2)} | IS $${f(li.perYear, 0)}/yr | OOS $${f(lo.perYear, 0)}/yr Sharpe ${f(lo.sharpe, 2)}`);
  void clusterDiff; void mean; void placebos; void (null as unknown as Scan);
}
