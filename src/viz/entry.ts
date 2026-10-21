// Browser entry for the inspector page. Same nodes, graph, sessions and presets as the CLI and the MCP server; any ingested market.
import { graph, nodes } from '../kev.js';
import { P } from '../core/params.js';
import { applyPreset, PRESETS } from '../core/presets.js';
import { makeFeed, H } from '../core/series.js';
import type { SessionSpec } from '../core/session.js';
import { analyzeLevel } from '../core/levelmath.js';
import { SetupSchema, type Candle, type Instrument } from '../core/types.js';
import { interpret as readMoment, topology } from '../interpret.js';

export interface RunIn { candles: Candle[]; T: number; session: SessionSpec; instrument: Instrument; dir: 'long' | 'short'; preset: string; centre?: number; price?: number }

/** One decision: run the whole graph at time T on what is knowable then, and (optionally) pin one level for drawing. */
export function run(a: RunIn) {
  applyPreset(a.preset);
  const feed = makeFeed(a.candles, { session: a.session });
  const res = graph.run(feed, a.T, SetupSchema.parse({ dir: a.dir }), { instrument: a.instrument, price: a.price });
  const flow = graph.flow(res);
  const atr = res.states.atr?.out.atr as { h1: number; d1: number } | undefined;
  const scan = (res.states['level-scan']?.out.levels as { price: number; rows: unknown[] } | undefined) ?? null;
  let level = null;
  if (a.centre !== undefined && atr) {
    const closed = a.candles.filter((b) => b.t + H <= a.T);
    level = analyzeLevel(closed.slice(-P.lookback.h1), a.centre, a.dir, P.levelScaleD1 ? atr.d1 : atr.h1);
  }
  const s = feed.session, day = { start: s.dayStart(a.T), weekday: s.weekday(a.T), weekStart: s.weekStart(a.T), londonHour: s.local(a.T).hour };
  const states = Object.fromEntries(Object.entries(res.states).map(([id, st]) => [id, { ran: st.ran, error: st.error, skipped: st.skipped, out: st.out, checks: st.checks }]));
  return { flow, atr, scan, level, states, order: res.order, day };
}

export function catalogue() {
  return nodes.map((n) => ({ id: n.id, title: n.title, stage: n.stage, summary: n.summary, decideAt: n.decideAt, step: n.step ?? null, inputs: Object.keys(n.inputs), outputs: Object.keys(n.outputs),
    rules: n.rules.map((r) => ({ id: r.id, text: r.text, kind: r.kind, status: r.status, codable: r.codable, note: r.note ?? null, sources: r.sources.map((x) => `${x.kind}: ${x.ref}`) })) }));
}
export const presets = () => Object.entries(PRESETS).map(([name, p]) => ({ name, note: p.note }));
/** kev's layered reading at T plus per-node status and salience (what lights the graph). */
export function interpret(a: { candles: Candle[]; T: number; session: SessionSpec; instrument: Instrument; preset: string; price?: number }) {
  applyPreset(a.preset);
  return readMoment(makeFeed(a.candles, { session: a.session }), a.T, { instrument: a.instrument, price: a.price });
}
(globalThis as unknown as { Kev: unknown }).Kev = { run, catalogue, presets, interpret, topology };
