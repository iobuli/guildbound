import { writeFileSync } from 'node:fs';
import { parseMt4 } from './mt4.js';
import { aggregate, D, H } from '../core/series.js';
import type { Candle } from '../core/types.js';

const DIR = '/root/.claude/uploads/6a8f537f-8d77-5682-b002-82305f0146b4/';
const FILES: Record<string, string> = { m30: 'bd123664-XAUUSD30.csv', h1: '299d826d-XAUUSD60_1.csv', h4: '84661a18-XAUUSD240.csv', d1: '7c855670-XAUUSD1440.csv' };
const data: Record<string, Candle[]> = {};
for (const [tf, f] of Object.entries(FILES)) {
  const { candles, problems } = parseMt4(DIR + f);
  data[tf] = candles;
  const gaps: Record<string, number> = {};
  const step = { m30: 1800e3, h1: H, h4: 4 * H, d1: D }[tf]!;
  for (let i = 1; i < candles.length; i++) { const g = (candles[i]!.t - candles[i - 1]!.t) / step; if (g > 1.01) gaps[g > 60 ? '>60' : g > 4 ? '4-60' : '1-4'] = (gaps[g > 60 ? '>60' : g > 4 ? '4-60' : '1-4'] ?? 0) + 1; }
  const first = new Date(candles[0]!.t).toISOString().slice(0, 16), last = new Date(candles.at(-1)!.t).toISOString().slice(0, 16);
  console.log(`${tf}: ${candles.length} bars ${first} -> ${last}  price ${candles.reduce((a, c) => Math.min(a, c.l), Infinity).toFixed(0)}..${candles.reduce((a, c) => Math.max(a, c.h), 0).toFixed(0)}  problems ${JSON.stringify(problems)}  gaps(in bars) ${JSON.stringify(gaps)}`);
  writeFileSync(new URL(`../../data/XAUUSD_${tf}.json`, import.meta.url), JSON.stringify(candles));
}

// cross-checks: do the timeframes describe the same market, on the same clock?
const cmp = (name: string, a: Candle[], b: Candle[], tol = 0.5) => {
  const m = new Map(b.map((x) => [x.t, x])); let n = 0, ok = 0, worst = 0;
  for (const x of a) { const y = m.get(x.t); if (!y) continue; n++; const d = Math.max(Math.abs(x.o - y.o), Math.abs(x.h - y.h), Math.abs(x.l - y.l), Math.abs(x.c - y.c)); worst = Math.max(worst, d); if (d <= tol) ok++; }
  console.log(`${name}: ${n} matching bars, ${(100 * ok / Math.max(1, n)).toFixed(1)}% within ${tol} USD on O/H/L/C, worst ${worst.toFixed(2)}`);
};
cmp('M30 aggregated to H1  vs H1 file', aggregate(data.m30!, H), data.h1!);
cmp('H1  aggregated to H4  vs H4 file (all)', aggregate(data.h1!, 4 * H), data.h4!);
cmp('H1  aggregated to D1  vs D1 file', aggregate(data.h1!, D), data.d1!);
cmp('H4  file          vs D1 (H4 starting 00:00 = D1 open)', data.h4!.filter((b) => b.t % D === 0), data.d1!.map((b) => ({ ...b })), 1e9);
const wk = (t: number) => new Date(t).getUTCDay();
const dow = [0, 0, 0, 0, 0, 0, 0]; for (const b of data.h1!) dow[wk(b.t)]!++;
console.log('H1 bars by weekday Sun..Sat:', dow.join(' '));
const hours = new Array(24).fill(0); for (const b of data.h1!) hours[new Date(b.t).getUTCHours()]++;
console.log('H1 bars by hour 0..23:', hours.join(' '));
