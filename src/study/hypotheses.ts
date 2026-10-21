// `kev study`: the pre-registered cross-market battery (kev-brain/experiments/2026-10-21-cross-market.md).
// Phase 1 runs every harness job once (child processes, 4 at a time). Phase 2 reads their JSON and applies the decision rules.
import { spawn } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openMarket } from '../core/market-io.js';
import { aggregateSession, buildSession } from '../core/session.js';
import { wilderAtr } from '../core/ta.js';
import { mean, clusterMean, clusterDiffBoot, benjaminiHochberg } from '../backtest/stats.js';
import { BRAIN, log } from '../brain/brain.js';
import { cell, diffOfDiffs, fmt, fromCompact, rows, table, type Rec } from './analyze.js';
import { OUT, runId, type RunOpts } from './run.js';

const MARKETS = ['SOLUSDT', 'ETHUSD', 'XAUUSD'] as const;
const KEY = 'S0.5_T2';
const JOBS: [string, RunOpts][] = [
  ...MARKETS.flatMap((m) => [[m, { preset: 'one-hour-spec' }], [m, { preset: 'kev-default' }]] as [string, RunOpts][]),
  ['XAUUSD', { preset: 'one-hour-spec', session: 'utc-raw' }], ['SOLUSDT', { preset: 'one-hour-spec', session: 'ny-close' }], ['ETHUSD', { preset: 'one-hour-spec', session: 'ny-close' }],
];
const EXP = `${BRAIN}experiments/2026-10-21-cross-market.md`;

export async function runJobs(force = false, parallel = 4) {
  // a job is done only if a FULL-range run of it exists (a short test run under the same name does not count)
  const done = (m: string, o: RunOpts) => { const f = `${OUT}${runId(m, o)}.json`; if (!existsSync(f)) return false; try { return JSON.parse(readFileSync(f, 'utf8')).full === true; } catch { return false; } };
  const todo = JOBS.filter(([m, o]) => force || !done(m, o));
  const cli = fileURLToPath(new URL('../cli.ts', import.meta.url));
  mkdirSync(`${OUT}logs/`, { recursive: true });
  let next = 0, running = 0;
  await new Promise<void>((done) => {
    if (!todo.length) return done();
    const go = () => {
      while (running < parallel && next < todo.length) {
        const [m, o] = todo[next++]!; running++;
        const args = ['--import', 'tsx', cli, 'backtest', m, `--preset=${o.preset}`, ...(o.session ? [`--session=${o.session}`] : [])];
        const logf = `${OUT}logs/${runId(m, o)}.log`;
        const p = spawn(process.execPath, args, { stdio: ['ignore', 'pipe', 'pipe'] });
        p.stdout.on('data', (d) => appendFileSync(logf, d)); p.stderr.on('data', (d) => appendFileSync(logf, d));
        p.on('exit', (code) => { running--; console.error(`${runId(m, o)} finished (${code})`); if (next >= todo.length && running === 0) done(); else go(); });
      }
    };
    go();
  });
}

const load = (m: string, o: RunOpts) => {
  const j = JSON.parse(readFileSync(`${OUT}${runId(m, o)}.json`, 'utf8'));
  return { real: (j.trades as never[]).map(fromCompact), plc: (j.placebo as never[]).map(fromCompact), isEnd: j.split.isEndYear as number, seconds: j.seconds as number, scans: j.scans as number };
};

export function analyse() {
  const R: Record<string, unknown> = {}, md: string[] = [];
  const primary = Object.fromEntries(MARKETS.map((m) => [m, load(m, { preset: 'one-hour-spec' })]));
  const secondary = Object.fromEntries(MARKETS.map((m) => [m, load(m, { preset: 'kev-default' })]));
  const sp = (t: Rec) => t.spaceOk;
  md.push(`## Results (appended ${new Date().toISOString().slice(0, 10)} by \`kev study\`)`, '', `Primary preset one-hour-spec; outcome ${KEY}; real vs its own matched placebos.`, '');
  const ov = MARKETS.map((m) => cell(`${m} all fills`, primary[m]!.real, primary[m]!.plc, KEY, () => true));
  md.push('### Overview (all fills)', '', table([...ov, ...MARKETS.map((m) => cell(`${m} GO at fill (descriptive)`, primary[m]!.real, primary[m]!.plc, KEY, (t) => t.go)), ...MARKETS.map((m) => cell(`${m} all fills (kev-default parser)`, secondary[m]!.real, secondary[m]!.plc, KEY, () => true))]), '');
  R.overview = ov.map((c) => `${c.name.split(' ')[0]} ${fmt(c.mean)}R vs random ${fmt(c.plMean)}R (diff ${fmt(c.diff)} [${fmt(c.dLo)}, ${fmt(c.dHi)}])`).join('; ');

  // ---- H1 space ----
  const h1 = [...MARKETS.map((m) => cell(`${m} space ok`, primary[m]!.real, primary[m]!.plc, KEY, sp)),
    cell('ETH + gold pooled, space ok', [...primary.ETHUSD!.real, ...primary.XAUUSD!.real], [...primary.ETHUSD!.plc, ...primary.XAUUSD!.plc], KEY, sp)];
  const q1 = benjaminiHochberg(h1.map((c) => (Number.isFinite(c.p) ? c.p : 1))); h1.forEach((c, k) => (c.q = q1[k]));
  const eth = h1[1]!, gold = h1[2]!, pooled = h1[3]!;
  const h1verdict = eth.diff > 0 && gold.diff > 0 && (pooled.q ?? 1) < 0.05 ? 'CONFIRMED' : pooled.dHi < 0.05 ? 'REJECTED' : 'INCONCLUSIVE';
  const net = MARKETS.map((m) => ({ m, ...clusterMean(rows(primary[m]!.real.filter(sp), KEY), 600) }));
  const h1split = MARKETS.flatMap((m) => [cell(`${m} space ok, in-sample <= ${primary[m]!.isEnd}`, primary[m]!.real, primary[m]!.plc, KEY, (t) => sp(t) && t.year <= primary[m]!.isEnd), cell(`${m} space ok, out-of-sample`, primary[m]!.real, primary[m]!.plc, KEY, (t) => sp(t) && t.year > primary[m]!.isEnd)]);
  const h1sec = MARKETS.map((m) => cell(`${m} space ok (kev-default parser)`, secondary[m]!.real, secondary[m]!.plc, KEY, sp));
  md.push('### H1 space', '', table(h1), '', `**Verdict: ${h1verdict}** (rule: real - placebo > 0 in ETH and gold, pooled q < 0.05; rejected if the pooled CI is below +0.05R).`, '',
    `Net mean R of space-ok fills: ${net.map((x) => `${x.m} ${fmt(x.mean)} [${fmt(x.lo)}, ${fmt(x.hi)}] (n ${x.n})`).join('; ')}.`, '', 'Split and robustness (not part of the decision):', '', table([...h1split, ...h1sec]), '');
  R.H1 = { cells: h1, verdict: h1verdict, net, split: h1split, secondary: h1sec };

  // ---- H2 touches ----
  const h2 = MARKETS.flatMap((m) => (['one-hour-spec', 'kev-default'] as const).map((p) => {
    const d = p === 'one-hour-spec' ? primary[m]! : secondary[m]!;
    const g = (fn: (t: Rec) => boolean) => [rows(d.real.filter(fn), KEY), rows(d.plc.filter(fn), KEY)] as const;
    const [aR, aP] = g((t) => t.touches >= 3), [bR, bP] = g((t) => t.touches === 1);
    return { m, preset: p, nHigh: aR.length, nOne: bR.length, ...diffOfDiffs(aR, aP, bR, bP) };
  }));
  const q2 = benjaminiHochberg(h2.map((x) => (Number.isFinite(x.p) ? x.p : 1))); h2.forEach((x, k) => ((x as { q?: number }).q = q2[k]));
  const h2bad = h2.filter((x) => x.d > 0.1 && ((x as { q?: number }).q ?? 1) < 0.05);
  md.push('### H2 touches', '', '| market | parser | n touches>=3 | n touches=1 | (real-placebo | >=3) - (real-placebo | =1) [95% CI] | p | q |', '|---|---|---|---|---|---|---|',
    ...h2.map((x) => `| ${x.m} | ${x.preset} | ${x.nHigh} | ${x.nOne} | ${fmt(x.d)} [${fmt(x.lo)}, ${fmt(x.hi)}] | ${fmt(x.p, 3)} | ${fmt((x as { q?: number }).q ?? NaN, 3)} |`), '',
    `**Verdict: ${h2bad.length ? `CONTRADICTED in ${h2bad.map((x) => `${x.m}/${x.preset}`).join(', ')}` : 'as expected, no touch-count effect beyond 0.1R with q < 0.05'}.**`, '');
  R.H2 = { cells: h2, contradicted: h2bad.length > 0 };

  // ---- H3 lookahead demonstration (placebos only) ----
  const h3 = MARKETS.map((m) => { const pl = primary[m]!.plc; const d = clusterDiffBoot(rows(pl.filter((t) => t.closedThrough), KEY), rows(pl.filter((t) => !t.closedThrough), KEY), 800); return { m, nCT: pl.filter((t) => t.closedThrough).length, nNot: pl.filter((t) => !t.closedThrough).length, ct: mean(rows(pl.filter((t) => t.closedThrough), KEY).map((x) => x.r)), not: mean(rows(pl.filter((t) => !t.closedThrough), KEY).map((x) => x.r)), ...d }; });
  const h3ok = h3.every((x) => x.diff < -0.2);
  md.push('### H3 lookahead demonstration (random orders only)', '', '| market | placebo fills | closed through | mean R closed through | mean R not | difference [95% CI] |', '|---|---|---|---|---|---|',
    ...h3.map((x) => `| ${x.m} | ${x.nCT + x.nNot} | ${x.nCT} | ${fmt(x.ct)} | ${fmt(x.not)} | ${fmt(x.diff)} [${fmt(x.lo)}, ${fmt(x.hi)}] |`), '',
    `**Verdict: ${h3ok ? 'CONFIRMED' : 'NOT confirmed in every market'}** (rule: difference < -0.2R in every market). Random orders whose fill bar closed through the price average ${h3.map((x) => `${x.m} ${fmt(x.ct)}`).join(', ')} R; the ones that did not average ${h3.map((x) => `${x.m} ${fmt(x.not)}`).join(', ')} R.`, '');
  R.H3 = { cells: h3, confirmed: h3ok };

  // ---- H4 settlement bias ----
  const h4 = (['ETHUSD', 'XAUUSD'] as const).flatMap((m) => [KEY, '2'].map((k) => {
    const d = primary[m]!.real.filter((t) => t.rc && Number.isFinite(t.r[k]) && Number.isFinite(t.rc[k]));
    const cm = clusterMean(d.map((t) => ({ r: t.r[k]! - t.rc![k]!, day: t.day })), 600);
    return { m, key: k, ...cm, n: d.length, ltfShare: d.filter((t) => t.ltf).length / Math.max(1, d.length), changed: d.filter((t) => Math.abs(t.r[k]! - t.rc![k]!) > 1e-9).length, fine: mean(d.map((t) => t.r[k]!)), cons: mean(d.map((t) => t.rc![k]!)) };
  }));
  md.push('### H4 settlement bias (finer bars minus conservative H1 rules, same trades)', '', '| market | outcome | trades | settled with finer bars | outcomes changed | mean R finer | mean R conservative | bias [95% CI] |', '|---|---|---|---|---|---|---|---|',
    ...h4.map((x) => `| ${x.m} | ${x.key === '2' ? 'structural 2R' : x.key} | ${x.n} | ${(100 * x.ltfShare).toFixed(0)}% | ${x.changed} | ${fmt(x.fine)} | ${fmt(x.cons)} | ${fmt(x.mean, 3)} [${fmt(x.lo, 3)}, ${fmt(x.hi, 3)}] |`), '');
  R.H4 = h4;

  // ---- H5 sessions ----
  const raw = load('XAUUSD', { preset: 'one-hour-spec', session: 'utc-raw' }), sNy = load('SOLUSDT', { preset: 'one-hour-spec', session: 'ny-close' }), eNy = load('ETHUSD', { preset: 'one-hour-spec', session: 'ny-close' });
  const gm = openMarket('XAUUSD'), atrMed = (spec: Parameters<typeof buildSession>[0]) => {
    const s = buildSession(spec, gm.h1), d = aggregateSession(gm.h1, s, 'day'), a: number[] = [], per = new Map<number, number>();
    for (const b of gm.h1) per.set(s.dayStart(b.t), (per.get(s.dayStart(b.t)) ?? 0) + 1);
    for (let i = 25; i < d.length; i++) a.push(wilderAtr(d.slice(Math.max(0, i - 120), i), 20));
    a.sort((x, y) => x - y);
    return { median: a[Math.floor(a.length / 2)]!, days: d.length, short: [...per.values()].filter((n) => n < 6).length };
  };
  const nyA = atrMed(gm.profile.session), rawA = atrMed({ ...gm.profile.session, day: 'utc', stubHours: 0 });
  const sess = [
    { name: 'XAUUSD NY-close (v2)', d: primary.XAUUSD! }, { name: 'XAUUSD UTC days with Sunday stubs (old)', d: raw },
    { name: 'SOLUSDT UTC days (v2)', d: primary.SOLUSDT! }, { name: 'SOLUSDT NY-close days', d: sNy },
    { name: 'ETHUSD UTC days (v2)', d: primary.ETHUSD! }, { name: 'ETHUSD NY-close days', d: eNy },
  ].map((x) => ({ name: x.name, fills: x.d.real.length, ...clusterMean(rows(x.d.real, KEY), 400), structural2: mean(rows(x.d.real.filter((t) => t.ok), '2').map((y) => y.r)), placebo: mean(rows(x.d.plc, KEY).map((y) => y.r)) }));
  md.push('### H5 sessions', '', `Gold daily bars: ${nyA.days} NY-close days (${nyA.short} with under 6 hours of bars) vs ${rawA.days} UTC days (${rawA.short} under 6 hours); median daily ATR(20) ${fmt(nyA.median, 2)} vs ${fmt(rawA.median, 2)} (${fmt(100 * (nyA.median / rawA.median - 1), 1)}% larger once Sunday stubs are gone).`, '',
    '| run | fills | mean R ' + KEY + ' [95% CI] | placebo | structural 2R |', '|---|---|---|---|---|', ...sess.map((x) => `| ${x.name} | ${x.fills} | ${fmt(x.mean)} [${fmt(x.lo)}, ${fmt(x.hi)}] | ${fmt(x.placebo)} | ${fmt(x.structural2)} |`), '');
  R.H5 = { goldAtr: { ny: nyA, raw: rawA }, runs: sess };

  mkdirSync(fileURLToPath(new URL('../../out/study/', import.meta.url)), { recursive: true });
  writeFileSync(fileURLToPath(new URL('../../out/study/hypotheses.json', import.meta.url)), JSON.stringify(R, null, 1));
  const exp = readFileSync(EXP, 'utf8'), cut = exp.indexOf('\n## Results');
  writeFileSync(EXP, (cut >= 0 ? exp.slice(0, cut) : exp).trimEnd() + '\n\n' + md.join('\n') + '\n');
  writeFindings(R);
  log(`study: H1 ${h1verdict}; H2 ${h2bad.length ? 'contradicted' : 'no touch effect'}; H3 ${h3ok ? 'confirmed' : 'not confirmed'}; results in experiments/2026-10-21-cross-market.md`);
  return md.join('\n');
}

function writeFindings(R: Record<string, any>) {
  const h1 = R.H1, h3 = R.H3;
  const L = ['# Findings', '', `What the data says so far. Generated by \`kev study\` on ${new Date().toISOString().slice(0, 10)}; details and tables in [experiments/2026-10-21-cross-market.md](experiments/2026-10-21-cross-market.md) and [experiments/user-pages-audit.md](experiments/user-pages-audit.md).`, '',
    '| question | answer | evidence |', '|---|---|---|',
    `| Do fills at the method's levels beat random orders at the same distance? | ${R.overview} | all-fills rows of the cross-market overview (stop 0.5 daily ATR, 2R, net of costs) |`,
    `| H1: does space (0.5 ATR clear before the return, prior bars only) add an edge? | ${h1.verdict} | pooled ETH+gold real-placebo ${fmt(h1.cells[3].diff)} [${fmt(h1.cells[3].dLo)}, ${fmt(h1.cells[3].dHi)}], q ${fmt(h1.cells[3].q, 3)} |`,
    `| H2: do more touches make a better level? | ${R.H2.contradicted ? 'yes somewhere (see table)' : 'no'} | difference-of-differences per market and parser |`,
    `| H3: does a "fill bar did not close through" filter create a fake edge from random orders? | ${h3.confirmed ? 'yes, in every market' : 'not in every market'} | ${h3.cells.map((x: any) => `${x.m} ${fmt(x.diff)}`).join(', ')} R |`,
    `| H4: how much do worst-case intrabar rules understate results? | ${R.H4.map((x: any) => `${x.m} ${x.key}: ${fmt(x.mean, 3)}R`).join('; ')} | same trades settled both ways |`,
    `| H5: did the Sunday-stub bug matter for gold? | median daily ATR ${fmt(100 * (R.H5.goldAtr.ny.median / R.H5.goldAtr.raw.median - 1), 1)}% larger after the fix | session runs table |`,
    '| Do the user\'s pages trade at knowable times? | no: Trade Check keeps trades by the fill bar\'s close; One Hour\'s space term includes the decision bar | experiments/user-pages-audit.md |', ''];
  writeFileSync(`${BRAIN}findings.md`, L.join('\n'));
}
