// kev's own reading of one moment: five layers from the weekly context down to the order it would place, plus how strongly each node
// shapes that reading (salience, 0-1). Deterministic: the same bars always give the same words. Runs the graph twice: once to mark the
// levels, once more on the nearest tradable level so the decision layer is about a real candidate.
import { graph, nodes } from './kev.js';
import type { Feed } from './core/series.js';
import type { RunResult } from './core/graph.js';
import { SetupSchema, type Instrument } from './core/types.js';

type Any = any; // node outputs are validated by their ports; here they are only read
export type Status = 'pass' | 'fail' | 'judgement' | 'info' | 'not-run' | 'card';
export interface Layer { id: 'regime' | 'bias' | 'location' | 'structure' | 'decision'; title: string; text: string }
export interface Reading {
  asOf: number; price: number; layers: Layer[]; headline: string;
  salience: Record<string, number>; status: Record<string, Status>; changed?: string[];
  focus: { price: number; dir: 'long' | 'short'; touches: number; tier: string; distAtr: number } | null;
  firstBlocker: { node: string; label: string } | null;
  keys: { trend: string; vol: string; bias: string; nLevels: number; go: boolean; spaceOk: boolean | null };
  /** the levels kev marked at this moment (compact): price, dir, touches, tier, bars since break, tradable */
  levels: { p: number; d: 1 | -1; n: number; tier: string; bsb: number; ok: boolean }[];
  atrD1: number;
  bias: { dir: string; fresh: boolean; kinds: string[] };
  sfp: { tf: string; dir: string }[];
}

const f = (x: number, d = 2) => (Number.isFinite(x) ? x.toFixed(d) : 'n/a');
const px = (x: number) => (Math.abs(x) >= 100 ? x.toFixed(1) : Math.abs(x) >= 1 ? x.toFixed(3) : x.toPrecision(4));
const clip = (x: number) => Math.max(0, Math.min(1, Number.isFinite(x) ? x : 0));
const pct = (x: number) => `${Math.round(100 * x)}%`;

function statuses(res: RunResult): Record<string, Status> {
  const out: Record<string, Status> = {};
  for (const n of nodes) {
    const st = res.states[n.id];
    if (!n.evaluate) { out[n.id] = 'card'; continue; }
    if (!st || !st.ran) { out[n.id] = 'not-run'; continue; }
    const fails = st.checks.some((c) => c.pass === false), judge = st.checks.some((c) => c.pass === null);
    out[n.id] = fails ? 'fail' : judge ? 'judgement' : st.checks.length ? 'pass' : 'info';
  }
  return out;
}

export function interpret(feed: Feed, T: number, ctx: { instrument: Instrument; price?: number }): Reading {
  const scanRun = graph.run(feed, T, SetupSchema.parse({ dir: 'long' }), { instrument: ctx.instrument, price: ctx.price });
  const S = (r: RunResult, id: string, port: string): Any => r.states[id]?.out[port];
  const atr: Any = S(scanRun, 'atr', 'atr') ?? { d1: 0, h1: 0 }, reg: Any = S(scanRun, 'regime', 'regime'), bias: Any = S(scanRun, 'daily-bias', 'bias');
  const scan: Any = S(scanRun, 'level-scan', 'levels') ?? { price: 0, rows: [] }, sfp: Any = S(scanRun, 'sfp', 'sfp') ?? { h1: [], d1: [] };
  const price = ctx.price ?? scan.price ?? feed.h1[0]?.c ?? 0;
  const rows: Any[] = scan.rows ?? [], a1 = atr.d1 || 1;
  const near = (side: 'below' | 'above') => rows.filter((r) => r.side === side).sort((x, y) => x.distAtr - y.distAtr)[0] ?? null;
  const below = near('below'), above = near('above');
  const focusRow = rows.filter((r) => r.tradable).sort((x, y) => x.distAtr - y.distAtr)[0] ?? null;

  // second pass: the decision for the nearest tradable level
  let fr: RunResult | null = null;
  if (focusRow) {
    const entry = focusRow.price;
    fr = graph.run(feed, T, SetupSchema.parse({ dir: focusRow.dir, style: 'sr-flip', levelPrice: focusRow.price, entry }), { instrument: ctx.instrument, price: ctx.price });
  }
  const R = fr ?? scanRun;
  const lvl: Any = S(R, 'level', 'level'), rt: Any = S(R, 'retest', 'retest'), cf: Any = S(R, 'confluence', 'confluence'), gate: Any = S(R, 'atr-gate', 'gate');
  const stop: Any = S(R, 'stop', 'stop'), tgt: Any = S(R, 'target', 'target'), ent: Any = S(R, 'entry', 'entry'), cx: Any = S(R, 'context', 'context');
  const tue: Any = S(R, 'tuesday-rto', 'tuesday'), news: Any = S(R, 'news', 'news'), risk: Any = S(R, 'risk', 'risk');
  const flow = graph.flow(R);

  // ---- layers ----
  const L1 = reg ? `Trend ${reg.trend} (20-day mean ${reg.slopeAtr >= 0 ? '+' : ''}${f(reg.slopeAtr, 1)} ATR in 10 days), weekly ${reg.weekTrend}. Volatility ${reg.volState} (${pct(reg.volPct)} of the past year; a day moves ${f(100 * reg.atrPct, 1)}% of price). Price is ${pct(reg.rangePos)} up its 60-day range and ${f(-100 * reg.drawdown, 0)}% below its high.` : 'Not enough daily history yet.';
  const sig = (b: Any) => (b?.signals?.length ? b.signals.map((s: Any) => `${s.dir === 'long' ? 'bullish' : 'bearish'} ${s.kind} ${s.ageBars === 0 ? 'yesterday' : `${s.ageBars + 1} bars ago`}`).join(', ') : 'no pattern');
  const L2 = bias ? `Daily bias ${bias.d1.dir}${bias.d1.dir !== 'neutral' ? ` from ${sig(bias.d1)}${bias.d1.fresh ? ' (fresh)' : ''}` : ''}${bias.d1.cancelled ? ', cancelled by price since' : ''}. Weekly ${bias.w1.dir}${bias.w1.dir !== 'neutral' ? ` (${sig(bias.w1)})` : ''}. Higher-timeframe call: ${bias.htf}.` : 'No bias yet.';
  const lv = (r: Any, word: string) => (r ? `${word} ${px(r.price)} (${r.touches} touch${r.touches === 1 ? '' : 'es'}, ${r.tier.replace(/-/g, ' ')}, broke ${r.barsSinceBreak} bars ago, ${f(r.distAtr * (atr.h1 || 0) / a1, 2)} daily ATR away${r.tradable ? '' : ', chopped'})` : `no ${word.toLowerCase()} marked`);
  const L3 = `${lv(below, 'Support')}; ${lv(above, 'Resistance')}. ${rows.length} flipped level${rows.length === 1 ? '' : 's'} within ${Math.round(100 * 0.2)}% of price.`;
  const fresh = [...(sfp.h1 ?? []), ...(sfp.d1 ?? [])].filter((s: Any) => s.valid && s.ageBars <= 3);
  const L4 = lvl?.found ? `Price broke ${focusRow!.dir === 'long' ? 'up through' : 'down through'} ${px(focusRow!.price)} ${lvl.barsSinceBreak} bars ago and has moved ${f(rt?.excursionAtr ?? 0, 2)} ATR clear (space ${rt?.spaceOk ? 'ok' : 'not yet'}); ${rt?.retested ? `retested after ${rt.barsBreakToRetest} bars (${rt.shape})` : 'not retested yet'}, momentum ${rt?.momentum ?? 'none'}.${fresh.length ? ` Fresh SFP: ${fresh.map((s: Any) => `${s.tf.toUpperCase()} ${s.dir}`).join(', ')}.` : ''}` : `No flipped level close enough to read the H1 structure.${fresh.length ? ` Fresh SFP: ${fresh.map((s: Any) => `${s.tf.toUpperCase()} ${s.dir}`).join(', ')}.` : ''}`;
  const p = ent?.primary;
  const L5 = p && focusRow ? `${p.go ? 'GO' : 'Wait'}: ${focusRow.dir === 'long' ? 'bid' : 'offer'} ${px(focusRow.price)}${stop?.stop ? `, stop ${px(stop.stop)}` : ''}${tgt?.chosen ? `, target ${px(tgt.chosen.price)} (${f(tgt.chosen.rr, 1)}R, ${tgt.chosen.kind})` : ''}.${p.go ? '' : ` Blocked by ${flow.firstBlocker ? `step ${flow.firstBlocker.step} (${flow.firstBlocker.label})` : p.vetoes[0] ?? 'a veto'}.`} ${cf ? (cf.blind ? 'Tier: traded blind, no confluence needed.' : `Confluences ${cf.score} of ${cf.needed} needed.`) : ''}` : 'No candidate order: no tradable flipped level near price.';
  const layers: Layer[] = [
    { id: 'regime', title: 'L1 Regime (W1/D1)', text: L1 }, { id: 'bias', title: 'L2 Daily bias', text: L2 }, { id: 'location', title: 'L3 Location', text: L3 },
    { id: 'structure', title: 'L4 H1 structure', text: L4 }, { id: 'decision', title: 'L5 Decision', text: L5 },
  ];
  const headline = `${reg ? `${reg.trend} market, ${reg.volState} volatility` : 'warming up'}; bias ${bias?.d1.dir ?? 'n/a'}; ${focusRow ? `watching ${px(focusRow.price)} (${focusRow.dir})` : 'no level in play'}${p?.go ? ' - GO' : ''}`;

  // ---- salience: how much each node is shaping this reading ----
  const sal: Record<string, number> = {};
  for (const n of nodes) sal[n.id] = n.evaluate ? 0.08 : 0;
  sal.atr = 0.35;
  if (reg) sal.regime = clip(0.2 + Math.abs(reg.slopeAtr) / 4 + Math.abs(reg.volPct - 0.5) / 2);
  sal['swing-depth'] = 0.3;
  sal.sfp = fresh.length ? 1 : (sfp.h1?.length || sfp.d1?.length) ? 0.35 : 0.1;
  sal['daily-bias'] = bias ? (bias.d1.fresh ? 1 : bias.d1.dir !== 'neutral' ? 0.6 : 0.15) : 0.1;
  sal['atr-gate'] = gate ? (gate.passedUp || gate.passedDown ? 1 : clip(Math.max(gate.fromHighAtr, gate.fromLowAtr) / 1.2)) : 0.1;
  sal['level-scan'] = clip(0.2 + rows.filter((r) => r.distAtr * (atr.h1 || 0) / a1 <= 1).length / 4);
  sal['market-side'] = focusRow ? 0.5 : 0.1;
  sal.level = lvl?.found ? clip(0.4 + 0.6 * Math.max(0, 1 - (focusRow?.distAtr ?? 9) * (atr.h1 || 0) / a1)) : 0.1;
  sal.retest = rt ? (rt.retested && rt.spaceOk ? 1 : rt.spaceOk ? 0.7 : rt.retested ? 0.5 : 0.25) : 0.1;
  sal.confluence = cf ? clip(cf.score / Math.max(1, cf.needed)) : 0.1;
  sal.news = news?.redNow ? 1 : 0.08;
  sal['tuesday-rto'] = tue?.valid ? 1 : 0.08;
  sal.context = cx?.fridayLate ? 0.7 : 0.2;
  sal.stop = stop?.ok ? 0.6 : 0.15; sal.target = tgt?.chosen ? clip(0.3 + tgt.chosen.rr / 4) : 0.15;
  sal.entry = p ? (p.go ? 1 : 0.55) : 0.1; sal.risk = risk?.ok ? 0.4 : 0.1; sal.management = 0.15;
  if (flow.firstBlocker) sal[flow.firstBlocker.node] = Math.max(sal[flow.firstBlocker.node] ?? 0, 0.9);

  return {
    asOf: T, price, layers, headline, salience: sal, status: statuses(R),
    focus: focusRow ? { price: focusRow.price, dir: focusRow.dir, touches: focusRow.touches, tier: focusRow.tier, distAtr: focusRow.distAtr * (atr.h1 || 0) / a1 } : null,
    firstBlocker: flow.firstBlocker ? { node: flow.firstBlocker.node, label: flow.firstBlocker.label } : null,
    keys: { trend: reg?.trend ?? 'n/a', vol: reg?.volState ?? 'n/a', bias: bias?.d1.dir ?? 'n/a', nLevels: rows.length, go: !!p?.go, spaceOk: rt ? !!rt.spaceOk : null },
    levels: rows.map((r) => ({ p: r.price, d: r.dir === 'long' ? 1 : -1, n: r.touches, tier: r.tier, bsb: r.barsSinceBreak, ok: r.tradable })),
    atrD1: atr.d1 ?? 0,
    bias: { dir: bias?.d1.dir ?? 'neutral', fresh: !!bias?.d1.fresh, kinds: (bias?.d1.signals ?? []).filter((s: Any) => s.ageBars === 0).map((s: Any) => `${s.dir} ${s.kind}`) },
    sfp: fresh.map((s: Any) => ({ tf: s.tf, dir: s.dir })),
  };
}

/** The graph's shape for drawing: every node with its stage and step, every wire. */
export function topology() {
  return {
    nodes: nodes.map((n) => ({ id: n.id, title: n.title, stage: n.stage, step: n.step?.n ?? null, computed: !!n.evaluate, decideAt: n.decideAt })),
    edges: graph.edges.map((e) => ({ from: e.from.split('.')[0]!, to: e.to.split('.')[0]!, port: e.to.split('.')[1]! })),
  };
}
