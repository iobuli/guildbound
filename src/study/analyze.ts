// Turning one harness run into numbers: every stratum is compared with its OWN placebos (same side, distance, window, risk),
// with day-cluster bootstrap CIs and a day-cluster permutation p, and Benjamini-Hochberg q across the strata of one question.
import type { Trade } from '../backtest/engine.js';
import { benjaminiHochberg, clusterDiffBoot, clusterMean } from '../backtest/stats.js';

export interface Cell { name: string; n: number; mean: number; lo: number; hi: number; plN: number; plMean: number; diff: number; dLo: number; dHi: number; p: number; q?: number }
const rows = (ts: Trade[], key: string) => ts.filter((t) => Number.isFinite(t.r[key])).map((t) => ({ r: t.r[key]!, day: t.day }));

/** `fn` selects real trades; their placebos are selected by the same fields (placebos copy the real trade's touches, space, side, ...). */
export function cell(name: string, real: Trade[], plc: Trade[], key: string, fn: (t: Trade) => boolean, reps = 400): Cell {
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
export const compact = (t: Trade) => ({ t: t.t, i: t.i, d: t.dir === 'long' ? 1 : -1, y: t.year, day: t.day, n: t.touches, far: +t.farAtr.toFixed(3), sp: t.spaceOk ? 1 : 0, go: t.go ? 1 : 0, ok: t.ok ? 1 : 0, ct: t.closedThrough ? 1 : 0, lt: t.resolvedBy === 'ltf' ? 1 : 0, bsb: t.barsSinceBreak, r: rnd(t.r), ...(t.rc ? { rc: rnd(t.rc) } : {}) });
const rnd = (r: Record<string, number>) => Object.fromEntries(Object.entries(r).filter(([, v]) => Number.isFinite(v)).map(([k, v]) => [k, +v.toFixed(4)]));
