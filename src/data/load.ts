// Any price file to clean, sorted, de-duplicated candles, with a report of everything that had to be fixed.
import { readFileSync } from 'node:fs';
import type { Candle } from '../core/types.js';
import { parseTime, sniff, type Layout } from './sniff.js';

export interface Problems { badRows: number; duplicates: number; outOfOrder: number; repairedOhlc: number; descending: boolean; endStamped: number }
export interface Loaded { file: string; candles: Candle[]; problems: Problems; layout: Layout | { name: string }; decimals: number; tf: number; aligned: number }

const MIN = 60_000;

export function loadFile(path: string): Loaded {
  const text = readFileSync(path, 'utf8').replace(/^﻿/, '');
  const problems: Problems = { badRows: 0, duplicates: 0, outOfOrder: 0, repairedOhlc: 0, descending: false, endStamped: 0 };
  let raw: Candle[] = [], decimals = 0, layout: Loaded['layout'];
  const dec = (s: string) => { const k = s.indexOf('.'); return k < 0 ? 0 : s.length - k - 1; };
  if (/^\s*[[{]/.test(text)) {
    // JSON: an array of {t,o,h,l,c,v} objects or Binance kline arrays
    const j = JSON.parse(text) as unknown;
    if (!Array.isArray(j)) throw new Error('JSON files must be an array of candles ({t,o,h,l,c,v} objects or kline arrays)');
    layout = { name: Array.isArray(j[0]) ? 'json klines' : 'json candles' };
    for (const x of j as (Record<string, unknown> | unknown[])[]) {
      const g = (k: string, i: number) => Number(Array.isArray(x) ? x[i] : (x as Record<string, unknown>)[k]);
      const r = { t: g('t', 0), o: g('o', 1), h: g('h', 2), l: g('l', 3), c: g('c', 4), v: g('v', 5) || 0 };
      if ([r.t, r.o, r.h, r.l, r.c].every(Number.isFinite)) { raw.push(r); for (const k of ['o', 'h', 'l', 'c'] as const) decimals = Math.max(decimals, dec(String(r[k]))); } else problems.badRows++;
    }
  } else {
    const lines = text.split(/\r?\n/);
    const L = sniff(lines);
    layout = L;
    const body = L.header ? lines.slice(lines.findIndex((l) => l.trim()) + 1) : lines;
    const num = (s: string | undefined) => Number(L.decimalComma ? (s ?? '').replace(',', '.') : s);
    for (const line of body) {
      if (!line.trim()) continue;
      const p = line.split(L.delim);
      const t = parseTime(p[L.time] ?? '', L.time2 === null ? null : p[L.time2] ?? '', L.dayFirst);
      const o = num(p[L.o]), h = num(p[L.h]), l = num(p[L.l]), c = num(p[L.c]), v = L.v === null ? 0 : num(p[L.v]);
      if (!Number.isFinite(t) || ![o, h, l, c].every((x) => Number.isFinite(x) && x > 0)) { problems.badRows++; continue; }
      raw.push({ t, o, h, l, c, v: Number.isFinite(v) ? v : 0 });
      for (const k of [L.o, L.h, L.l, L.c]) decimals = Math.max(decimals, dec((p[k] ?? '').trim()));
    }
  }
  // newest-first files are reversed; anything else out of order is counted, then sorted
  let desc = 0; for (let i = 1; i < raw.length; i++) if (raw[i]!.t < raw[i - 1]!.t) desc++;
  if (desc > raw.length / 2) { raw.reverse(); problems.descending = true; desc = raw.length - 1 - desc; }
  problems.outOfOrder = desc;
  if (desc) raw = raw.map((b, i) => ({ b, i })).sort((a, b) => a.b.t - b.b.t || a.i - b.i).map((x) => x.b);
  const out: Candle[] = [];
  for (const b of raw) {
    const last = out[out.length - 1];
    if (last && b.t === last.t) { problems.duplicates++; continue; }
    if (b.h < Math.max(b.o, b.c) || b.l > Math.min(b.o, b.c) || b.h < b.l) { problems.repairedOhlc++; const hi = Math.max(b.h, b.l, b.o, b.c), lo = Math.min(b.h, b.l, b.o, b.c); b.h = hi; b.l = lo; }
    out.push(b);
  }
  // timeframe = the most common spacing; bars stamped one second (or 1 ms) before their end are moved to their start
  const tf = modalSpacing(out);
  const res = tf > 0 ? out.map((b) => ((b.t % tf) + tf) % tf) : [];
  const endRes = [tf - 1000, tf - 1].find((r) => res.filter((x) => x === r).length > res.length / 2);
  if (endRes !== undefined) { for (const b of out) b.t -= endRes; problems.endStamped = out.length; }
  const aligned = tf > 0 && tf < 86_400_000 ? out.filter((b) => b.t % Math.min(tf, 3_600_000) === 0).length / Math.max(1, out.length) : 1;
  return { file: path, candles: out, problems, layout, decimals, tf, aligned };
}

export function modalSpacing(c: readonly Candle[]): number {
  const m = new Map<number, number>();
  for (let i = 1; i < c.length; i++) { const d = c[i]!.t - c[i - 1]!.t; if (d > 0) m.set(d, (m.get(d) ?? 0) + 1); }
  let best = 0, n = -1;
  for (const [d, k] of m) if (k > n) { best = d; n = k; }
  return best;
}

export const tfName = (ms: number) => (ms % 86_400_000 === 0 ? `d${ms / 86_400_000}` : ms % 3_600_000 === 0 ? `h${ms / 3_600_000}` : ms % MIN === 0 ? `m${ms / MIN}` : `${ms}ms`);
