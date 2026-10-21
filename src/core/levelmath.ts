import type { Candle } from './types.js';
import { P } from './params.js';

export interface LevelAnalysis {
  found: boolean; role: 'support' | 'resistance' | null; center: number; zoneLo: number; zoneHi: number;
  breakIdx: number | null; breakT: number | null; barsSinceBreak: number;
  touches: number; lastTouchT: number | null; lxpbPrice: number | null; spanBars: number;
  chopCrossings: number; disrespected: boolean; tradable: boolean;
  touchT: [number, number][]; // [first, last] bar time of every counted touch cluster, for drawing; not a port field
}
export const empty = (center: number): LevelAnalysis => ({
  found: false, role: null, center, zoneLo: center, zoneHi: center, breakIdx: null, breakT: null, barsSinceBreak: 0,
  touches: 0, lastTouchT: null, lxpbPrice: null, spanBars: 0, chopCrossings: 0, disrespected: false, tradable: false, touchT: [],
});
export type Tier = 'blind' | 'needs-one-confluence' | 'needs-two-confluences' | 'none';
export const tierOf = (touches: number): Tier => (touches >= 3 ? 'blind' : touches === 2 ? 'needs-one-confluence' : touches === 1 ? 'needs-two-confluences' : 'none');

/**
 * Reconstructs a flipped level from CLOSED H1 bars. For a long the level must now be acting as support after an upside
 * break, so the touches that count ("opposite last") are the times it held as RESISTANCE before the break; mirror for a short.
 */
export function analyzeLevel(c: readonly Candle[], centre: number, dir: 'long' | 'short', atrH1: number): LevelAnalysis {
  const n = c.length, long = dir === 'long';
  if (n < 60 || !(atrH1 > 0)) return empty(centre);
  const zh = P.zoneHalfAtr * atrH1, lo = centre - zh, hi = centre + zh;
  const base = empty(centre);
  base.zoneLo = lo; base.zoneHi = hi; base.role = long ? 'support' : 'resistance';
  let cross = 0;
  for (let i = Math.max(1, n - P.chopWindow); i < n; i++) if (Math.sign(c[i]!.c - centre) !== Math.sign(c[i - 1]!.c - centre) && c[i - 1]!.c !== centre) cross++;
  base.chopCrossings = cross;
  let pre = -1;
  for (let i = n - 1; i >= 0; i--) if (long ? c[i]!.c < lo : c[i]!.c > hi) { pre = i; break; }
  let br = -1;
  if (pre >= 0) for (let j = pre + 1; j < n; j++) if (long ? c[j]!.c > hi : c[j]!.c < lo) { br = j; break; }
  if (br < 0) return base;
  let era = Math.max(0, br - P.eraLookbackBars);
  for (let i = br - 1; i >= 0; i--) if (long ? c[i]!.c > hi : c[i]!.c < lo) { era = i + 1; break; }
  const contact = (i: number) => (long ? c[i]!.h >= lo : c[i]!.l <= hi);
  const react = P.touchReactionAtr * atrH1;
  const away = (i: number) => (long ? lo - c[i]!.l : c[i]!.h - hi);
  const clusters: { s: number; e: number }[] = [];
  let prev = -1;
  for (let i = era; i < br; i++) {
    if (!contact(i)) continue;
    let split = prev < 0 || i - prev > P.touchClusterBars;
    if (!split) for (let k = prev + 1; k < i; k++) if (away(k) >= react) { split = true; break; }
    if (split) clusters.push({ s: i, e: i }); else clusters[clusters.length - 1]!.e = i;
    prev = i;
  }
  const counted: { s: number; e: number }[] = [];
  for (const cl of clusters) {
    for (let k = cl.e + 1; k < Math.min(br, cl.e + 1 + P.touchReactionBars); k++) if (away(k) >= react) { counted.push(cl); break; }
  }
  const last = counted[counted.length - 1] ?? null;
  let lxpb: number | null = null;
  if (last) {
    lxpb = long ? -Infinity : Infinity;
    for (let i = last.s; i <= last.e; i++) lxpb = long ? Math.max(lxpb, c[i]!.h) : Math.min(lxpb, c[i]!.l);
  }
  const chop = cross >= P.chopCrossings;
  return {
    ...base, found: true, breakIdx: br, breakT: c[br]!.t, barsSinceBreak: n - 1 - br, touches: counted.length,
    lastTouchT: last ? c[last.e]!.t : null, lxpbPrice: lxpb, spanBars: counted.length > 1 ? counted[counted.length - 1]!.e - counted[0]!.e : 0,
    disrespected: chop, tradable: counted.length >= 1 && !chop, touchT: counted.map((cl) => [c[cl.s]!.t, c[cl.e]!.t] as [number, number]),
  };
}
