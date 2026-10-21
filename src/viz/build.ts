// Builds out/inspector.html: the bundled engine + packed price data + the Trade Check cases + the page template.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { buildSync } from 'esbuild';
import type { Candle } from '../core/types.js';

const root = new URL('../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');
const H = 3_600_000;

function pack(c: Candle[], k: number) {
  const dh: number[] = [], q = (x: number) => Math.round(x * k);
  c.forEach((b, i) => dh.push(i === 0 ? 0 : (b.t - c[i - 1]!.t) / H));
  return { t0: c[0]!.t, k, dh, o: c.map((b) => q(b.o)), h: c.map((b) => q(b.h)), l: c.map((b) => q(b.l)), c: c.map((b) => q(b.c)) };
}
const sol: Candle[] = JSON.parse(read('data/SOLUSDT_1h.json')), gold: Candle[] = JSON.parse(read('data/XAUUSD_h1.json'));
const data = Buffer.from(gzipSync(JSON.stringify({ SOL: pack(sol, 1e4), GOLD: pack(gold, 1e3) }), { level: 9 })).toString('base64');

const tc = JSON.parse(read('data/trade_check_sol.json'));
const ts = (s: string) => Date.parse(s.replace(' ', 'T') + ':00Z');
const cases = tc.trades.map((t: any) => {
  const at = (i: number) => ts(t.c[i][0]), fb = t.c[t.entry_i], L = t.level, long = t.side === 'long';
  return {
    n: t.n, kind: t.kind, side: t.side, win: t.win, R: t.R, nT: t.nT, level: L, stop: t.stop, soft: t.soft, tgt: t.tgt, riskPct: t.risk_pct, mfe: t.mfe, bars: t.bars,
    brkT: at(t.brk_i), touchT: t.touches.map(at), entryT: at(t.entry_i), exitT: at(t.exit_i), fb: fb.slice(1),
    fillOk: fb[3] <= L && L <= fb[2], closedThrough: long ? fb[4] < L : fb[4] > L,
  };
});
const compat = JSON.parse(read('out/compat.json')).filter((r: any) => !r.held).map((r: any) => ({ mkt: r.mkt, tier: r.minTier, space: r.space, fills: r.fills, r4: r.r4.mean, lo: r.r4.lo, hi: r.r4.hi, pl: r.pl4.mean, is: r.is4.mean, oos: r.oos4.mean, win: r.one4.win, perYear: r.one4.perYear }));

// One Hour decisions: their level, side, touch count and `far`, next to the same measure computed from bars strictly before the decision bar.
const oh = JSON.parse(read('data/one_hour_sol.json')), solIdx = new Map(sol.map((b, i) => [b.t, i]));
const ohDec = oh.frames.flatMap((fr: any) => (fr.retest ?? []).filter((r: any) => r.T1 !== undefined).map((r: any) => {
  const long = r.side === 'long', atr = fr.atr, t = ts(fr.ts), j = solIdx.get(t)!, bar = sol[j]!;
  let farP = 0; for (let k = r.b + 1; k < fr.f; k++) { const g = oh.frames[k]; farP = Math.max(farP, ((long ? g.h - r.p : r.p - g.l)) / atr); }
  return { f: fr.f, t, p: r.p, side: r.side, n: r.n, bT: ts(oh.frames[r.b].ts), far: r.far, farPrior: Math.max(0, farP), ct: long ? bar.c < r.p : bar.c > r.p, T: [r.T1, r.T2, r.T3, r.T4], atr };
}));
const study = JSON.parse(read('out/sol_levels.json'));
const bundle = buildSync({ entryPoints: [new URL('entry.ts', import.meta.url).pathname], bundle: true, minify: true, format: 'iife', platform: 'browser', target: 'es2022', write: false }).outputFiles[0]!.text.replace(/<\/script/gi, '<\\/script');
let html = read('src/viz/inspector.html');
html = html.replace('/*@@BUNDLE@@*/', () => bundle).replace('@@DATA@@', () => data).replace('@@CASES@@', () => JSON.stringify(cases)).replace('@@COMPAT@@', () => JSON.stringify(compat)).replace('@@OH@@', () => JSON.stringify(ohDec)).replace('@@STUDY@@', () => JSON.stringify(study)).replace('@@SUMMARY@@', () => JSON.stringify(tc.summary));
mkdirSync(new URL('out/', root), { recursive: true });
writeFileSync(new URL('out/inspector.html', root), html);
console.log(`out/inspector.html ${(html.length / 1e6).toFixed(2)} MB (bundle ${(bundle.length / 1e3).toFixed(0)} kB, data ${(data.length / 1e6).toFixed(2)} MB)`);
