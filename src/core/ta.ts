import type { Candle } from './types.js';
import { P } from './params.js';

export const rng = (b: Candle) => b.h - b.l;
export const bodyOf = (b: Candle) => Math.abs(b.c - b.o);

export function wilderAtr(c: readonly Candle[], n: number = P.atrN): number {
  if (c.length < 2) return 0;
  const tr: number[] = [];
  for (let i = 1; i < c.length; i++) tr.push(Math.max(c[i]!.h - c[i]!.l, Math.abs(c[i]!.h - c[i - 1]!.c), Math.abs(c[i]!.l - c[i - 1]!.c)));
  if (tr.length < n) return tr.reduce((a, b) => a + b, 0) / tr.length;
  let a = tr.slice(0, n).reduce((x, y) => x + y, 0) / n;
  for (let i = n; i < tr.length; i++) a = (a * (n - 1) + tr[i]!) / n;
  return a;
}

export interface Fractal { i: number; side: 'high' | 'low'; price: number }

/** Confirmed fractal swings (needs n bars after), left side strict, right side non-strict so equal highs both register. */
export function fractals(c: readonly Candle[], n: number = P.fractalN): Fractal[] {
  const out: Fractal[] = [];
  for (let i = n; i < c.length - n; i++) {
    let hi = true, lo = true;
    for (let k = 1; k <= n; k++) {
      if (!(c[i]!.h > c[i - k]!.h && c[i]!.h >= c[i + k]!.h)) hi = false;
      if (!(c[i]!.l < c[i - k]!.l && c[i]!.l <= c[i + k]!.l)) lo = false;
    }
    if (hi) out.push({ i, side: 'high', price: c[i]!.h });
    if (lo) out.push({ i, side: 'low', price: c[i]!.l });
  }
  return out;
}

/**
 * Swing in / swing out (the lesson's depth measure). Draw a line at the swing price and run it left until it hits a
 * candle and right until the sweep; the lowest (highest) point of each section is the swing in / out. Depth = the smaller.
 * Returns sweptAt = first bar that trades through the swing (null if never).
 */
export function swingInOut(c: readonly Candle[], f: Fractal) {
  const P0 = f.price;
  const hit = (b: Candle) => (f.side === 'high' ? b.h >= P0 : b.l <= P0);
  const hitStrict = (b: Candle) => (f.side === 'high' ? b.h > P0 : b.l < P0);
  const far = (b: Candle) => (f.side === 'high' ? b.l : b.h);
  const pick = f.side === 'high' ? Math.min : Math.max;
  let left = f.side === 'high' ? Infinity : -Infinity, right = left;
  for (let j = f.i - 1; j >= 0 && !hit(c[j]!); j--) left = pick(left, far(c[j]!));
  let sweptAt: number | null = null;
  for (let k = f.i + 1; k < c.length; k++) {
    if (hitStrict(c[k]!)) { sweptAt = k; break; }
    right = pick(right, far(c[k]!));
  }
  const dist = (x: number) => (Number.isFinite(x) ? Math.abs(P0 - x) : 0);
  const swingIn = dist(left), swingOut = dist(right);
  return { swingIn, swingOut, depth: Math.min(swingIn, swingOut), sweptAt };
}

export const clamp01 = (x: number) => Math.max(0, Math.min(1, x));
