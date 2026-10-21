// Browser entry for the inspector page. Same nodes and graph as the MCP server; the page only calls these functions.
import { graph, nodes } from '../kev.js';
import { P } from '../core/params.js';
import { makeFeed, MONDAY, SUNDAY_NOON, SOL, GOLD, H } from '../core/series.js';
import { analyzeLevel } from '../core/levelmath.js';
import { SetupSchema, type Candle } from '../core/types.js';

const DEFAULTS = JSON.parse(JSON.stringify(P)) as Record<string, unknown>;
/** The Trade Check page's config, expressed in this graph's parameters. */
const SPEC: Record<string, unknown> = { levelScaleD1: 1, zoneHalfAtr: 0.05, touchReactionAtr: 0.5, retestMinExcursionAtr: 0.5, touchClusterBars: 12, scanClusterAtr: 0.1 };
const MKT = { SOL: { instrument: SOL, weekAnchor: MONDAY }, GOLD: { instrument: GOLD, weekAnchor: SUNDAY_NOON } } as const;
type Mkt = keyof typeof MKT;

function preset(spec: boolean) { Object.assign(P, DEFAULTS, spec ? SPEC : {}); }

export interface RunIn { candles: Candle[]; T: number; mkt: Mkt; dir: 'long' | 'short'; spec: boolean; centre?: number; price?: number }

/** One decision: run the whole graph at time T on what is knowable then, and (optionally) pin one level for drawing. */
export function run(a: RunIn) {
  preset(a.spec);
  const feed = makeFeed(a.candles, { weekAnchor: MKT[a.mkt].weekAnchor });
  const setup = SetupSchema.parse({ dir: a.dir });
  const res = graph.run(feed, a.T, setup, { instrument: MKT[a.mkt].instrument, price: a.price });
  const flow = graph.flow(res);
  const atr = res.states.atr?.out.atr as { h1: number; d1: number } | undefined;
  const scan = (res.states['level-scan']?.out.levels as { price: number; rows: unknown[] } | undefined) ?? null;
  let level = null;
  if (a.centre !== undefined && atr) {
    const closed = a.candles.filter((b) => b.t + H <= a.T);
    level = analyzeLevel(closed.slice(-P.lookback.h1), a.centre, a.dir, P.levelScaleD1 ? atr.d1 : atr.h1);
  }
  const states = Object.fromEntries(Object.entries(res.states).map(([id, s]) => [id, { ran: s.ran, error: s.error, skipped: s.skipped, out: s.out, checks: s.checks }]));
  return { flow, atr, scan, level, states, order: res.order };
}

export function catalogue() {
  return nodes.map((n) => ({ id: n.id, title: n.title, stage: n.stage, summary: n.summary, decideAt: n.decideAt, step: n.step ?? null, inputs: Object.keys(n.inputs), outputs: Object.keys(n.outputs),
    rules: n.rules.map((r) => ({ id: r.id, text: r.text, kind: r.kind, status: r.status, codable: r.codable, note: r.note ?? null, sources: r.sources.map((s) => `${s.kind}: ${s.ref}`) })) }));
}
export const params = (spec: boolean) => { preset(spec); return JSON.parse(JSON.stringify(P)); };
(globalThis as unknown as { Kev: unknown }).Kev = { run, catalogue, params };
