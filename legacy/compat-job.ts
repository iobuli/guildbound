// One job: the SOL Trade Check spec (every multiple in DAILY-ATR units) run causally through the node graph. Prints one JSON line.
import { readFileSync } from 'node:fs';
import { P } from '../core/params.js';
import { H, MONDAY } from '../core/series.js';
import { SOL } from '../core/series.js';
import { INSTRUMENTS } from '../core/instruments.js';
import { runBacktest, placebos, type Scan, type Trade, type RKey, type BtConfig } from './engine.js';
import { ledger, loadGold } from './gold.js';
import { clusterMean, mean } from './stats.js';
import type { Candle } from '../core/types.js';

const arg = (k: string, d: string) => process.argv.find((a) => a.startsWith(`--${k}=`))?.slice(k.length + 3) ?? d;
const mkt = arg('mkt', 'sol'), space = Number(arg('space', '0.5')), minTier = Number(arg('tier', '2')), held = arg('held', '0') === '1', react = Number(arg('react', '0.5'));
Object.assign(P, { levelScaleD1: 1, zoneHalfAtr: 0.05, touchReactionAtr: react, retestMinExcursionAtr: space, touchClusterBars: 12, scanClusterAtr: 0.1 });
const gold = mkt === 'gold';
const c: Candle[] = gold ? loadGold('h1') : JSON.parse(readFileSync(new URL('../../data/SOLUSDT_1h.json', import.meta.url), 'utf8'));
const cfgI = INSTRUMENTS[gold ? 'XAUUSD' : 'SOLUSDT']!;
const split = gold ? 2018 : 2023;
const start = c.findIndex((b) => b.t >= (gold ? Date.UTC(2010, 5, 1) : Date.UTC(2021, 0, 1)));
const cfg: Partial<BtConfig> & { start: number; end: number } = { start, end: 1e9, inst: cfgI.instrument, weekAnchor: cfgI.weekAnchor, spreadBps: cfgI.spreadBps, slipR: cfgI.slipR, heldExit: held, minTier, entryAtLevel: true };
const bt = runBacktest(c, cfg);
const pl = placebos(c, bt.trades, new Map<number, Scan>(bt.scans.map((s) => [s.j, s])), bt.cfg);
const ok = bt.trades.filter((t) => t.ok);
const years = (c.at(-1)!.t - c[start]!.t) / (365.25 * 86_400_000);
const m = (ts: Trade[], k: RKey) => { const x = clusterMean(ts.filter((t) => t.r[k] !== undefined).map((t) => ({ r: t.r[k]!, day: t.day })), 200); return { n: x.n, mean: x.mean, lo: x.lo, hi: x.hi }; };
const l4 = ledger(ok, '4', years), l2 = ledger(ok, '2', years);
console.log(JSON.stringify({
  mkt, space, react, minTier, held, fills: ok.length, perYear: ok.length / years,
  r2: m(ok, '2'), r4: m(ok, '4'), fta: m(ok, 'fta'), pl4: m(pl, '4'), is4: m(ok.filter((t) => t.year <= split), '4'), oos4: m(ok.filter((t) => t.year > split), '4'),
  one4: { n: l4.n, win: l4.win, mean: l4.meanR, perYear: l4.perYear, sharpe: l4.sharpe }, one2: { n: l2.n, win: l2.win, mean: l2.meanR, perYear: l2.perYear, sharpe: l2.sharpe },
  go4: m(ok.filter((t) => t.go), '4'), tiers: ok.reduce((a: Record<string, number>, t) => ((a[String(t.touches)] = (a[String(t.touches)] ?? 0) + 1), a), {}), avgRisk: mean(ok.map((t) => t.risk / t.fill)),
}));
void H; void MONDAY; void SOL;
