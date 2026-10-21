// `kev atlas SYMBOL --from=2020 --to=2022`: the whole window read two ways.
//   causal:    every H1 bar replayed through the graph and the interpreter (what kev knew at that hour)
//   hindsight: every level kev marked, followed to the end of its life (what that level turned out to be)
// The two are kept apart in the output. Hindsight describes; it is never evidence of an edge.
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openMarket } from '../core/market-io.js';
import { withPreset } from '../core/presets.js';
import { aggregateSession } from '../core/session.js';
import { interpret, type Reading } from '../interpret.js';
import { placebos, runBacktest, type Scan } from '../backtest/engine.js';
import { ltfIndex } from '../backtest/settle.js';
import { clusterDiffBoot, clusterMean, mean } from '../backtest/stats.js';
import { log } from '../brain/brain.js';
import type { Candle } from '../core/types.js';

const H = 3_600_000, DAY = 86_400_000;
export const ATLAS_DIR = fileURLToPath(new URL('../../out/atlas/', import.meta.url));
const iso = (t: number) => new Date(t).toISOString().slice(0, 10);

export interface LevelId {
  id: number; dir: 1 | -1; price: number;
  first: number; last: number; seen: number;
  firstTouches: number; firstTier: string; firstBsb: number; firstTrend: string; firstVol: string; firstBias: string;
  maxTouches: number; flipOf: number | null;
  // hindsight, from bars after first sight
  tests: number; held: number; diedAt: number | null; firstTestAfterBars: number | null; zone: number;
}
export interface Leg { from: number; to: number; dir: 'up' | 'down'; pFrom: number; pTo: number; movePct: number; moveAtr: number; days: number }

/** Zigzag on daily closes: a leg ends when price reverses by `k` daily ATRs from its extreme. */
function zigzag(d: Candle[], atr: number[], k: number): Leg[] {
  const legs: Leg[] = [];
  if (d.length < 3) return legs;
  let dir: 'up' | 'down' = d[1]!.c >= d[0]!.c ? 'up' : 'down', startI = 0, extI = 0;
  for (let i = 1; i < d.length; i++) {
    const a = atr[i] || atr.find((x) => x > 0) || 1;
    if (dir === 'up') {
      if (d[i]!.h >= d[extI]!.h) extI = i;
      else if (d[extI]!.h - d[i]!.l >= k * a) { legs.push(leg(d, atr, startI, extI, 'up')); dir = 'down'; startI = extI; extI = i; }
    } else {
      if (d[i]!.l <= d[extI]!.l) extI = i;
      else if (d[i]!.h - d[extI]!.l >= k * a) { legs.push(leg(d, atr, startI, extI, 'down')); dir = 'up'; startI = extI; extI = i; }
    }
  }
  legs.push(leg(d, atr, startI, d.length - 1, dir));
  return legs;
}
function leg(d: Candle[], atr: number[], s: number, e: number, dir: 'up' | 'down'): Leg {
  const pFrom = dir === 'up' ? d[s]!.l : d[s]!.h, pTo = dir === 'up' ? d[e]!.h : d[e]!.l;
  return { from: d[s]!.t, to: d[e]!.t, dir, pFrom, pTo, movePct: pTo / pFrom - 1, moveAtr: Math.abs(pTo - pFrom) / (atr[s] || 1), days: Math.round((d[e]!.t - d[s]!.t) / DAY) };
}
function dailyAtr(d: Candle[], n = 20): number[] {
  const out: number[] = [0]; let a = 0;
  for (let i = 1; i < d.length; i++) { const tr = Math.max(d[i]!.h - d[i]!.l, Math.abs(d[i]!.h - d[i - 1]!.c), Math.abs(d[i]!.l - d[i - 1]!.c)); a = i <= n ? (a * (i - 1) + tr) / i : (a * (n - 1) + tr) / n; out.push(a); }
  return out;
}
/** Mann-Whitney AUC: probability a random positive has a higher score than a random negative (ties count half). */
function auc(pos: number[], neg: number[]): number {
  if (!pos.length || !neg.length) return NaN;
  let s = 0; for (const p of pos) for (const q of neg) s += p > q ? 1 : p === q ? 0.5 : 0;
  return s / (pos.length * neg.length);
}

export function atlas(sym: string, from: number, to: number, preset = 'kev-default', log2: (s: string) => void = () => {}) {
  const m = openMarket(sym), c = m.h1, feed = m.feed();
  const i0 = Math.max(c.findIndex((b) => b.t >= Math.max(from, m.profile.data.from + 45 * DAY)), 1), i1 = (() => { const k = c.findIndex((b) => b.t >= to); return k < 0 ? c.length - 1 : k - 1; })();
  const t0 = Date.now();
  return withPreset(preset, () => {
    // ---------- 1. causal replay, every bar ----------
    const steps: { t: number; r: Reading }[] = [];
    for (let i = i0; i <= i1; i++) {
      const T = c[i]!.t; // decided at the bar's open: the bar itself is still hidden
      steps.push({ t: T, r: interpret(feed, T, { instrument: m.instrument, price: c[i]!.o }) });
      if ((i - i0) % 2000 === 0) log2(`replay ${iso(T)} (${((Date.now() - t0) / 1000).toFixed(0)}s)`);
    }

    // ---------- 2-3. level identities, then each level's hindsight fate ----------
    const ids = mergeLevels(steps);
    const end = Math.min(c.length - 1, i1 + 24 * 90); // let fates finish up to 90 days past the window
    const idx = new Map(c.map((b, i) => [b.t, i]));
    for (const L of ids) fate(c, idx.get(L.first)!, end, L);

    // ---------- 4. phases: daily swing legs (3 ATR) inside major legs (8 ATR) ----------
    const d1 = aggregateSession(c.slice(i0, i1 + 1), m.session, 'day'), atr1 = dailyAtr(d1);
    const minor = zigzag(d1, atr1, 3), major = zigzag(d1, atr1, 8);

    // ---------- 5. events (causal) ----------
    const biasEv = steps.filter((s, k) => s.r.bias.fresh && s.r.bias.kinds.length && (k === 0 || !steps[k - 1]!.r.bias.fresh)).map((s) => ({ t: s.t, dir: s.r.bias.dir, kinds: s.r.bias.kinds }));
    const sfpEv: { t: number; tf: string; dir: string }[] = [];
    for (let k = 0; k < steps.length; k++) for (const s of steps[k]!.r.sfp) if (!(k && steps[k - 1]!.r.sfp.some((x) => x.tf === s.tf && x.dir === s.dir))) sfpEv.push({ t: steps[k]!.t, ...s });
    // bias follow-through: from the close of the signal candle (known when the signal fires) to the close of the next 1 and 3 trading days,
    // in the signal's direction, in daily ATRs. The outcome is hindsight; the signal is causal.
    const dIdx = (t: number) => d1.findIndex((b) => b.t + 12 * H > t); // the trading day that starts at (or contains) the signal time
    const follow = biasEv.map((e) => { const k = dIdx(e.t), sgn = e.dir === 'long' ? 1 : e.dir === 'short' ? -1 : 0; const base = k > 0 ? d1[k - 1]!.c : NaN, a = k > 0 ? atr1[k - 1] || 1 : 1; return { ...e, f1: k >= 0 && d1[k] ? sgn * (d1[k]!.c - base) / a : NaN, f3: k >= 0 && d1[k + 2] ? sgn * (d1[k + 2]!.c - base) / a : NaN }; });

    // ---------- 6. kev's causal fills in the window, against matched random orders ----------
    const bt = runBacktest(c, { start: i0, end: i1, session: m.session, inst: m.instrument, spreadBps: m.profile.costs.spreadBps, slipR: m.profile.costs.slipR, entryAtLevel: true, ltf: ltfIndex(m.ltf()) });
    const pl = placebos(c, bt.trades, new Map<number, Scan>(bt.scans.map((s) => [s.j, s])), bt.cfg, 2);
    const KEY = 'S0.5_T2', rows = (ts: typeof bt.trades) => ts.filter((t) => Number.isFinite(t.r[KEY])).map((t) => ({ r: t.r[KEY]!, day: t.day }));

    // ---------- 7. per-phase statistics ----------
    const inLeg = (t: number, g: Leg) => t >= g.from && t < g.to + DAY;
    const phaseStats = major.map((g) => {
      const lv = ids.filter((x) => inLeg(x.first, g)), withTest = lv.filter((x) => x.tests > 0);
      const supF = lv.filter((x) => x.dir === 1).length, resF = lv.filter((x) => x.dir === -1).length;
      const realP = bt.trades.filter((t) => inLeg(t.t, g)), plP = pl.filter((t) => inLeg(t.t, g));
      const fm = clusterMean(rows(realP), 300), fd = clusterDiffBoot(rows(realP), rows(plP), 300);
      const fb = follow.filter((e) => inLeg(e.t, g) && Number.isFinite(e.f1));
      const ch = steps.filter((s) => inLeg(s.t, g)), allRows = ch.flatMap((s) => s.r.levels);
      return {
        from: g.from, to: g.to, dir: g.dir, movePct: g.movePct, days: g.days,
        levels: lv.length, supportFlips: supF, resistanceFlips: resF,
        firstTestHeld: withTest.length ? withTest.filter((x) => x.held >= 1).length / withTest.length : NaN,
        neverTested: lv.length ? lv.filter((x) => x.tests === 0).length / lv.length : NaN,
        medianBarsToFirstTest: median(withTest.map((x) => x.firstTestAfterBars!)),
        strongShare: lv.length ? lv.filter((x) => x.held >= 3).length / lv.length : NaN,
        chopShare: allRows.length ? allRows.filter((r) => !r.ok).length / allRows.length : NaN,
        trendHours: { up: ch.filter((s) => s.r.keys.trend === 'up').length, down: ch.filter((s) => s.r.keys.trend === 'down').length, range: ch.filter((s) => s.r.keys.trend === 'range').length },
        biasSignals: fb.length, biasFollow1: mean(fb.map((e) => e.f1)), biasHit1: fb.length ? fb.filter((e) => e.f1 > 0).length / fb.length : NaN,
        fills: fm.n, fillMean: fm.mean, fillLo: fm.lo, fillHi: fm.hi, fillVsRandom: fd.diff, fillVsRandomLo: fd.lo, fillVsRandomHi: fd.hi,
        goFills: realP.filter((t) => t.go).length,
      };
    });

    // ---------- 8. could kev tell the strong levels apart when it first saw them? ----------
    const tested = ids.filter((x) => x.tests > 0), strong = tested.filter((x) => x.held >= 3), weak = tested.filter((x) => x.held < 3);
    const tierRank = (t: string) => (t === 'blind' ? 3 : t === 'needs-one-confluence' ? 2 : t === 'needs-two-confluences' ? 1 : 0);
    const align = (x: LevelId) => (x.firstTrend === 'up' && x.dir === 1) || (x.firstTrend === 'down' && x.dir === -1) ? 1 : x.firstTrend === 'range' ? 0.5 : 0;
    const biasAlign = (x: LevelId) => (x.firstBias === 'long' && x.dir === 1) || (x.firstBias === 'short' && x.dir === -1) ? 1 : x.firstBias === 'neutral' ? 0.5 : 0;
    const feats: [string, (x: LevelId) => number][] = [['touches at first sight', (x) => x.firstTouches], ['tier at first sight', (x) => tierRank(x.firstTier)], ['bars since break at first sight', (x) => x.firstBsb],
      ['regime trend agrees with the level', align], ['daily bias agrees with the level', biasAlign], ['low volatility at first sight', (x) => (x.firstVol === 'low' ? 1 : x.firstVol === 'normal' ? 0.5 : 0)], ['was a flip of an earlier level', (x) => (x.flipOf !== null ? 1 : 0)]];
    const hindsight = { tested: tested.length, strong: strong.length, strongShare: tested.length ? strong.length / tested.length : NaN, auc: feats.map(([name, fn]) => ({ name, auc: auc(strong.map(fn), weak.map(fn)) })),
      peakTouchesStrong: mean(strong.map((x) => x.maxTouches)), peakTouchesWeak: mean(weak.map((x) => x.maxTouches)), firstTouchesStrong: mean(strong.map((x) => x.firstTouches)), firstTouchesWeak: mean(weak.map((x) => x.firstTouches)) };

    const daily = d1.map((b, k) => {
      const s = steps.find((x) => x.t >= b.t + 12 * H) ?? steps[steps.length - 1]!;
      return { t: b.t, c: b.c, atr: +atr1[k]!.toPrecision(4), trend: s.r.keys.trend, vol: s.r.keys.vol, bias: s.r.keys.bias };
    });
    const fillAll = clusterMean(rows(bt.trades), 400), fillDiff = clusterDiffBoot(rows(bt.trades), rows(pl), 400);
    const out = {
      symbol: sym, preset, from: c[i0]!.t, to: c[i1]!.t, hours: steps.length, seconds: Math.round((Date.now() - t0) / 1000),
      phases: { major, minor }, phaseStats, daily,
      levels: ids.map((x) => ({ ...x, price: +x.price.toPrecision(6), zone: +x.zone.toPrecision(4) })),
      events: { bias: follow.map((e) => ({ t: e.t, dir: e.dir, kinds: e.kinds, f1: +e.f1.toFixed(2), f3: +e.f3.toFixed(2) })), sfp: sfpEv },
      fills: { n: fillAll.n, mean: fillAll.mean, lo: fillAll.lo, hi: fillAll.hi, vsRandom: fillDiff.diff, vsRandomLo: fillDiff.lo, vsRandomHi: fillDiff.hi, go: bt.trades.filter((t) => t.go).length },
      hindsight,
      totals: { levels: ids.length, supports: ids.filter((x) => x.dir === 1).length, resistances: ids.filter((x) => x.dir === -1).length, flips: ids.filter((x) => x.flipOf !== null).length, tested: tested.length, neverTested: ids.length - tested.length, died: ids.filter((x) => x.diedAt !== null).length,
        firstTestHeld: tested.length ? tested.filter((x) => x.held >= 1).length / tested.length : NaN, medianTests: median(tested.map((x) => x.tests)), biasSignals: follow.length, sfpSignals: sfpEv.length },
    };
    mkdirSync(ATLAS_DIR, { recursive: true });
    const name = `${sym}_${iso(from).slice(0, 4)}-${iso(to - 1).slice(0, 4)}`;
    writeFileSync(`${ATLAS_DIR}${name}.json`, JSON.stringify(out));
    writeFileSync(`${ATLAS_DIR}${name}.md`, summary(out));
    log(`atlas ${name}: ${steps.length} hours replayed, ${ids.length} levels, ${major.length} major phases`);
    return out;
  });
}

/** The same level seen in successive scans is one identity: same side, within 0.25 daily ATR, seen again within 30 days. */
export function mergeLevels(steps: { t: number; r: Pick<Reading, 'levels' | 'atrD1' | 'keys'> }[]): LevelId[] {
  const ids: LevelId[] = [], active: LevelId[] = [];
  for (const { t, r } of steps) {
    const tol = 0.25 * (r.atrD1 || 1);
    for (const L of r.levels) {
      let hit = active.find((x) => x.dir === L.d && Math.abs(x.price - L.p) <= tol && t - x.last <= 30 * DAY);
      if (!hit) {
        const prev = ids.slice().reverse().find((x) => x.dir === -L.d && Math.abs(x.price - L.p) <= tol && t - x.last <= 30 * DAY) ?? null;
        hit = { id: ids.length, dir: L.d, price: L.p, first: t, last: t, seen: 0, firstTouches: L.n, firstTier: L.tier, firstBsb: L.bsb, firstTrend: r.keys.trend, firstVol: r.keys.vol, firstBias: r.keys.bias,
          maxTouches: L.n, flipOf: prev ? prev.id : null, tests: 0, held: 0, diedAt: null, firstTestAfterBars: null, zone: 0.1 * (r.atrD1 || 1) };
        ids.push(hit); active.push(hit);
      }
      hit.last = t; hit.seen++; hit.maxTouches = Math.max(hit.maxTouches, L.n);
    }
    for (let k = active.length - 1; k >= 0; k--) if (t - active[k]!.last > 30 * DAY) active.splice(k, 1);
  }
  return ids;
}

/**
 * Hindsight fate from the bar AFTER first sight (index s) to `end`. A test = price comes back into the zone after a whole bar spent
 * 0.25 ATR clear of it; it held if price then got 0.5 ATR clear again before a close 0.1 ATR beyond the zone, which ends the level.
 */
export function fate(c: readonly Candle[], s: number, end: number, L: LevelId) {
  const z = L.zone, a = z * 10, lo = L.price - z, hi = L.price + z, sup = L.dir === 1;
  let inTest = false, away = false;
  for (let j = s + 1; j <= end; j++) {
    const b = c[j]!;
    if (sup ? b.c < lo - z : b.c > hi + z) { L.diedAt = b.t; break; }
    const touching = sup ? b.l <= hi : b.h >= lo;
    if (touching && away && !inTest) { L.tests++; inTest = true; away = false; if (L.firstTestAfterBars === null) L.firstTestAfterBars = j - s; }
    if (inTest && (sup ? b.h - hi : lo - b.l) >= 0.5 * a) { L.held++; inTest = false; }
    if (!touching && (sup ? b.l - hi : lo - b.h) >= 0.25 * a) away = true;
  }
  return L;
}
function median(xs: number[]): number { const s = xs.filter(Number.isFinite).sort((a, b) => a - b); return s.length ? s[Math.floor(s.length / 2)]! : NaN; }

const f = (x: number, d = 2) => (Number.isFinite(x) ? x.toFixed(d) : 'n/a'), P = (x: number) => (Number.isFinite(x) ? `${Math.round(100 * x)}%` : 'n/a');
export function summary(a: ReturnType<typeof atlas>): string {
  const L = [`# Atlas ${a.symbol} ${iso(a.from)} to ${iso(a.to)} (${a.preset})`, '', `${a.hours} hours replayed causally in ${a.seconds}s. Levels and fates below are HINDSIGHT unless marked causal.`, '',
    `## Totals`, '', `- Levels kev marked (identities): ${a.totals.levels} (${a.totals.supports} support, ${a.totals.resistances} resistance; ${a.totals.flips} were flips of an earlier level).`,
    `- Tested after first sight: ${a.totals.tested}; never tested: ${a.totals.neverTested}; closed through (died): ${a.totals.died}. First test held: ${P(a.totals.firstTestHeld)}; median tests per tested level: ${a.totals.medianTests}.`,
    `- Daily bias signals: ${a.totals.biasSignals}; fresh SFPs: ${a.totals.sfpSignals}.`,
    `- Causal fills (stop 0.5 daily ATR, 2R, net): ${a.fills.n}, mean ${f(a.fills.mean)}R [${f(a.fills.lo)}, ${f(a.fills.hi)}], vs random orders ${f(a.fills.vsRandom)}R [${f(a.fills.vsRandomLo)}, ${f(a.fills.vsRandomHi)}]; GO ${a.fills.go}.`, '',
    '## Major phases (daily swings of 8+ ATR)', '', '| from | to | leg | move | days | levels (sup/res) | 1st test held | never tested | strong (3+ holds) | chop | bias hit next day | fills mean R | fills vs random |', '|---|---|---|---|---|---|---|---|---|---|---|---|---|',
    ...a.phaseStats.map((p) => `| ${iso(p.from)} | ${iso(p.to)} | ${p.dir} | ${f(100 * p.movePct, 0)}% | ${p.days} | ${p.levels} (${p.supportFlips}/${p.resistanceFlips}) | ${P(p.firstTestHeld)} | ${P(p.neverTested)} | ${P(p.strongShare)} | ${P(p.chopShare)} | ${P(p.biasHit1)} (${p.biasSignals}) | ${f(p.fillMean)} (${p.fills}) | ${f(p.fillVsRandom)} [${f(p.fillVsRandomLo)}, ${f(p.fillVsRandomHi)}] |`), '',
    '## Could kev tell the strong levels apart when it first saw them?', '', `${a.hindsight.strong} of ${a.hindsight.tested} tested levels held 3+ tests (${P(a.hindsight.strongShare)}). Peak touches: strong ${f(a.hindsight.peakTouchesStrong, 1)} vs weak ${f(a.hindsight.peakTouchesWeak, 1)}; touches when first seen: strong ${f(a.hindsight.firstTouchesStrong, 1)} vs weak ${f(a.hindsight.firstTouchesWeak, 1)}.`, '',
    '| feature known at first sight | AUC (0.5 = no better than a coin) |', '|---|---|', ...a.hindsight.auc.map((x) => `| ${x.name} | ${f(x.auc)} |`), '',
    '## Minor legs (3+ ATR)', '', a.phases.minor.map((g) => `${iso(g.from)}→${iso(g.to)} ${g.dir} ${f(100 * g.movePct, 0)}%`).join(' · '), ''];
  return L.join('\n');
}
