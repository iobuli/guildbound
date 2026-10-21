export const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);
export const sd = (xs: number[]) => { const m = mean(xs); return Math.sqrt(xs.reduce((a, b) => a + (b - m) ** 2, 0) / Math.max(1, xs.length - 1)); };

function rng(seed: number) { let s = seed >>> 0; return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296); }

/** Mean with a bootstrap CI that resamples whole DAYS, because fills inside a day share the same market and are not independent. */
export function clusterMean(rows: { r: number; day: number }[], reps = 600, seed = 11) {
  const n = rows.length;
  if (!n) return { n: 0, mean: NaN, lo: NaN, hi: NaN };
  const byDay = new Map<number, number[]>();
  for (const x of rows) (byDay.get(x.day) ?? byDay.set(x.day, []).get(x.day)!).push(x.r);
  const days = [...byDay.values()], rand = rng(seed), means: number[] = [];
  for (let b = 0; b < reps; b++) {
    let s = 0, k = 0;
    for (let d = 0; d < days.length; d++) { const g = days[Math.floor(rand() * days.length)]!; for (const v of g) { s += v; k++; } }
    means.push(s / k);
  }
  means.sort((a, b) => a - b);
  return { n, mean: mean(rows.map((x) => x.r)), lo: means[Math.floor(0.025 * reps)]!, hi: means[Math.floor(0.975 * reps)]! };
}

/** Two-sample difference of means, p from a day-cluster permutation of the group labels (cheap and honest about overlap). */
export function clusterDiff(a: { r: number; day: number }[], b: { r: number; day: number }[], reps = 800, seed = 5) {
  if (a.length < 2 || b.length < 2) return { diff: NaN, p: NaN };
  const diff = mean(a.map((x) => x.r)) - mean(b.map((x) => x.r));
  const days = [...new Set([...a, ...b].map((x) => x.day))];
  const rand = rng(seed);
  const inA = new Set(a.map((x) => x.day)), inB = new Set(b.map((x) => x.day));
  let ge = 0;
  const all = [...a.map((x) => ({ ...x, g: 1 })), ...b.map((x) => ({ ...x, g: 0 }))];
  for (let k = 0; k < reps; k++) {
    // shuffle which days carry label A (days that contain both groups keep their within-day split)
    const flip = new Map<number, number>();
    for (const d of days) flip.set(d, rand() < 0.5 ? 1 : 0);
    const A: number[] = [], B: number[] = [];
    for (const x of all) { const swap = inA.has(x.day) && inB.has(x.day) ? false : flip.get(x.day) === 1; ((x.g === 1) !== swap ? A : B).push(x.r); }
    if (A.length > 1 && B.length > 1 && Math.abs(mean(A) - mean(B)) >= Math.abs(diff)) ge++;
  }
  return { diff, p: (ge + 1) / (reps + 1) };
}

export function benjaminiHochberg(ps: number[]) {
  const idx = ps.map((p, k) => [p, k] as const).sort((x, y) => x[0] - y[0]);
  const q = new Array<number>(ps.length);
  let min = 1;
  for (let r = idx.length - 1; r >= 0; r--) { min = Math.min(min, (idx[r]![0] * idx.length) / (r + 1)); q[idx[r]![1]] = min; }
  return q;
}
export const f = (x: number, d = 3) => (Number.isFinite(x) ? x.toFixed(d) : 'n/a');
