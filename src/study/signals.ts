// `kev signals SYMBOL`: every causal fill kev would have taken (and its matched random orders), with the features known at the fill
// and the outcome and exit time per outcome key. The Python paper book (kev-lab) trades from these rows.
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openMarket } from '../core/market-io.js';
import { withPreset } from '../core/presets.js';
import { placebos, runBacktest, type Scan, type Trade } from '../backtest/engine.js';
import { ltfIndex } from '../backtest/settle.js';

export const SIG_DIR = fileURLToPath(new URL('../../out/signals/', import.meta.url));

export function signals(sym: string, preset = 'one-hour-spec', from?: number) {
  const m = openMarket(sym), c = m.h1, p = m.profile;
  const start = Math.max(c.findIndex((b) => b.t >= p.data.from + 45 * 86_400_000), from ? c.findIndex((b) => b.t >= from) : 0);
  return withPreset(preset, () => {
    const bt = runBacktest(c, { start, end: 1e9, session: m.session, inst: m.instrument, spreadBps: p.costs.spreadBps, slipR: p.costs.slipR, entryAtLevel: true, ltf: ltfIndex(m.ltf()) });
    const pl = placebos(c, bt.trades, new Map<number, Scan>(bt.scans.map((s) => [s.j, s])), bt.cfg, 1);
    const row = (t: Trade) => ({
      mkt: sym, t: t.t, dir: t.dir === 'long' ? 1 : -1, px: t.px, fill: t.fill, atr: t.atrD1, n: t.touches, sp: t.spaceOk ? 1 : 0, far: +t.farAtr.toFixed(3), go: t.go ? 1 : 0,
      trendPct: t.ctx?.trendPct ?? 'range', trend: t.ctx?.trend ?? 'range', vol: +(t.ctx?.volPct ?? 0.5).toFixed(3), htf: t.ctx?.htf ?? 'neutral',
      r: Object.fromEntries(Object.entries(t.r).filter(([, v]) => Number.isFinite(v)).map(([k, v]) => [k, +v.toFixed(4)])),
      xt: Object.fromEntries(Object.entries(t.x).map(([k, i]) => [k, c[i]?.t ?? t.t])),
    });
    const out = { symbol: sym, preset, cls: p.cls, costs: p.costs, from: c[start]!.t, to: c.at(-1)!.t, lastBar: c.at(-1)!.t, trades: bt.trades.map(row), placebo: pl.map(row) };
    mkdirSync(SIG_DIR, { recursive: true });
    writeFileSync(`${SIG_DIR}${sym}.json`, JSON.stringify(out));
    return { n: out.trades.length, placebo: out.placebo.length, from: out.from, to: out.to };
  });
}
