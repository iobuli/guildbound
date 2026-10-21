// What does SOL do when price comes back to a level it just broke? Everything is decided from bars strictly before the fill bar.
// Level = cluster of same-kind 3-bar swing points within 0.1 x prior-day ATR(20), >= 12 bars apart (the user's One Hour definition, recovered from its data).
// Event = close beyond the level by 0.1 ATR, then first return to the level within 48 bars. Order = limit at the level, resting before the fill bar.
import { readFileSync, writeFileSync } from 'node:fs';
import { P } from '../core/params.js';
import { analyzeLevel } from '../core/levelmath.js';
import type { Candle } from '../core/types.js';
import { clusterMean, clusterDiff, benjaminiHochberg, f } from '../backtest/stats.js';

const c: Candle[] = JSON.parse(readFileSync(new URL('../../data/SOLUSDT_1h.json', import.meta.url), 'utf8'));
const N = c.length, DAY = 86_400_000;
const TOL = 0.1, BRK = 0.1, GAP = 12, SEARCH = 48, MAXHOLD = 96, SLIP = 0.1, SPACE = 0.5, MAXAGE = 300;
Object.assign(P, { levelScaleD1: 1, zoneHalfAtr: 0.05, touchReactionAtr: 0.5, touchClusterBars: 12 }); // kev's reaction-count, scaled like the user's page

// ---- prior-day Wilder ATR(20) per bar (day before, as the method says) ----
const dayKey = (t: number) => Math.floor(t / DAY);
const days: { k: number; o: number; h: number; l: number; c: number }[] = [];
for (const b of c) { const k = dayKey(b.t), d = days[days.length - 1]; if (d && d.k === k) { d.h = Math.max(d.h, b.h); d.l = Math.min(d.l, b.l); d.c = b.c; } else days.push({ k, o: b.o, h: b.h, l: b.l, c: b.c }); }
const atrAfter: number[] = []; // ATR using days 0..d inclusive
{ const tr = days.map((d, i) => Math.max(d.h - d.l, i ? Math.abs(d.h - days[i - 1]!.c) : 0, i ? Math.abs(d.l - days[i - 1]!.c) : 0)); let a = 0; tr.forEach((x, i) => { if (i < 19) { a += x; atrAfter.push(NaN); } else if (i === 19) { a = (a + x) / 20; atrAfter.push(a); } else { a = (a * 19 + x) / 20; atrAfter.push(a); } }); }
const dayIdx = new Map(days.map((d, i) => [d.k, i]));
const atrAt = new Array<number>(N), sma50 = new Array<number>(N);
for (let j = 0; j < N; j++) { const di = dayIdx.get(dayKey(c[j]!.t))!; atrAt[j] = di > 0 ? atrAfter[di - 1]! : NaN; const s = Math.max(0, di - 50); sma50[j] = di > 0 ? days.slice(s, di).reduce((x, d) => x + d.c, 0) / (di - s) : NaN; }
// prior-24h volume relative to the prior-20-day average daily volume (known at the break bar)
const volRatio = new Array<number>(N).fill(NaN);
{ const cs = [0]; for (const b of c) cs.push(cs[cs.length - 1]! + b.v); for (let j = 480; j < N; j++) volRatio[j] = ((cs[j]! - cs[j - 24]!) / 24) / ((cs[j - 24]! - cs[j - 504 < 0 ? 0 : j - 504]!) / Math.max(1, Math.min(480, j - 24))); }
const volTerc = (() => { const xs = volRatio.filter(Number.isFinite).sort((a, b) => a - b); return [xs[Math.floor(xs.length / 3)]!, xs[Math.floor((2 * xs.length) / 3)]!]; })();
const volPct = (() => { const xs = [...new Set(atrAt.map((a, j) => (Number.isFinite(a) ? a / c[j]!.c : NaN)).filter(Number.isFinite))].sort((a, b) => a - b); return [xs[Math.floor(xs.length / 3)]!, xs[Math.floor((2 * xs.length) / 3)]!]; })();

// ---- level construction, bar by bar ----
interface Lvl { kind: 'high' | 'low'; piv: { i: number; v: number }[]; p: number; broken: boolean }
const med = (xs: number[]) => { const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]!; };
const lv: Lvl[] = [];
interface Ev { b: number; j: number; dir: 'long' | 'short'; p: number; atr: number; nF: number; nK: number; farPrior: number; farIncl: number; age: number; closedThrough: boolean; ctx: { vr: number; volT: number; trend: 'with' | 'against'; year: number; day: number; hod: number; dow: number } }
const events: Ev[] = [];

function settle(j: number, long: boolean, p: number, S: number): Record<string, number> {
  // fill bar j: filled at p. Stop first if the fill bar's own adverse extreme reaches it (we cannot see inside the bar); targets are only tested from j+1.
  const stop = long ? p - S : p + S, out: Record<string, number> = {};
  for (const k of [1, 2, 4]) {
    let r = NaN;
    if (long ? c[j]!.l <= stop : c[j]!.h >= stop) r = -1;
    else {
      const tgt = long ? p + k * S : p - k * S;
      for (let t = j + 1; t < Math.min(N, j + 1 + MAXHOLD); t++) {
        const b = c[t]!;
        if (long ? b.l <= stop : b.h >= stop) { r = -1; break; }
        if (long ? b.h >= tgt : b.l <= tgt) { r = k; break; }
        if (t === Math.min(N, j + MAXHOLD) - 0 || t === N - 1) { r = (long ? b.c - p : p - b.c) / S; }
      }
      if (Number.isNaN(r)) { const b = c[Math.min(N - 1, j + MAXHOLD)]!; r = (long ? b.c - p : p - b.c) / S; }
    }
    out[`S${S}_T${k}`] = r - SLIP; // net of 0.10R slippage, like the user's page
  }
  return out;
}
const sKeys = (atr: number) => [0.5, 1.0].map((m) => ({ m, S: m * atr }));

// pending breaks waiting for their first return
interface Pend { b: number; p: number; dir: 'long' | 'short'; nF: number; atr: number; lvl: Lvl; done: boolean }
const pend: Pend[] = [];
const rows: { ev: Ev; r: Record<string, number> }[] = [];
const evNK = (b: number, p: number, dir: 'long' | 'short', atr: number) => analyzeLevel(c.slice(Math.max(0, b - 799), b + 1), p, dir, atr).touches;

for (let j = 2; j < N; j++) {
  const atr = atrAt[j]!; if (!Number.isFinite(atr)) continue;
  // 1. pivot at j-2 is confirmed by bar j-1 (3-bar fractal); use it from decisions at bar j onward, i.e. everything strictly before j
  const i = j - 2;
  if (i >= 1) for (const kind of ['high', 'low'] as const) {
    const piv = kind === 'high' ? c[i]!.h > c[i - 1]!.h && c[i]!.h > c[i + 1]!.h : c[i]!.l < c[i - 1]!.l && c[i]!.l < c[i + 1]!.l, v = kind === 'high' ? c[i]!.h : c[i]!.l;
    if (!piv) continue;
    const a = atrAt[i + 1]!; if (!Number.isFinite(a)) continue;
    const L = lv.find((x) => x.kind === kind && !x.broken && Math.abs(x.p - v) <= TOL * a && j - x.piv[x.piv.length - 1]!.i <= MAXAGE);
    if (L) { if (i - L.piv[L.piv.length - 1]!.i >= GAP) L.piv.push({ i, v }); else L.piv[L.piv.length - 1] = L.piv[L.piv.length - 1]!; L.p = med(L.piv.map((q) => q.v)); }
    else lv.push({ kind, piv: [{ i, v }], p: v, broken: false });
  }
  // 2. fills for pending breaks (bar j touches the level): decided with bars < j only
  for (const q of pend) {
    if (q.done || j <= q.b) continue;
    if (j - q.b > SEARCH) { q.done = true; continue; }
    const long = q.dir === 'long', b = c[j]!;
    if (long ? b.l <= q.p : b.h >= q.p) {
      q.done = true;
      let farP = 0; for (let t = q.b + 1; t < j; t++) farP = Math.max(farP, (long ? c[t]!.h - q.p : q.p - c[t]!.l) / q.atr);
      const farI = Math.max(farP, (long ? b.h - q.p : q.p - b.l) / q.atr);
      const ev: Ev = { b: q.b, j, dir: q.dir, p: q.p, atr: q.atr, nF: q.nF, nK: evNK(q.b, q.p, q.dir, q.atr), farPrior: farP, farIncl: farI, age: j - q.b, closedThrough: long ? b.c < q.p : b.c > q.p,
        ctx: { vr: volRatio[q.b]!, volT: c[q.b]!.c * 0 + (q.atr / c[q.b]!.c < volPct[0]! ? 0 : q.atr / c[q.b]!.c < volPct[1]! ? 1 : 2), trend: (long ? c[q.b]!.c > sma50[q.b]! : c[q.b]!.c < sma50[q.b]!) ? 'with' : 'against', year: new Date(c[j]!.t).getUTCFullYear(), day: dayKey(c[j]!.t), hod: new Date(c[j]!.t).getUTCHours(), dow: new Date(c[j]!.t).getUTCDay() } };
      const r: Record<string, number> = {};
      for (const { S } of sKeys(q.atr)) Object.assign(r, settle(j, long, q.p, S));
      // keys are per absolute S; remap to multiples of ATR for readability
      const rr: Record<string, number> = {};
      for (const { m, S } of sKeys(q.atr)) for (const k of [1, 2, 4]) rr[`S${m}_T${k}`] = r[`S${S}_T${k}`]!;
      rows.push({ ev, r: rr });
    }
  }
  // 3. breaks at bar j: close beyond a never-broken level by BRK x ATR
  for (const L of lv) {
    if (L.broken) continue;
    if (j - L.piv[L.piv.length - 1]!.i > MAXAGE) { L.broken = true; continue; }
    const brokenUp = L.kind === 'high' && c[j]!.c > L.p + BRK * atr, brokenDn = L.kind === 'low' && c[j]!.c < L.p - BRK * atr;
    if (brokenUp || brokenDn) { L.broken = true; pend.push({ b: j, p: L.p, dir: brokenUp ? 'long' : 'short', nF: L.piv.length, atr, lvl: L, done: false }); }
  }
  if (j % 4000 === 0) { for (let k = lv.length - 1; k >= 0; k--) if (lv[k]!.broken || j - lv[k]!.piv[lv[k]!.piv.length - 1]!.i > MAXAGE) lv.splice(k, 1); for (let k = pend.length - 1; k >= 0; k--) if (pend[k]!.done) pend.splice(k, 1); }
}

// ---- placebo: same construction relative to a RANDOM bar. level = close_b' -/+ the real level's distance from its break close, same first-touch rule, same stops ----
function rng(seed: number) { let s = seed >>> 0; return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296); }
const rand = rng(7), plc: { ev: Ev; r: Record<string, number> }[] = [];
for (const { ev } of rows) for (let rep = 0; rep < 3; rep++) {
  const b2 = 60 + Math.floor(rand() * (N - 60 - SEARCH - MAXHOLD - 5)), a2 = atrAt[b2]!; if (!Number.isFinite(a2)) continue;
  const long = ev.dir === 'long', off = (c[ev.b]!.c - ev.p) * (long ? 1 : -1) / ev.atr; // break close distance beyond the level, in ATR
  const p2 = c[b2]!.c - (long ? 1 : -1) * off * a2;
  let fill = -1; for (let t = b2 + 1; t <= b2 + SEARCH; t++) if (long ? c[t]!.l <= p2 : c[t]!.h >= p2) { fill = t; break; }
  if (fill < 0) continue;
  let farP = 0; for (let t = b2 + 1; t < fill; t++) farP = Math.max(farP, (long ? c[t]!.h - p2 : p2 - c[t]!.l) / a2);
  const farI = Math.max(farP, (long ? c[fill]!.h - p2 : p2 - c[fill]!.l) / a2);
  const e2: Ev = { ...ev, b: b2, j: fill, p: p2, atr: a2, farPrior: farP, farIncl: farI, age: fill - b2, closedThrough: long ? c[fill]!.c < p2 : c[fill]!.c > p2, ctx: { ...ev.ctx, year: new Date(c[fill]!.t).getUTCFullYear(), day: dayKey(c[fill]!.t) } };
  const r: Record<string, number> = {};
  for (const m of [0.5, 1.0]) Object.assign(r, Object.fromEntries(Object.entries(settle(fill, long, p2, m * a2)).map(([k, v]) => [k.replace(/S[\d.]+/, `S${m}`), v])));
  plc.push({ ev: e2, r });
}

// ---- strata, pre-registered ----
const PRIMARY = 'S0.5_T2';
type Row = (typeof rows)[number];
const strata: [string, (x: Row) => boolean][] = [
  ['all fills', () => true],
  ['nF=1 (fractal count)', (x) => x.ev.nF === 1], ['nF=2', (x) => x.ev.nF === 2], ['nF>=3', (x) => x.ev.nF >= 3],
  ['nK=0 (kev reaction count)', (x) => x.ev.nK === 0], ['nK=1', (x) => x.ev.nK === 1], ['nK>=2', (x) => x.ev.nK >= 2],
  ['age 1-2 bars after break', (x) => x.ev.age <= 2], ['age 3-12', (x) => x.ev.age > 2 && x.ev.age <= 12], ['age 13-48', (x) => x.ev.age > 12],
  ['space ok on PRIOR bars (far>=0.5)', (x) => x.ev.farPrior >= SPACE], ['space not ok on prior bars', (x) => x.ev.farPrior < SPACE],
  ['EXPLORATORY far(prior) 0-0.25', (x) => x.ev.farPrior < 0.25], ['EXPLORATORY far(prior) 0.25-0.5', (x) => x.ev.farPrior >= 0.25 && x.ev.farPrior < 0.5], ['EXPLORATORY far(prior) 0.5-1', (x) => x.ev.farPrior >= 0.5 && x.ev.farPrior < 1], ['EXPLORATORY far(prior) 1-2', (x) => x.ev.farPrior >= 1 && x.ev.farPrior < 2], ['EXPLORATORY far(prior) 2+', (x) => x.ev.farPrior >= 2],
  ['EXPLORATORY far>=0.5 and IS<=2023', (x) => x.ev.farPrior >= SPACE && x.ev.ctx.year <= 2023], ['EXPLORATORY far>=0.5 and OOS 2024+', (x) => x.ev.farPrior >= SPACE && x.ev.ctx.year >= 2024],
  ['EXPLORATORY far>=0.5 and not closed through', (x) => x.ev.farPrior >= SPACE && !x.ev.closedThrough],
  ['space ok only if fill bar counted (leak group)', (x) => x.ev.farPrior < SPACE && x.ev.farIncl >= SPACE],
  ['fill bar closed through level (unknowable at fill)', (x) => x.ev.closedThrough], ['fill bar did not close through', (x) => !x.ev.closedThrough],
  ['long', (x) => x.ev.dir === 'long'], ['short', (x) => x.ev.dir === 'short'],
  ['vol tercile low', (x) => x.ev.ctx.volT === 0], ['vol tercile mid', (x) => x.ev.ctx.volT === 1], ['vol tercile high', (x) => x.ev.ctx.volT === 2],
  ['with D50 trend', (x) => x.ev.ctx.trend === 'with'], ['against D50 trend', (x) => x.ev.ctx.trend === 'against'],
  ['weekend (Sat/Sun UTC)', (x) => x.ev.ctx.dow === 0 || x.ev.ctx.dow === 6], ['weekday', (x) => x.ev.ctx.dow > 0 && x.ev.ctx.dow < 6],
  ['fill 00-08 UTC', (x) => x.ev.ctx.hod < 8], ['fill 08-16 UTC', (x) => x.ev.ctx.hod >= 8 && x.ev.ctx.hod < 16], ['fill 16-24 UTC', (x) => x.ev.ctx.hod >= 16],
  ['volume low (prior 24h vs 20d)', (x) => x.ev.ctx.vr < volTerc[0]!], ['volume mid', (x) => x.ev.ctx.vr >= volTerc[0]! && x.ev.ctx.vr < volTerc[1]!], ['volume high', (x) => x.ev.ctx.vr >= volTerc[1]!],
  ...[2021, 2022, 2023, 2024, 2025, 2026].map((y): [string, (x: Row) => boolean] => [`year ${y}`, (x) => x.ev.ctx.year === y]),
  ['IS year<=2023', (x) => x.ev.ctx.year <= 2023], ['OOS 2024+', (x) => x.ev.ctx.year >= 2024],
];
const rowsOf = (xs: Row[], key: string) => xs.map((x) => ({ r: x.r[key]!, day: x.ev.ctx.day })).filter((x) => Number.isFinite(x.r));
const table = strata.map(([name, fn]) => {
  const a = rows.filter(fn), b = plc.filter(fn);
  const m = clusterMean(rowsOf(a, PRIMARY), 300), pm = clusterMean(rowsOf(b, PRIMARY), 300), d = clusterDiff(rowsOf(a, PRIMARY), rowsOf(b, PRIMARY), 400);
  const alt = ['S1_T2', 'S1_T4', 'S0.5_T4'].map((k) => mean2(rowsOf(a, k)));
  return { name, n: m.n, mean: m.mean, lo: m.lo, hi: m.hi, plMean: pm.mean, plN: pm.n, diff: d.diff, p: d.p, alt };
});
function mean2(xs: { r: number }[]) { return xs.length ? xs.reduce((a, b) => a + b.r, 0) / xs.length : NaN; }
const q = benjaminiHochberg(table.map((t) => (Number.isFinite(t.p) ? t.p : 1)));
table.forEach((t, k) => ((t as any).q = q[k]));

const header = `# SOL levels study (H1, ${new Date(c[0]!.t).toISOString().slice(0, 10)} to ${new Date(c.at(-1)!.t).toISOString().slice(0, 10)})\n\n` +
  `Events: ${rows.length} first returns to a freshly broken level (placebo: ${plc.length}). Primary outcome ${PRIMARY}: stop 0.5 x prior-day ATR beyond the level, target 2R, net of 0.10R slippage, fill-bar stop counted, targets only from the next bar. ` +
  `95% CI by day-cluster bootstrap; p and q (BH over ${table.length} strata) compare each stratum with the same stratum of random-bar placebos.\n\n` +
  `| stratum | n | mean R [95% CI] | placebo | diff | p | q | S1_T2 | S1_T4 | S0.5_T4 |\n|---|---|---|---|---|---|---|---|---|---|\n` +
  table.map((t: any) => `| ${t.name} | ${t.n} | ${f(t.mean, 2)} [${f(t.lo, 2)}, ${f(t.hi, 2)}] | ${f(t.plMean, 2)} | ${f(t.diff, 2)} | ${f(t.p, 3)} | ${f(t.q, 3)} | ${t.alt.map((x: number) => f(x, 2)).join(' | ')} |`).join('\n');
writeFileSync(new URL('../../out/sol_levels.md', import.meta.url), header + '\n');
writeFileSync(new URL('../../out/sol_levels.json', import.meta.url), JSON.stringify({ table, events: rows.length, placebo: plc.length }, null, 1));
console.log(header);
