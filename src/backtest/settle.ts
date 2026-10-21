// Settling a filled order. With only H1 bars the order of events inside a bar is unknown, so the rules are conservative:
// a fill bar that reaches the stop is a loss, targets are not credited on the fill bar, and a bar that holds both stop and
// target counts as the stop. When finer bars cover the hour, they decide instead, and the trade records resolvedBy = 'ltf'.
import type { Candle } from '../core/types.js';

const H = 3_600_000;

/** Index of the finer series by hour, finest first, used only where it covers a whole hour. */
export interface Ltf { hours: (hourStart: number) => Candle[] | null }
export function ltfIndex(series: { tf: number; c: Candle[] }[]): Ltf | undefined {
  if (!series.length) return undefined;
  const idx = series.map(({ tf, c }) => {
    const m = new Map<number, [number, number]>();
    for (let i = 0; i < c.length; i++) { const k = Math.floor(c[i]!.t / H) * H, r = m.get(k); if (r) r[1] = i; else m.set(k, [i, i]); }
    return { tf, c, m, need: H / tf };
  }).sort((a, b) => a.tf - b.tf);
  return {
    hours: (h) => {
      for (const s of idx) { const r = s.m.get(h); if (r && r[1] - r[0] + 1 === s.need) return s.c.slice(r[0], r[1] + 1); }
      return null;
    },
  };
}

export interface Settled { r: Record<string, number>; x: Record<string, number>; resolvedBy: 'h1' | 'ltf' }

/**
 * `targets` maps a key to a target price. Returns R per key (net of `slip` R) and the H1 index where each key exited.
 * `heldLevel`: optional exit at the fill bar's close if it closes on the wrong side of the level (knowable only after the bar).
 */
export function settleOrder(c: readonly Candle[], j: number, long: boolean, entry: number, stop: number, targets: Record<string, number>, maxHold: number, slip: number, opt: { ltf?: Ltf; heldLevel?: number } = {}): Settled {
  const risk = Math.abs(entry - stop), r: Record<string, number> = {}, x: Record<string, number> = {};
  const left = new Set(Object.keys(targets).filter((k) => (long ? targets[k]! > entry : targets[k]! < entry)));
  let resolvedBy: Settled['resolvedBy'] = 'h1';
  const stopHit = (b: Candle) => (long ? b.l <= stop : b.h >= stop), tgtHit = (b: Candle, p: number) => (long ? b.h >= p : b.l <= p);
  const lose = (at: number) => { for (const k of left) { r[k] = -1 - slip; x[k] = at; } left.clear(); };
  /** walks finer bars of one hour in order; within each finer bar the stop still wins a tie */
  const walk = (sub: readonly Candle[], at: number, from = 0) => {
    for (let s = from; s < sub.length && left.size; s++) {
      if (stopHit(sub[s]!)) { lose(at); return; }
      for (const k of [...left]) if (tgtHit(sub[s]!, targets[k]!)) { r[k] = Math.abs(targets[k]! - entry) / risk - slip; x[k] = at; left.delete(k); }
    }
  };
  // ---- the fill bar ----
  const fb = c[j]!, sub = opt.ltf?.hours(fb.t);
  if (sub && opt.heldLevel === undefined) {
    const f = sub.findIndex((b) => (long ? b.l <= entry : b.h >= entry));
    if (f >= 0) {
      resolvedBy = 'ltf';
      if (stopHit(sub[f]!)) lose(j); // the stop inside the very bar that filled: the fill came first, so it is a loss
      else walk(sub, j, f + 1);
    } else if (stopHit(fb)) lose(j);
  } else {
    if (stopHit(fb)) lose(j);
    else if (opt.heldLevel !== undefined && (long ? fb.c < opt.heldLevel : fb.c > opt.heldLevel)) {
      const rr = (long ? fb.c - entry : entry - fb.c) / risk - slip;
      for (const k of left) { r[k] = rr; x[k] = j; }
      left.clear();
    }
  }
  // ---- the bars after ----
  for (let t = j + 1; t <= j + maxHold && t < c.length && left.size; t++) {
    const b = c[t]!, stopped = stopHit(b), anyTgt = [...left].some((k) => tgtHit(b, targets[k]!));
    if (stopped && anyTgt && opt.ltf) { const s2 = opt.ltf.hours(b.t); if (s2) { resolvedBy = 'ltf'; walk(s2, t); continue; } }
    if (stopped) { lose(t); break; }
    for (const k of [...left]) if (tgtHit(b, targets[k]!)) { r[k] = Math.abs(targets[k]! - entry) / risk - slip; x[k] = t; left.delete(k); }
  }
  const end = Math.min(c.length - 1, j + maxHold), last = c[end]!;
  for (const k of left) { r[k] = (long ? last.c - entry : entry - last.c) / risk - slip; x[k] = end; }
  // targets on the wrong side of the entry never trade
  for (const k of Object.keys(targets)) if (r[k] === undefined) { r[k] = NaN; x[k] = j; }
  return { r, x, resolvedBy };
}
