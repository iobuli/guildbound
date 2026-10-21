// Does kev read SOL the way the user's One Hour engine does? Same bars (exact match for 2021-04..05), same ATR (taken from their frames).
import { readFileSync, writeFileSync } from 'node:fs';
import { P } from '../core/params.js';
import { analyzeLevel } from '../core/levelmath.js';
import { makeFeed, SOL, MONDAY } from '../core/series.js';
import { SetupSchema, type Candle } from '../core/types.js';
import { graph } from '../kev.js';

const rd = (p: string) => JSON.parse(readFileSync(new URL(`../../${p}`, import.meta.url), 'utf8'));
const sol: Candle[] = rd('data/SOLUSDT_1h.json'), D = rd('data/one_hour_sol.json');
const idx = new Map(sol.map((b, i) => [b.t, i]));
const ts = (s: string) => Date.parse(s.replace(' ', 'T') + ':00Z');
const F: any[] = D.frames, tol = D.cfg.tol;
const mode = (process.argv[2] ?? 'spec');
Object.assign(P, mode === 'spec' ? { levelScaleD1: 1, zoneHalfAtr: 0.05, touchReactionAtr: 0.5, touchClusterBars: 12 } : {});

const breaks: any[] = [];
F.forEach((f) => (f.broke ?? []).forEach((u: number) => { const L = f.lv.find((x: any) => x.u === u); if (L) breaks.push({ f: f.f, ...L }); }));
const decisions = F.flatMap((f) => (f.retest ?? []).filter((r: any) => r.T1 !== undefined).map((r: any) => ({ ...r, f: f.f })));

const rows = breaks.map((L) => {
  const j = idx.get(ts(F[L.f].ts))!, dir = L.side as 'long' | 'short';
  const closed = sol.slice(Math.max(0, j - 799), j + 1); // bars through the break bar, all closed
  const a = analyzeLevel(closed, L.p, dir, F[L.f].atr);
  return { f: L.f, side: dir, p: L.p, theirN: L.n, theirB: L.b, kevFound: a.found, kevN: a.touches, kevBreakFrame: a.found ? F.findIndex((x) => ts(x.ts) === a.breakT) : null };
});
const agree = (k: (r: typeof rows[0]) => boolean) => rows.filter(k).length;
const out = {
  mode, n: rows.length,
  kevFoundBreak: agree((r) => r.kevFound), sameBreakBar: agree((r) => r.kevBreakFrame === r.theirB),
  sameTouches: agree((r) => r.kevFound && r.kevN === r.theirN), kevMore: agree((r) => r.kevFound && r.kevN > r.theirN), kevFewer: agree((r) => r.kevFound && r.kevN < r.theirN),
  theirTouchDist: Object.fromEntries([1, 2, 3, 4, 5].map((n) => [n, rows.filter((r) => r.theirN === n).length])),
  kevTouchDist: Object.fromEntries([0, 1, 2, 3, 4, 5].map((n) => [n, rows.filter((r) => r.kevFound && r.kevN === n).length])),
  theirN2plus: agree((r) => r.theirN >= 2), kevN2plus: agree((r) => r.kevFound && r.kevN >= 2),
  decisions: decisions.length,
};
// Does kev's scan (run before the decision bar, prior_bars only) contain the decision's level?
const feed = makeFeed(sol, { weekAnchor: MONDAY });
let inScan = 0, inScanTier2 = 0;
for (const d of decisions) {
  const j = idx.get(ts(F[d.f].ts))!;
  const r = graph.run(feed, sol[j]!.t, SetupSchema.parse({ dir: d.side }), { instrument: SOL });
  const rowsS = ((r.states['level-scan']?.out.levels as any)?.rows ?? []) as { price: number; touches: number }[];
  const hit = rowsS.find((x) => Math.abs(x.price - d.p) <= tol * F[d.f].atr);
  if (hit) { inScan++; if (hit.touches >= 2) inScanTier2++; }
}
(out as any).decisionLevelInKevScan = inScan; (out as any).decisionLevelInKevScanTouches2 = inScanTier2;
writeFileSync(new URL(`../../out/agree_${mode}.json`, import.meta.url), JSON.stringify({ summary: out, rows }, null, 1));
console.log(JSON.stringify(out, null, 1));
const dis = rows.filter((r) => !(r.kevFound && r.kevN === r.theirN)).slice(0, 8);
console.log('first disagreements', dis);
