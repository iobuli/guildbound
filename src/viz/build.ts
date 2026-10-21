// Builds out/inspector.html: the bundled engine, every ingested market (packed H1), the user's page cases, and the study results.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { buildSync } from 'esbuild';
import { listMarkets, openMarket } from '../core/market-io.js';
import { BRAIN } from '../brain/brain.js';

const root = fileURLToPath(new URL('../../', import.meta.url));
const read = (p: string) => readFileSync(p, 'utf8');
const H = 3_600_000;

export function buildInspector() {
  const packs: Record<string, unknown> = {}, markets: Record<string, unknown> = {};
  for (const sym of listMarkets()) {
    const m = openMarket(sym), c = m.h1, p = m.profile, k = 10 ** Math.min(6, p.decimals);
    packs[sym] = { t0: c[0]!.t, k, dh: c.map((b, i) => (i ? (b.t - c[i - 1]!.t) / H : 0)), o: c.map((b) => Math.round(b.o * k)), h: c.map((b) => Math.round(b.h * k)), l: c.map((b) => Math.round(b.l * k)), c: c.map((b) => Math.round(b.c * k)) };
    markets[sym] = { instrument: m.instrument, session: p.session, profile: { cls: p.cls, tick: p.tick, clock: p.clock, costs: p.costs, split: p.split, from: p.data.from, to: p.data.to, ltf: p.data.ltf.map((x) => x.file), sources: p.data.sources, notes: p.notes } };
  }
  const data = Buffer.from(gzipSync(JSON.stringify(packs), { level: 9 })).toString('base64');
  const ts = (s: string) => Date.parse(s.replace(' ', 'T') + ':00Z');
  const tcPath = `${BRAIN}sources/pages/SOL_Trade_Check.data.json`, ohPath = `${BRAIN}sources/pages/One_Hour_At_A_Time.data.json`;
  const tc = existsSync(tcPath) ? JSON.parse(read(tcPath)) : { trades: [], summary: {} };
  const cases = tc.trades.map((t: any) => {
    const at = (i: number) => ts(t.c[i][0]), fb = t.c[t.entry_i], L = t.level, long = t.side === 'long';
    return { n: t.n, kind: t.kind, side: t.side, win: t.win, R: t.R, nT: t.nT, level: L, stop: t.stop, soft: t.soft, tgt: t.tgt, brkT: at(t.brk_i), touchT: t.touches.map(at), entryT: at(t.entry_i), exitT: at(t.exit_i), fb: fb.slice(1), fillOk: fb[3] <= L && L <= fb[2], closedThrough: long ? fb[4] < L : fb[4] > L };
  });
  let oh: unknown[] = [];
  if (existsSync(ohPath)) {
    const D = JSON.parse(read(ohPath)), F = D.frames;
    oh = F.flatMap((fr: any) => (fr.retest ?? []).filter((r: any) => r.T1 !== undefined).map((r: any) => {
      const long = r.side === 'long'; let farP = 0;
      for (let k = r.b + 1; k < fr.f; k++) farP = Math.max(farP, (long ? F[k].h - r.p : r.p - F[k].l) / fr.atr);
      return { f: fr.f, t: ts(fr.ts), p: r.p, side: r.side, n: r.n, bT: ts(F[r.b].ts), far: r.far, farPrior: Math.max(0, farP), ct: long ? fr.c < r.p : fr.c > r.p, T: [r.T1, r.T2, r.T3, r.T4] };
    }));
  }
  const study = existsSync(`${root}out/study/hypotheses.json`) ? JSON.parse(read(`${root}out/study/hypotheses.json`)) : null;
  const audit = existsSync(`${BRAIN}experiments/user-pages-audit.md`) ? read(`${BRAIN}experiments/user-pages-audit.md`) : '';
  const bundle = buildSync({ entryPoints: [fileURLToPath(new URL('entry.ts', import.meta.url))], bundle: true, minify: true, format: 'iife', platform: 'browser', target: 'es2022', write: false }).outputFiles[0]!.text.replace(/<\/script/gi, '<\\/script');
  const j = (x: unknown) => JSON.stringify(x).replace(/<\/script/gi, '<\\/script');
  let html = read(fileURLToPath(new URL('inspector.html', import.meta.url)));
  html = html.replace('/*@@BUNDLE@@*/', () => bundle).replace('@@DATA@@', () => data).replace('@@MARKETS@@', () => j(markets)).replace('@@CASES@@', () => j(cases)).replace('@@OH@@', () => j(oh))
    .replace('@@STUDY@@', () => j(study)).replace('@@AUDIT@@', () => j(audit)).replace('@@SUMMARY@@', () => j(tc.summary));
  mkdirSync(`${root}out/`, { recursive: true });
  writeFileSync(`${root}out/inspector.html`, html);
  return `out/inspector.html ${(html.length / 1e6).toFixed(2)} MB (engine ${(bundle.length / 1e3).toFixed(0)} kB, prices ${(data.length / 1e6).toFixed(2)} MB, markets ${Object.keys(markets).join(', ')})`;
}
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) console.log(buildInspector());
