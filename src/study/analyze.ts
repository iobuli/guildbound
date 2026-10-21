// Turning one harness run into numbers: every stratum is compared with its OWN placebos (same side, distance, window, risk),
// with day-cluster bootstrap CIs and a day-cluster permutation p, and Benjamini-Hochberg q across the strata of one question.
import type { Trade } from '../backtest/engine.js';
import { benjaminiHochberg, clusterDiffBoot, clusterMean } from '../backtest/stats.js';

export interface Cell { name: string; n: number; mean: number; lo: number; hi: number; plN: number; plMean: number; diff: number; dLo: number; dHi: number; p: number; q?: number }
export const rows = (ts: { r: Record<string, number>; day: number }[], key: string) => ts.filter((t) => Number.isFinite(t.r[key])).map((t) => ({ r: t.r[key]!, day: t.day }));

export interface Rec { r: Record<string, number>; rc?: Record<string, number>; day: number; year: number; touches: number; spaceOk: boolean; go: boolean; closedThrough: boolean; ok: boolean; dir: 'long' | 'short'; far: number; atr: number; ltf: boolean }
/** Compact record (as written to out/bt/*.json) back to the fields the analysis needs. */
export const fromCompact = (x: ReturnType<typeof compact>): Rec => ({ r: x.r, rc: x.rc, day: x.day, year: x.y, touches: x.n, spaceOk: !!x.sp, go: !!x.go, closedThrough: !!x.ct, ok: !!x.ok, dir: x.d === 1 ? 'long' : 'short', far: x.far, atr: x.a, ltf: !!x.lt });

/** `fn` selects real trades; their placebos are selected by the same fields (placebos copy the real trade's touches, space, side, ...). */
export function cell<T extends { r: Record<string, number>; day: number }>(name: string, real: T[], plc: T[], key: string, fn: (t: T) => boolean, reps = 400): Cell {
  const a = rows(real.filter(fn), key), b = rows(plc.filter(fn), key);
  const m = clusterMean(a, reps), pm = clusterMean(b, 50), d = clusterDiffBoot(a, b, reps);
  return { name, n: m.n, mean: m.mean, lo: m.lo, hi: m.hi, plN: pm.n, plMean: pm.mean, diff: d.diff, dLo: d.lo, dHi: d.hi, p: d.p };
}
export function withQ(cells: Cell[]): Cell[] {
  const q = benjaminiHochberg(cells.map((c) => (Number.isFinite(c.p) ? c.p : 1)));
  return cells.map((c, k) => ({ ...c, q: q[k] }));
}
export const fmt = (x: number, d = 2) => (Number.isFinite(x) ? x.toFixed(d) : 'n/a');
export function table(cells: Cell[]): string {
  return ['| group | n | mean R [95% CI] | placebo | real - placebo [95% CI] | p | q |', '|---|---|---|---|---|---|---|',
    ...cells.map((c) => `| ${c.name} | ${c.n} | ${fmt(c.mean)} [${fmt(c.lo)}, ${fmt(c.hi)}] | ${fmt(c.plMean)} | ${fmt(c.diff)} [${fmt(c.dLo)}, ${fmt(c.dHi)}] | ${fmt(c.p, 3)} | ${c.q === undefined ? '' : fmt(c.q, 3)} |`)].join('\n');
}

/** Compact trade records for out/*.json (everything later questions need, nothing else). */
export const compact = (t: Trade) => ({ t: t.t, i: t.i, d: t.dir === 'long' ? 1 : -1, y: t.year, day: t.day, n: t.touches, far: +t.farAtr.toFixed(3), sp: t.spaceOk ? 1 : 0, go: t.go ? 1 : 0, ok: t.ok ? 1 : 0, ct: t.closedThrough ? 1 : 0, lt: t.resolvedBy === 'ltf' ? 1 : 0, bsb: t.barsSinceBreak, a: +t.atrD1.toPrecision(5), r: rnd(t.r), ...(t.rc ? { rc: rnd(t.rc) } : {}) });
const rnd = (r: Record<string, number>) => Object.fromEntries(Object.entries(r).filter(([, v]) => Number.isFinite(v)).map(([k, v]) => [k, +v.toFixed(4)]));

/** (realA - placeboA) - (realB - placeboB) with a day-cluster bootstrap: does stratum A beat its placebos by more than stratum B does? */
export function diffOfDiffs(aR: { r: number; day: number }[], aP: { r: number; day: number }[], bR: { r: number; day: number }[], bP: { r: number; day: number }[], reps = 800, seed = 9) {
  const groups = [aR, aP, bR, bP], days = new Map<number, number[][]>();
  groups.forEach((g, gi) => { for (const x of g) { const d = days.get(x.day) ?? [[0, 0], [0, 0], [0, 0], [0, 0]]; d[gi]![0] += x.r; d[gi]![1]++; days.set(x.day, d); } });
  const stat = (S: number[][]) => (S[0]![0] / S[0]![1] - S[1]![0] / S[1]![1]) - (S[2]![0] / S[2]![1] - S[3]![0] / S[3]![1]);
  const tot = [[0, 0], [0, 0], [0, 0], [0, 0]]; for (const d of days.values()) d.forEach((g, gi) => { tot[gi]![0] += g[0]!; tot[gi]![1] += g[1]!; });
  if (tot.some((g) => g[1]! < 5)) return { d: NaN, lo: NaN, hi: NaN, p: NaN };
  const list = [...days.values()]; let s = seed >>> 0; const rand = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  const bs: number[] = [];
  for (let k = 0; k < reps; k++) { const S = [[0, 0], [0, 0], [0, 0], [0, 0]]; for (let i = 0; i < list.length; i++) { const d = list[Math.floor(rand() * list.length)]!; d.forEach((g, gi) => { S[gi]![0] += g[0]!; S[gi]![1] += g[1]!; }); } if (S.every((g) => g[1]! > 0)) bs.push(stat(S)); }
  bs.sort((x, y) => x - y);
  const z = bs.filter((x) => x === 0).length / bs.length, le = bs.filter((x) => x < 0).length / bs.length + z / 2;
  return { d: stat(tot), lo: bs[Math.floor(0.025 * bs.length)]!, hi: bs[Math.floor(0.975 * bs.length)]!, p: Math.min(1, 2 * Math.min(le, 1 - le)) };
}
