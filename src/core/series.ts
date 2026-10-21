import type { Candle, Instrument, Knowable, NewsEvent, View } from './types.js';
import { rank } from './types.js';
import { P } from './params.js';

export const H = 3_600_000;
export const D = 24 * H;
export const W = 7 * D;
export const MONDAY = 4 * D; // 1970-01-05 00:00 UTC
export const SUNDAY_NOON = MONDAY - 12 * H; // week starts Sunday 12:00, so Sunday-evening FX/metals bars open the new week

export function aggregate(c: Candle[], period: number, anchor = 0): Candle[] {
  const out: Candle[] = [];
  for (const b of c) {
    const t = Math.floor((b.t - anchor) / period) * period + anchor;
    const last = out[out.length - 1];
    if (last && last.t === t) { last.h = Math.max(last.h, b.h); last.l = Math.min(last.l, b.l); last.c = b.c; last.v += b.v; }
    else out.push({ ...b, t });
  }
  return out;
}

export interface Feed { h1: Candle[]; d1: Candle[]; w1: Candle[]; period: number; weekAnchor: number }
/** `base` is the execution timeframe (H1 for the method; M30/H4 for robustness runs). It fills the 'h1' slot of every view. */
export function makeFeed(base: Candle[], opts: { period?: number; weekAnchor?: number } = {}): Feed {
  const period = opts.period ?? H, weekAnchor = opts.weekAnchor ?? MONDAY;
  return { h1: base, d1: aggregate(base, D), w1: aggregate(base, W, weekAnchor), period, weekAnchor };
}

/** Index of the last bar with t + period <= T, or -1. */
export function lastClosed(arr: Candle[], period: number, T: number): number {
  let lo = 0, hi = arr.length - 1, ans = -1;
  while (lo <= hi) {
    const m = (lo + hi) >> 1;
    if (arr[m]!.t + period <= T) { ans = m; lo = m + 1; } else hi = m - 1;
  }
  return ans;
}

function indexAtOrAfter(arr: Candle[], t: number): number {
  let lo = 0, hi = arr.length;
  while (lo < hi) { const m = (lo + hi) >> 1; if (arr[m]!.t < t) lo = m + 1; else hi = m; }
  return lo;
}

export const dayStart = (t: number) => Math.floor(t / D) * D;
export const weekStart = (t: number, anchor: number = MONDAY) => Math.floor((t - anchor) / W) * W + anchor;

/**
 * Builds the view a node of the given phase is allowed to see. Bars are cut to those CLOSED at the
 * decision time, so a prior_bars/fill_time node physically cannot read the fill bar's high, low or close.
 */
export function viewAt(feed: Feed, T: number, phase: Knowable, instrument: Instrument, opts: { price?: number; calendar?: NewsEvent[] } = {}): View {
  const P0 = feed.period;
  const tt = rank(phase) >= rank('bar_close') ? Math.ceil(T / P0) * P0 : T;
  const cut = (arr: Candle[], period: number, n: number) => {
    const k = lastClosed(arr, period, tt);
    return k < 0 ? [] : arr.slice(Math.max(0, k + 1 - n), k + 1);
  };
  const h1 = cut(feed.h1, P0, P.lookback.h1);
  let now: View['now'] = null;
  if (phase !== 'prior_bars') {
    const di = indexAtOrAfter(feed.h1, dayStart(T)), wi = indexAtOrAfter(feed.h1, weekStart(T, feed.weekAnchor));
    const price = phase === 'bar_open' ? (feed.h1[indexAtOrAfter(feed.h1, Math.floor(T / P0) * P0)]?.o ?? h1.at(-1)?.c ?? 0)
      : opts.price ?? h1.at(-1)?.c ?? 0;
    now = { t: T, price, dayOpen: feed.h1[di]?.o ?? price, weekOpen: feed.h1[wi]?.o ?? price };
  }
  return { period: P0, weekAnchor: feed.weekAnchor, asOf: tt, phase, instrument, h1, d1: cut(feed.d1, D, P.lookback.d1), w1: cut(feed.w1, W, P.lookback.w1), now, calendar: opts.calendar ?? [] };
}

export const SOL: Instrument = { symbol: 'SOLUSDT', cls: 'crypto', pip: 0.01 };
export const GOLD: Instrument = { symbol: 'XAUUSD', cls: 'metal', pip: 0.1 };
