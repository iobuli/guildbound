// One harness run on one ingested market under one preset (and optionally a different session), with matched placebos.
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openMarket } from '../core/market-io.js';
import { withPreset } from '../core/presets.js';
import { buildSession, type DayBoundary } from '../core/session.js';
import { placebos, runBacktest, type Scan, type Trade } from '../backtest/engine.js';
import { ltfIndex } from '../backtest/settle.js';
import { compact } from './analyze.js';

export const OUT = fileURLToPath(new URL('../../out/bt/', import.meta.url));
export interface RunOpts { preset: string; session?: DayBoundary | 'utc-raw'; ltf?: boolean; from?: number; to?: number; placeboReps?: number }
export const runId = (sym: string, o: RunOpts) => `${sym}__${o.preset}${o.session ? `__${o.session}` : ''}`;

export function runMarket(sym: string, o: RunOpts, log: (s: string) => void = () => {}) {
  const m = openMarket(sym), p = m.profile, c = m.h1;
  const spec = o.session === undefined ? p.session : o.session === 'utc-raw' ? { ...p.session, day: 'utc' as const, stubHours: 0 } : { ...p.session, day: o.session };
  const session = buildSession(spec, c);
  const warm = c.findIndex((b) => b.t >= p.data.from + 45 * 86_400_000);
  const start = Math.max(warm, o.from ? c.findIndex((b) => b.t >= o.from!) : 0), end = o.to ? c.findIndex((b) => b.t >= o.to!) : 1e9;
  const ltf = o.ltf === false ? undefined : ltfIndex(m.ltf());
  const t0 = Date.now();
  return withPreset(o.preset, () => {
    const bt = runBacktest(c, { start, end: end < 0 ? 1e9 : end, session, inst: m.instrument, spreadBps: p.costs.spreadBps, slipR: p.costs.slipR, entryAtLevel: true, ltf },
      (j) => log(`${sym} ${o.preset}: ${new Date(c[j]!.t).toISOString().slice(0, 10)} (${((Date.now() - t0) / 1000).toFixed(0)}s)`));
    const pl = placebos(c, bt.trades, new Map<number, Scan>(bt.scans.map((s) => [s.j, s])), bt.cfg, o.placeboReps ?? 2);
    const res = { symbol: sym, preset: o.preset, full: !o.from && !o.to, session: spec, split: p.split, costs: p.costs, seconds: (Date.now() - t0) / 1000, scans: bt.scans.length, trades: bt.trades, placebo: pl };
    mkdirSync(OUT, { recursive: true });
    writeFileSync(`${OUT}${runId(sym, o)}.json`, JSON.stringify({ ...res, trades: bt.trades.map(compact), placebo: pl.map(compact) }));
    return res;
  });
}
export type RunResult = ReturnType<typeof runMarket> & { trades: Trade[]; placebo: Trade[] };
