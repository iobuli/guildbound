// Measurements of a price series that decide how it must be treated: weekend regime, clock, consistency between timeframes,
// volume comparability, and which day boundary a daily file uses.
import type { Candle } from '../core/types.js';
import { nyOffsetH } from '../core/session.js';

const H = 3_600_000, D = 24 * H;
const year = (t: number) => new Date(t).getUTCFullYear();
const median = (xs: number[]) => { if (!xs.length) return NaN; const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]!; };

export interface Stats {
  n: number; from: number; to: number; tf: number;
  gaps: number; holes: number; weekendCloses: number;
  /** share of bars that fall on Saturday or Sunday (data clock), by year */
  weekendShare: Record<number, number>;
  /** hour of the first bar after each 24h+ gap: on the data clock, and on New York time if the data clock were UTC */
  reopenHour: Record<number, number>; reopenNyHour: Record<number, number>;
  volumeMedianByYear: Record<number, number>;
}

export function stats(c: readonly Candle[], tf: number): Stats {
  const s: Stats = { n: c.length, from: c[0]?.t ?? 0, to: c.at(-1)?.t ?? 0, tf, gaps: 0, holes: 0, weekendCloses: 0, weekendShare: {}, reopenHour: {}, reopenNyHour: {}, volumeMedianByYear: {} };
  const wk: Record<number, [number, number]> = {}, vol: Record<number, number[]> = {};
  for (let i = 0; i < c.length; i++) {
    const b = c[i]!, y = year(b.t), wd = new Date(b.t).getUTCDay();
    (wk[y] ??= [0, 0])[0]++; if (wd === 0 || wd === 6) wk[y]![1]++;
    (vol[y] ??= []).push(b.v);
    if (i === 0) continue;
    const g = b.t - c[i - 1]!.t;
    if (g > tf) s.gaps++;
    if (g >= D) {
      const prevWd = new Date(c[i - 1]!.t).getUTCDay();
      if (prevWd === 5 || prevWd === 6 || wd === 0 || wd === 1) s.weekendCloses++; else s.holes++;
      const h = new Date(b.t).getUTCHours(), nh = new Date(b.t + nyOffsetH(b.t) * H).getUTCHours();
      s.reopenHour[h] = (s.reopenHour[h] ?? 0) + 1; s.reopenNyHour[nh] = (s.reopenNyHour[nh] ?? 0) + 1;
    }
  }
  for (const [y, [n, w]] of Object.entries(wk)) s.weekendShare[+y] = +(w / n).toFixed(3);
  for (const [y, v] of Object.entries(vol)) s.volumeMedianByYear[+y] = median(v);
  return s;
}

const mode = (h: Record<number, number>) => { let k = -1, n = 0, tot = 0; for (const [a, b] of Object.entries(h)) { tot += b; if (b > n) { n = b; k = +a; } } return { k, share: tot ? n / tot : 0, tot }; };

/**
 * Infers how the file's clock relates to UTC from the weekly reopen. FX and metals reopen at 17:00-18:00 New York: if that is what we
 * see with the clock read as UTC, the clock is UTC. If the reopen sits at 00:00-01:00 on the data clock in summer and winter alike,
 * it is the common broker clock that runs 7 hours ahead of New York (UTC+2/+3).
 */
export function inferClock(s: Stats): { kind: 'fixed' | 'ny+7'; offsetHours: number; method: string; confident: boolean } {
  const ny = mode(s.reopenNyHour), dc = mode(s.reopenHour);
  if (ny.tot < 8) return { kind: 'fixed', offsetHours: 0, method: 'assumed UTC (fewer than 8 weekend reopens to infer from)', confident: false };
  if ((ny.k === 17 || ny.k === 18) && ny.share >= 0.6) return { kind: 'fixed', offsetHours: 0, method: `weekly reopen at ${ny.k}:00 New York on ${(100 * ny.share).toFixed(0)}% of ${ny.tot} weeks with the clock read as UTC, so the clock is UTC`, confident: true };
  if ((dc.k === 0 || dc.k === 1) && dc.share >= 0.6) return { kind: 'ny+7', offsetHours: 0, method: `weekly reopen at ${dc.k}:00 on the data clock in ${(100 * dc.share).toFixed(0)}% of ${dc.tot} weeks, through summer and winter: a New York + 7h broker clock (UTC+2/+3)`, confident: true };
  const off = ny.k >= 0 ? (((ny.k - 17) % 24) + 24) % 24 : 0;
  return { kind: 'fixed', offsetHours: off > 12 ? off - 24 : off, method: `weekly reopen at ${ny.k}:00 New York if read as UTC; offset estimated as ${off > 12 ? off - 24 : off}h (low confidence)`, confident: false };
}

/** Converts a data-clock time to UTC. */
export function toUtc(t: number, clock: { kind: 'fixed' | 'ny+7'; offsetHours: number }): number {
  if (clock.kind === 'fixed') return t - clock.offsetHours * H;
  const guess = t - (7 + nyOffsetH(t)) * H; // server = New York + 7h
  return t - (7 + nyOffsetH(guess)) * H;
}

/** Aggregates bars into fixed buckets on the data clock (anchor 0) and keeps how many bars fell in each. */
export function bucket(c: readonly Candle[], period: number) {
  const m = new Map<number, Candle & { n: number }>();
  for (const b of c) {
    const k = Math.floor(b.t / period) * period, x = m.get(k);
    if (!x) m.set(k, { t: k, o: b.o, h: b.h, l: b.l, c: b.c, v: b.v, n: 1 });
    else { x.h = Math.max(x.h, b.h); x.l = Math.min(x.l, b.l); x.c = b.c; x.v += b.v; x.n++; }
  }
  return m;
}

/** Compares a coarse file with the fine file aggregated to it, on buckets the fine file fills completely. */
export function crossCheck(fine: readonly Candle[], tfFine: number, coarse: readonly Candle[], tfCoarse: number, tol: number) {
  const agg = bucket(fine, tfCoarse), need = tfCoarse / tfFine;
  let compared = 0, mismatched = 0, maxDiff = 0;
  for (const b of coarse) {
    const a = agg.get(b.t);
    if (!a || a.n !== need) continue;
    compared++;
    const d = Math.max(Math.abs(a.o - b.o), Math.abs(a.h - b.h), Math.abs(a.l - b.l), Math.abs(a.c - b.c));
    maxDiff = Math.max(maxDiff, d);
    if (d > tol) mismatched++;
  }
  return { compared, mismatched, maxDiff };
}

/** Which day boundary a daily file uses: tries every whole-hour anchor on the data clock against the hourly series. */
export function dailyBoundary(daily: readonly Candle[], hourly: readonly Candle[], tol: number) {
  const res: { anchorHour: number; compared: number; matchedClose: number }[] = [];
  const byT = new Map(hourly.map((b) => [b.t, b]));
  for (let a = 0; a < 24; a++) {
    let compared = 0, matched = 0;
    for (const d of daily) {
      // the daily close should equal the close of the last hourly bar before the next boundary
      const end = Math.floor((d.t + D - a * H) / D) * D + a * H;
      let last: Candle | undefined;
      for (let k = 1; k <= 6 && !last; k++) last = byT.get(end - k * H);
      if (!last) continue;
      compared++;
      if (Math.abs(last.c - d.c) <= tol) matched++;
    }
    res.push({ anchorHour: a, compared, matchedClose: compared ? matched / compared : 0 });
  }
  return res.sort((x, y) => y.matchedClose - x.matchedClose)[0]!;
}

/** For crypto: the data clock's offset from Binance (hours), from hourly return correlation in up to three windows. */
export async function binanceOffset(symbol: string, h1: readonly Candle[]): Promise<{ pair: string; offsetHours: number; corr: number; windows: number } | null> {
  const base = symbol.replace(/[-_/]/g, '').replace(/(USDT|USD|USDC|PERP)$/i, '').toUpperCase(), pair = `${base}USDT`;
  const byT = new Map(h1.map((b) => [b.t, b]));
  const starts = [0.15, 0.5, 0.9].map((f) => Math.floor((h1[0]!.t + f * (h1.at(-1)!.t - h1[0]!.t)) / H) * H);
  const score = new Map<number, number[]>();
  let windows = 0;
  for (const st of starts) {
    let rows: [number, string, string, string, string][];
    try {
      const r = await fetch(`https://data-api.binance.vision/api/v3/klines?symbol=${pair}&interval=1h&startTime=${st}&limit=1000`);
      if (!r.ok) return null;
      rows = (await r.json()) as typeof rows;
    } catch { return null; }
    if (rows.length < 200) continue;
    windows++;
    for (let off = -12; off <= 12; off++) {
      const a: number[] = [], b: number[] = [];
      for (const k of rows) { const x = byT.get(Number(k[0]) + off * H); if (x) { a.push(Math.log(Number(k[4]) / Number(k[1]))); b.push(Math.log(x.c / x.o)); } }
      if (a.length < 150) continue;
      const ma = a.reduce((s, v) => s + v, 0) / a.length, mb = b.reduce((s, v) => s + v, 0) / b.length;
      let num = 0, da = 0, db = 0; for (let i = 0; i < a.length; i++) { num += (a[i]! - ma) * (b[i]! - mb); da += (a[i]! - ma) ** 2; db += (b[i]! - mb) ** 2; }
      (score.get(off) ?? score.set(off, []).get(off)!).push(num / Math.sqrt(da * db));
    }
  }
  if (!windows) return null;
  let best = { off: 0, corr: -1 };
  for (const [off, cs] of score) { const m = cs.reduce((s, v) => s + v, 0) / cs.length; if (m > best.corr) best = { off, corr: m }; }
  return { pair, offsetHours: best.off, corr: +best.corr.toFixed(3), windows };
}
