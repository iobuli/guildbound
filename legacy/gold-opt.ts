// One job: run the gold backtest for one threshold configuration (on real or shuffled gold) and print one JSON line.
import { P } from '../core/params.js';
import { scaleForPeriod } from '../core/scale.js';
import { H } from '../core/series.js';
import { shuffledCandles } from './nullmodel.js';
import { loadGold, runGold, isIS, isOOS, ledger, TFS, type Tf } from './gold.js';
import { mean } from './stats.js';
import type { Trade } from './engine.js';

const arg = (k: string, d: string) => process.argv.find((a) => a.startsWith(`--${k}=`))?.slice(k.length + 3) ?? d;
const tf = arg('tf', 'h1') as Tf, nullSeed = Number(arg('null', '0'));
const cfg = { zoneHalfAtr: Number(arg('zone', '0.25')), touchReactionAtr: Number(arg('react', '1')), retestMinExcursionAtr: Number(arg('space', '0.5')) };
let c = loadGold(tf);
if (nullSeed) c = shuffledCandles(c, nullSeed, Math.round(24 * H / TFS[tf]));
const restore = scaleForPeriod(TFS[tf]);
Object.assign(P, cfg);
const { trades, cfg: bc } = runGold(c, tf);
restore();
const years = (c.at(-1)!.t - c[bc.start]!.t) / (365.25 * 86_400_000);
const stat = (pick: (t: Trade) => boolean) => {
  const ts = trades.filter((t) => t.ok && pick(t));
  const m = (k: '2' | '4') => mean(ts.filter((t) => t.r[k] !== undefined).map((t) => t.r[k]!));
  return { n: ts.length, m2: m('2'), m4: m('4') };
};
const oosYears = years - (2019 - 2010.4);
console.log(JSON.stringify({
  tf, nullSeed, cfg,
  goIS: stat((t) => t.go && isIS(t)), goOOS: stat((t) => t.go && isOOS(t)), allIS: stat(isIS), allOOS: stat(isOOS),
  oos4: (() => { const l = ledger(trades.filter((t) => isOOS(t)), '4', oosYears); return { n: l.n, perYear: l.perYear, sharpe: l.sharpe }; })(),
  goOos4: (() => { const l = ledger(trades.filter((t) => isOOS(t) && t.go), '4', oosYears); return { n: l.n, perYear: l.perYear, sharpe: l.sharpe }; })(),
}));
