// `kev events SYMBOL`: every causal H1 pattern event, for the pattern lab (kev-lab/patterns.py measures what price does next).
// Definitions are in kev-brain/experiments/2026-10-21-pattern-lab.md. Everything is known at the event bar's close.
//   sweep: first bar to trade beyond a confirmed swing (fractals, kev's fractalN) and close back inside (an SFP, relaxed: no visit/age vetoes)
//   break: first close beyond the most recent confirmed swing; msb = that swing was against the prior structure (lower high / higher low)
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openMarket } from '../core/market-io.js';
import { aggregateSession, H } from '../core/session.js';
import { fractals, swingInOut, type Fractal } from '../core/ta.js';
import { P } from '../core/params.js';
import type { Candle } from '../core/types.js';

export const EV_DIR = fileURLToPath(new URL('../../out/events/', import.meta.url));
const SWEEP_MAX_AGE = 120; // bars: older swings are not "liquidity" any more (5 days); age is recorded so it can be split

/** Prior-day Wilder ATR(20) for every H1 bar, from completed session days only. */
export function priorDayAtr(c: readonly Candle[], d1: readonly Candle[], n = 20): number[] {
  const atr: number[] = []; let a = NaN;
  for (let i = 0; i < d1.length; i++) {
    const tr = i === 0 ? d1[0]!.h - d1[0]!.l : Math.max(d1[i]!.h - d1[i]!.l, Math.abs(d1[i]!.h - d1[i - 1]!.c), Math.abs(d1[i]!.l - d1[i - 1]!.c));
    a = i < n ? (i === 0 ? tr : (a * i + tr) / (i + 1)) : (a * (n - 1) + tr) / n;
    atr.push(a);
  }
  const out: number[] = []; let j = -1;
  for (const b of c) { while (j + 1 < d1.length && d1[j + 1]!.e! <= b.t) j++; out.push(j >= n ? atr[j]! : NaN); }
  return out;
}

/** Volatility monitor: percentile of the prior-day ATR at bar k within the trailing year (one sample per day). */
function volPctAt(atr: number[], k: number): number {
  const w: number[] = [];
  for (let i = k - 24; i >= Math.max(0, k - 24 * 365); i -= 24) if (Number.isFinite(atr[i]!)) w.push(atr[i]!);
  return w.length > 30 ? w.filter((x) => x <= atr[k]!).length / w.length : NaN;
}

export interface Ev { k: number; t: number; kind: 'sweep' | 'break'; dir: 1 | -1; level: number; deep: number; age: number; msb: number; closeLoc: number; barAtr: number }

export function detect(c: readonly Candle[], atr: number[]): Ev[] {
  const n = P.fractalN, fr = fractals(c, n), ev: Ev[] = [];
  // sweeps: each swing's first strict trade-through, if it is within SWEEP_MAX_AGE and the swing was confirmed before that bar
  const best = new Map<string, Ev>();
  for (const f of fr) {
    const s = swingInOut(c, f), k = s.sweptAt;
    if (k === null || k - f.i > SWEEP_MAX_AGE || f.i + n > k - 1 || !(atr[k]! > 0)) continue;
    const b = c[k]!, hi = f.side === 'high';
    if (hi ? !(b.c < f.price) : !(b.c > f.price)) continue; // closed through: that is a break, not a sweep
    const r = b.h - b.l;
    const e: Ev = { k, t: b.t, kind: 'sweep', dir: hi ? -1 : 1, level: f.price, deep: s.depth / atr[k]!, age: k - f.i, msb: 0,
      closeLoc: r > 0 ? (hi ? (b.h - b.c) / r : (b.c - b.l) / r) : 0.5, barAtr: r / atr[k]! };
    const key = `${k}:${e.dir}`, o = best.get(key);
    if (!o || e.deep > o.deep) best.set(key, e); // one event per bar and side: the deepest swing swept
  }
  ev.push(...best.values());
  // breaks of structure: walk bars, tracking the last two confirmed swing highs and lows
  const conf = new Map<number, Fractal[]>();
  for (const f of fr) { const at = f.i + n; (conf.get(at) ?? conf.set(at, []).get(at)!).push(f); }
  const highs: Fractal[] = [], lows: Fractal[] = [];
  let usedH = -1, usedL = -1;
  for (let k = 0; k < c.length; k++) {
    const b = c[k]!, h = highs.at(-1), l = lows.at(-1);
    if (atr[k]! > 0) {
      if (h && h.i !== usedH && b.c > h.price) {
        usedH = h.i; const prev = highs.at(-2);
        ev.push({ k, t: b.t, kind: 'break', dir: 1, level: h.price, deep: 0, age: k - h.i, msb: prev && h.price < prev.price ? 1 : 0, closeLoc: (b.c - b.l) / Math.max(1e-12, b.h - b.l), barAtr: (b.h - b.l) / atr[k]! });
      }
      if (l && l.i !== usedL && b.c < l.price) {
        usedL = l.i; const prev = lows.at(-2);
        ev.push({ k, t: b.t, kind: 'break', dir: -1, level: l.price, deep: 0, age: k - l.i, msb: prev && l.price > prev.price ? 1 : 0, closeLoc: (b.h - b.c) / Math.max(1e-12, b.h - b.l), barAtr: (b.h - b.l) / atr[k]! });
      }
    }
    for (const f of conf.get(k) ?? []) (f.side === 'high' ? highs : lows).push(f); // confirmed at this bar's close: usable from k+1
  }
  return ev.sort((a, b) => a.k - b.k);
}

export function events(sym: string) {
  const m = openMarket(sym), c = m.h1, d1 = aggregateSession(c, m.session, 'day');
  const atr = priorDayAtr(c, d1), ev = detect(c, atr);
  const cols = <K extends keyof Ev>(key: K) => ev.map((e) => (typeof e[key] === 'number' ? +(e[key] as number).toPrecision(6) : e[key]));
  const out = {
    symbol: sym, hourMs: H, n: ev.length,
    ev: { k: cols('k'), t: cols('t'), kind: cols('kind'), dir: cols('dir'), level: cols('level'), deep: cols('deep'), age: cols('age'), msb: cols('msb'), closeLoc: cols('closeLoc'), barAtr: cols('barAtr') },
    vol: ev.map((e) => { const v = volPctAt(atr, e.k); return Number.isFinite(v) ? +v.toFixed(3) : null; }),
    atr: atr.map((x) => (Number.isFinite(x) ? +x.toPrecision(6) : null)),
  };
  mkdirSync(EV_DIR, { recursive: true });
  writeFileSync(`${EV_DIR}${sym}.json`, JSON.stringify(out));
  return { n: ev.length, sweeps: ev.filter((e) => e.kind === 'sweep').length, breaks: ev.filter((e) => e.kind === 'break').length, msb: ev.filter((e) => e.msb).length };
}
