import { readFileSync } from 'node:fs';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { graph, nodes, edges } from './kev.js';
import { H, makeFeed } from './core/series.js';
import { INSTRUMENTS } from './core/instruments.js';
import { encode } from './core/token.js';
import { SetupSchema, type Candle } from './core/types.js';

const feeds = new Map<string, { feed: ReturnType<typeof makeFeed>; last: number; cfg: (typeof INSTRUMENTS)[string] }>();
/** Loads (once) the H1 history and the market-specific configuration for a symbol. */
function market(symbol = 'SOLUSDT') {
  const cfg = INSTRUMENTS[symbol];
  if (!cfg) throw new Error(`unknown symbol ${symbol}; available: ${Object.keys(INSTRUMENTS).join(', ')}`);
  if (!feeds.has(symbol)) {
    const c: Candle[] = JSON.parse(readFileSync(new URL(`../data/${cfg.dataFile}`, import.meta.url), 'utf8'));
    feeds.set(symbol, { feed: makeFeed(c, { weekAnchor: cfg.weekAnchor }), last: c.at(-1)!.t, cfg });
  }
  return feeds.get(symbol)!;
}
const SYMBOL = z.enum(['SOLUSDT', 'XAUUSD']).optional().describe('market; default SOLUSDT');
const json = (x: unknown) => ({ content: [{ type: 'text' as const, text: JSON.stringify(x) }] });

const trim = (port: string, v: any): unknown => {
  if (port === 'swings') return Object.fromEntries(['h1', 'd1'].map((tf) => [tf, { n: v[tf].length, deep: v[tf].filter((s: any) => s.deep).length, latest: v[tf].slice(-3).map((s: any) => ({ side: s.side, price: s.price, depthAtr: +s.depthAtr.toFixed(2), deep: s.deep, swept: s.sweptAt !== null })) }]));
  if (port === 'sfp') return Object.fromEntries(['h1', 'd1'].map((tf) => [tf, v[tf].slice(0, 3)]));
  return v;
};

const defaultT = (symbol?: string) => market(symbol).last + H + 60_000;
const server = new McpServer({ name: 'kev-nodes', version: '0.2.0' });

server.registerTool('kev_nodes', {
  description: 'List every node in the graph: stage (Direction/Objection/Location/Execution/Management/Risk/Context), when it decides, what it needs, and how many rules it owns.',
  inputSchema: {},
}, async () => json(nodes.map((n) => ({
  id: n.id, title: n.title, stage: n.stage, decidesAt: n.decideAt, computed: !!n.evaluate, rules: n.rules.length, coded: n.rules.filter((r) => r.codable).length,
  needs: Object.keys(n.inputs), gives: Object.keys(n.outputs), summary: n.summary,
}))));

server.registerTool('kev_node', {
  description: 'Everything one node knows: its rules with their sources, status (stated / observed / inferred / backtest) and whether code tests them, plus its conflicts.',
  inputSchema: { id: z.string().describe('node id, e.g. "sfp" or "confluence"') },
}, async ({ id }) => {
  const n = graph.byId.get(id);
  return json(n ? { id: n.id, title: n.title, stage: n.stage, decidesAt: n.decideAt, summary: n.summary, inputs: Object.fromEntries(Object.keys(n.inputs).map((k) => [k, edges.find((e) => e.to === `${n.id}.${k}`)?.from])), outputs: Object.keys(n.outputs), rules: n.rules, conflicts: n.conflicts ?? [] } : { error: `no node "${id}"`, available: nodes.map((x) => x.id) });
});

server.registerTool('kev_rules', {
  description: 'Search all rules by words (e.g. "stop thrust", "news", "swerve"). Optionally filter by status or node. Returns each matching rule with its node and sources.',
  inputSchema: { query: z.string(), status: z.enum(['stated', 'observed', 'inferred', 'backtest-reported', 'backtest-rejected']).optional(), node: z.string().optional(), limit: z.number().int().positive().max(40).optional() },
}, async ({ query, status, node, limit }) => {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const hits = nodes.filter((n) => !node || n.id === node).flatMap((n) => n.rules.map((r) => ({ node: n.id, r })))
    .filter(({ r }) => !status || r.status === status)
    .map(({ node: id, r }) => ({ id, r, score: words.filter((w) => `${r.id} ${r.text} ${r.note ?? ''}`.toLowerCase().includes(w)).length }))
    .filter((x) => x.score > 0).sort((a, b) => b.score - a.score).slice(0, limit ?? 12);
  return json(hits.map(({ r }) => ({ id: r.id, text: r.text, kind: r.kind, status: r.status, coded: r.codable, note: r.note, sources: r.sources })));
});

server.registerTool('kev_levels', {
  description: 'Step 1 of the method: mark the levels on SOL/USDT or XAU/USD. Clusters the swing highs/lows near price, keeps the ones price has already broken, and for each counts the swing touches in its previous role ("opposite last"), the tier (3+ blind, 2 needs one confluence, 1 needs two), the last high/low before the breakout, and whether it is on the buy or sell side of the market.',
  inputSchema: { symbol: SYMBOL, time: z.string().optional().describe('ISO decision time; default is just after the latest closed hour'), side: z.enum(['below', 'above']).optional().describe('only levels below price (buys) or above (sells)'), limit: z.number().int().positive().max(30).optional() },
}, async ({ symbol, time, side, limit }) => {
  const m = market(symbol), T = time ? Date.parse(time) : defaultT(symbol);
  const res = graph.run(m.feed, T, SetupSchema.parse({ dir: 'long' }), { instrument: m.cfg.instrument });
  const scan = res.states['level-scan']?.out.levels as { price: number; rows: any[] } | undefined;
  if (!scan) return json({ error: res.states['level-scan']?.error ?? 'scanner did not run' });
  const rows = scan.rows.filter((r) => !side || r.side === side).slice(0, limit ?? 12).map((r) => ({
    level: +r.price.toFixed(3), trade: r.dir === 'long' ? 'BUY (level is below price)' : 'SELL (level is above price)', distAtrH1: +r.distAtr.toFixed(1),
    swingTouches: r.touches, tier: r.tier, orderAt: r.lxpbPrice === null ? null : +r.lxpbPrice.toFixed(3), barsSinceBreak: r.barsSinceBreak, touchesSpanBars: r.spanBars, tradable: r.tradable, chopCrossings: r.chopCrossings,
  }));
  return json({ asOf: new Date(T).toISOString(), symbol: m.cfg.instrument.symbol, lastClose: scan.price, levels: rows, howToRead: 'swingTouches counts only reactions in the level\'s PREVIOUS role. tier says how many extra confluences (daily bias, fib 38/50/61, at-or-past daily ATR) the method asks for. orderAt is the last high (low) before the breakout.', caveat: 'Level clustering is the builder\'s; the trader marks levels by eye. Method written for forex; untested on crypto.' });
});

server.registerTool('kev_evaluate', {
  description: 'Run the whole graph on SOL/USDT or XAU/USD for a proposed trade, as of a moment (default: just after the latest closed hour). Returns a go / no-go for the chosen entry style with every veto, every rule check that passed or failed, the state token, and the nodes\' outputs. Only information that existed at that moment is used.',
  inputSchema: {
    symbol: SYMBOL,
    dir: z.enum(['long', 'short']),
    style: z.enum(['sr-flip', 'sfp', 'tuesday-rto']).optional(),
    levelPrice: z.number().optional().describe('the S/R level the trade is built on (required for sr-flip)'),
    entry: z.number().optional(), stop: z.number().optional(), target: z.number().optional(),
    placedAt: z.string().optional().describe('ISO time the order was placed (enables swerve checks)'),
    fillAt: z.string().optional().describe('ISO time of the fill (enables management checks)'),
    time: z.string().optional().describe('ISO decision time; default is just after the latest closed hour'),
    price: z.number().optional().describe('live price at that moment; default is the last close'),
    redNews: z.array(z.string()).optional().describe('ISO times of red-folder releases'),
    riskPct: z.number().optional(), equity: z.number().optional(), highProbability: z.boolean().optional(),
    detail: z.boolean().optional().describe('include every node output (large)'),
  },
}, async (a) => {
  const m = market(a.symbol), T = a.time ? Date.parse(a.time) : defaultT(a.symbol);
  let levelChosen: unknown = undefined, levelPrice = a.levelPrice;
  if (levelPrice === undefined && (a.style ?? 'sr-flip') === 'sr-flip') {
    const pre = graph.run(m.feed, T, SetupSchema.parse({ dir: a.dir }), { instrument: m.cfg.instrument, price: a.price });
    const rows = ((pre.states['level-scan']?.out.levels as { rows: any[] } | undefined)?.rows ?? []).filter((r) => r.dir === a.dir && r.tier !== 'none' && r.tradable);
    const best = rows[0];
    if (best) { levelPrice = best.price; levelChosen = { price: best.price, touches: best.touches, tier: best.tier, note: 'no levelPrice given: used the nearest tradable level on the right side of the market' }; }
    else levelChosen = { none: true, note: `no tradable level on the ${a.dir === 'long' ? 'below' : 'above'}-market side` };
  }
  const setup = SetupSchema.parse({ dir: a.dir, style: a.style, levelPrice, entry: a.entry, stop: a.stop, target: a.target, placedAt: a.placedAt ? Date.parse(a.placedAt) : undefined, fillAt: a.fillAt ? Date.parse(a.fillAt) : undefined, riskPct: a.riskPct, equity: a.equity, highProbability: a.highProbability });
  const res = graph.run(m.feed, T, setup, { instrument: m.cfg.instrument, price: a.price, calendar: (a.redNews ?? []).map((t) => ({ t: Date.parse(t), impact: 'red' as const, label: 'red' })) });
  const checks = res.order.flatMap((id) => res.states[id]!.checks.map((c) => ({ node: id, ...c })));
  const entry = (res.states.entry?.out.entry as { primary: { go: boolean; vetoes: string[]; notes: string[]; price: number | null } | null } | undefined)?.primary;
  const tok = encode(res), flow = graph.flow(res);
  return json({
    asOf: new Date(T).toISOString(), instrument: m.cfg.instrument.symbol, style: setup.style, levelChosen,
    firstBlocker: flow.firstBlocker && { step: flow.firstBlocker.step, label: flow.firstBlocker.label, why: flow.firstBlocker.notes.filter((n) => !n.includes(': ok')).slice(0, 3) },
    flowInOrder: flow.steps.map((x) => ({ step: x.step, label: x.label, status: x.status })),
    verdict: entry ? { go: entry.go, orderPrice: entry.price, vetoes: entry.vetoes, notes: entry.notes } : null,
    failedChecks: checks.filter((c) => c.pass === false), judgementCalls: checks.filter((c) => c.pass === null), passedChecks: checks.filter((c) => c.pass === true).length,
    token: tok,
    problems: res.order.filter((id) => res.states[id]!.error || res.states[id]!.skipped).map((id) => ({ node: id, error: res.states[id]!.error, skipped: res.states[id]!.skipped })),
    nodes: a.detail ? Object.fromEntries(res.order.filter((id) => res.states[id]!.ran).map((id) => [id, Object.fromEntries(Object.entries(res.states[id]!.out).map(([p, v]) => [p, trim(p, v)]))])) : undefined,
    caveats: ['Rules are a third party\'s notes of a discretionary FX/index trader. Backtests of this graph on SOL and gold (H4, H1, M30) found no net edge after costs; see README.', 'Daily bias reads closed bars only.'],
  });
});

server.registerTool('kev_graph', {
  description: 'The wiring: evaluation order, every wire, and what each node is allowed to know and when.',
  inputSchema: {},
}, async () => json({ order: graph.order(), phases: ['prior_bars', 'bar_open', 'fill_time', 'bar_close', 'post_fill'], wires: edges.map((e) => `${e.from} -> ${e.to}`), problems: graph.validate() }));

await server.connect(new StdioServerTransport());
