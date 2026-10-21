import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { graph } from './kev.js';
import { H, makeFeed, SOL } from './core/series.js';
import { encode, N_COARSE, N_FINE } from './core/token.js';
import { SetupSchema, type Candle } from './core/types.js';
import type { SfpSignalT } from './core/ports.js';

const sol: Candle[] = JSON.parse(readFileSync(new URL('../data/SOLUSDT_1h.json', import.meta.url), 'utf8'));
const feed = makeFeed(sol);
const STEP = Number(process.argv[2] ?? 4), FROM = Date.UTC(2022, 0, 1);
const idxOf = new Map(sol.map((b, i) => [b.t, i]));

const tokC = new Map<number, number>(), tokF = new Map<number, number>();
const events = new Map<string, { sig: SfpSignalT; seenAt: number }>();
const biasDays = { long: 0, short: 0, neutral: 0 }, dayEvents: { t: number; dir: 'long' | 'short' }[] = [];
let runs = 0, t0 = Date.now(), lastDay = -1;
const setup = SetupSchema.parse({ dir: 'long' });
for (let i = sol.findIndex((b) => b.t >= FROM); i < sol.length - 80; i += STEP) {
  const T = sol[i]!.t + 37 * 60000, price = sol[i]!.o;
  const r = graph.run(feed, T, setup, { instrument: SOL, price });
  runs++;
  const tok = encode(r);
  if (tok) { tokC.set(tok.coarse, (tokC.get(tok.coarse) ?? 0) + 1); tokF.set(tok.fine, (tokF.get(tok.fine) ?? 0) + 1); }
  const sfp = r.states.sfp?.out.sfp as { h1: SfpSignalT[]; d1: SfpSignalT[] } | undefined;
  for (const s of [...(sfp?.h1 ?? []), ...(sfp?.d1 ?? [])]) { const k = `${s.tf}|${s.sweepT}|${s.dir}`; if (!events.has(k)) events.set(k, { sig: s, seenAt: i }); }
  const day = Math.floor(T / 86_400_000), bias = r.states['daily-bias']?.out.bias as { d1: { dir: 'long' | 'short' | 'neutral'; fresh: boolean; ageBars: number | null } } | undefined;
  if (bias && day !== lastDay) {
    lastDay = day; biasDays[bias.d1.dir]++;
    if (bias.d1.dir !== 'neutral' && bias.d1.ageBars === 0) dayEvents.push({ t: T, dir: bias.d1.dir });
  }
}
const secs = (Date.now() - t0) / 1000;

// ---- forward returns, aligned with the signal direction, vs the same-direction baseline over the same period ----
const fwd = (i: number, h: number, dir: 'long' | 'short') => (i + h < sol.length ? (dir === 'long' ? 1 : -1) * (sol[i + h]!.c / sol[i]!.c - 1) : null);
const start = sol.findIndex((b) => b.t >= FROM);
const base = (h: number, dir: 'long' | 'short') => { const xs: number[] = []; for (let i = start; i < sol.length - h; i += 3) xs.push(fwd(i, h, dir)!); return xs; };
const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / (xs.length || 1);
const sd = (xs: number[]) => { const m = mean(xs); return Math.sqrt(xs.reduce((a, b) => a + (b - m) ** 2, 0) / Math.max(1, xs.length - 1)); };
function summarize(name: string, evs: { i: number; dir: 'long' | 'short' }[], h: number) {
  const xs = evs.map((e) => fwd(e.i, h, e.dir)).filter((x): x is number => x !== null);
  const bl = evs.filter((e) => e.dir === 'long').length / Math.max(1, evs.length);
  const bs = [...base(h, 'long'), ...base(h, 'short')];
  const m0 = mean(base(h, 'long')) * bl + mean(base(h, 'short')) * (1 - bl); // baseline matched to the long/short mix
  const se = sd(xs) / Math.sqrt(Math.max(1, xs.length));
  return { group: name, horizonH: h, n: xs.length, meanPct: +(100 * mean(xs)).toFixed(3), baselinePct: +(100 * m0).toFixed(3), diffPct: +(100 * (mean(xs) - m0)).toFixed(3), tStat: +(((mean(xs) - m0) / (se || 1))).toFixed(2), hit: +(xs.filter((x) => x > 0).length / Math.max(1, xs.length)).toFixed(3), sdPct: +(100 * sd(bs)).toFixed(2) };
}
const sweepIdx = (s: SfpSignalT) => idxOf.get(s.sweepT)!;
const h1 = [...events.values()].filter((e) => e.sig.tf === 'h1').map((e) => e.sig);
const groups: [string, { i: number; dir: 'long' | 'short' }[]][] = [
  ['H1 SFP, all rules pass (valid)', h1.filter((s) => s.valid).map((s) => ({ i: sweepIdx(s), dir: s.dir }))],
  ['H1 SFP, swing NOT deep', h1.filter((s) => !s.deep && !s.tooSmall && s.visitNo <= 3).map((s) => ({ i: sweepIdx(s), dir: s.dir }))],
  ['H1 SFP, 4th+ visit', h1.filter((s) => s.visitNo > 3 && s.deep).map((s) => ({ i: sweepIdx(s), dir: s.dir }))],
  ['D1 bias event (fresh signal, bar 0)', dayEvents.map((e) => ({ i: idxOf.get(Math.floor(e.t / H) * H) ?? 0, dir: e.dir }))],
];
const results = groups.flatMap(([name, evs]) => [24, 72].map((h) => summarize(name, evs, h)));

mkdirSync(new URL('../out/', import.meta.url), { recursive: true });
const report = {
  range: { from: new Date(sol[start]!.t).toISOString().slice(0, 10), to: new Date(sol.at(-1)!.t).toISOString().slice(0, 10), step: STEP, runs, seconds: +secs.toFixed(1), msPerRun: +(1000 * secs / runs).toFixed(1) },
  tokens: { coarseUsed: `${tokC.size}/${N_COARSE}`, fineUsed: `${tokF.size}/${N_FINE}`, topCoarse: [...tokC].sort((a, b) => b[1] - a[1]).slice(0, 5) },
  dailyBias: biasDays,
  sfpEvents: { h1Total: h1.length, h1Valid: h1.filter((s) => s.valid).length, h1NotDeep: h1.filter((s) => !s.deep).length, h1Visit4Plus: h1.filter((s) => s.visitNo > 3).length, d1Total: [...events.values()].filter((e) => e.sig.tf === 'd1').length },
  forwardReturns: results,
};
writeFileSync(new URL('../out/replay.json', import.meta.url), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 1));
