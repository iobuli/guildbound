import type { AnyNode } from './node.js';
import { rank, type Check, type Instrument, type NewsEvent, type Setup } from './types.js';
import { viewAt, type Feed } from './series.js';

export interface Edge { from: string; to: string } // 'node.port' -> 'node.port'

export interface NodeState {
  ran: boolean; // false for rule-card nodes without evaluate()
  skipped?: string;
  error?: string;
  out: Record<string, unknown>;
  checks: Check[];
}
export interface RunResult { order: string[]; states: Record<string, NodeState>; asOf: number }
export interface FlowStep { step: number; label: string; node: string; status: 'pass' | 'fail' | 'judgement' | 'info' | 'not-run'; notes: string[] }

const split = (s: string) => { const k = s.indexOf('.'); return [s.slice(0, k), s.slice(k + 1)] as const; };

export class Graph {
  readonly byId = new Map<string, AnyNode>();
  constructor(readonly nodes: AnyNode[], readonly edges: Edge[]) {
    for (const n of nodes) this.byId.set(n.id, n);
  }

  /** Structural + causal validation. Returns every problem, not just the first. */
  validate(): string[] {
    const errs: string[] = [];
    if (this.byId.size !== this.nodes.length) errs.push('duplicate node ids');
    const incoming = new Map<string, number>();
    for (const e of this.edges) {
      const [fn, fp] = split(e.from), [tn, tp] = split(e.to);
      const f = this.byId.get(fn), t = this.byId.get(tn);
      if (!f) { errs.push(`edge ${e.from}: unknown node ${fn}`); continue; }
      if (!t) { errs.push(`edge ${e.to}: unknown node ${tn}`); continue; }
      if (!(fp in f.outputs)) errs.push(`edge ${e.from}: ${fn} has no output port ${fp}`);
      if (!(tp in t.inputs)) errs.push(`edge ${e.to}: ${tn} has no input port ${tp}`);
      incoming.set(e.to, (incoming.get(e.to) ?? 0) + 1);
      if (rank(f.decideAt) > rank(t.decideAt))
        errs.push(`CAUSALITY: ${e.from} is only knowable at ${f.decideAt}, but ${tn} decides at ${t.decideAt}`);
    }
    for (const n of this.nodes)
      for (const p of Object.keys(n.inputs)) {
        const c = incoming.get(`${n.id}.${p}`) ?? 0;
        if (c !== 1) errs.push(`input ${n.id}.${p} has ${c} wires (needs exactly 1)`);
      }
    const ruleIds = new Set<string>();
    for (const n of this.nodes)
      for (const r of n.rules) {
        if (!r.id.startsWith(`${n.id}.`)) errs.push(`rule ${r.id} is not prefixed by its node ${n.id}`);
        if (ruleIds.has(r.id)) errs.push(`duplicate rule id ${r.id}`);
        ruleIds.add(r.id);
        if (!r.sources.length) errs.push(`rule ${r.id} has no source`);
      }
    for (const n of this.nodes)
      for (const c of n.conflicts ?? []) for (const id of [c.a, c.b]) if (!ruleIds.has(id)) errs.push(`conflict references unknown rule ${id}`);
    try { this.order(); } catch (e) { errs.push((e as Error).message); }
    return errs;
  }

  order(): string[] {
    const indeg = new Map<string, number>(this.nodes.map((n) => [n.id, 0]));
    for (const e of this.edges) indeg.set(split(e.to)[0], (indeg.get(split(e.to)[0]) ?? 0) + 1);
    const stepOf = (id: string) => this.byId.get(id)?.step?.n ?? 99;
    const q = this.nodes.filter((n) => indeg.get(n.id) === 0).map((n) => n.id);
    const out: string[] = [];
    while (q.length) {
      // among nodes that are ready, take the one that comes earliest in the trader's order of operations
      q.sort((a, b) => stepOf(a) - stepOf(b) || a.localeCompare(b));
      const id = q.shift()!;
      out.push(id);
      for (const e of this.edges) if (split(e.from)[0] === id) {
        const t = split(e.to)[0];
        indeg.set(t, indeg.get(t)! - 1);
        if (indeg.get(t) === 0) q.push(t);
      }
    }
    if (out.length !== this.nodes.length) throw new Error(`cycle among: ${this.nodes.filter((n) => !out.includes(n.id)).map((n) => n.id).join(', ')}`);
    return out;
  }

  /** The run laid out in the trader's order of operations, with the first step that blocks the trade. */
  flow(res: RunResult): { steps: FlowStep[]; firstBlocker: FlowStep | null } {
    const steps = this.nodes.filter((n) => n.step && n.step.n >= 0.5).sort((a, b) => a.step!.n - b.step!.n).map((n): FlowStep => {
      const st = res.states[n.id];
      if (!st || !st.ran) return { step: n.step!.n, label: n.step!.label, node: n.id, status: 'not-run', notes: [st?.error ?? st?.skipped ?? 'did not run'] };
      const fails = st.checks.filter((c) => c.pass === false), judge = st.checks.filter((c) => c.pass === null);
      const status = fails.length ? 'fail' : judge.length ? 'judgement' : st.checks.length ? 'pass' : 'info';
      return { step: n.step!.n, label: n.step!.label, node: n.id, status, notes: [...fails, ...judge, ...st.checks.filter((c) => c.pass === true)].map((c) => `${c.rule}: ${c.detail}`) };
    });
    return { steps, firstBlocker: steps.find((s) => s.status === 'fail') ?? null };
  }

  run(feed: Feed, T: number, setup: Setup, ctx: { instrument: Instrument; price?: number; calendar?: NewsEvent[] }): RunResult {
    const order = this.order();
    const states: Record<string, NodeState> = {};
    for (const id of order) {
      const node = this.byId.get(id)!;
      const st: NodeState = { ran: false, out: {}, checks: [] };
      states[id] = st;
      if (!node.evaluate) continue;
      const inp: Record<string, unknown> = {};
      let blocked: string | undefined;
      for (const e of this.edges) {
        const [tn, tp] = split(e.to);
        if (tn !== id) continue;
        const [fn, fp] = split(e.from);
        const up = states[fn]!;
        if (!up.ran || up.error || up.skipped) { blocked = `upstream ${fn} did not run`; break; }
        const parsed = node.inputs[tp].safeParse(up.out[fp]);
        if (!parsed.success) { st.error = `input ${tp} from ${e.from} failed its contract: ${parsed.error.message.slice(0, 200)}`; break; }
        inp[tp] = parsed.data;
      }
      if (blocked) { st.skipped = blocked; continue; }
      if (st.error) continue;
      try {
        const view = viewAt(feed, T, node.decideAt, ctx.instrument, { price: ctx.price, calendar: ctx.calendar });
        const res = node.evaluate(inp as never, view, setup);
        const out: Record<string, unknown> = {};
        for (const [port, schema] of Object.entries(node.outputs)) {
          const parsed = (schema as { safeParse(x: unknown): { success: boolean; data?: unknown; error?: { message: string } } }).safeParse((res.out as Record<string, unknown>)[port]);
          if (!parsed.success) throw new Error(`output ${port} broke its contract: ${parsed.error?.message.slice(0, 200)}`);
          out[port] = parsed.data;
        }
        st.out = out; st.checks = res.checks ?? []; st.ran = true;
      } catch (e) {
        st.error = (e as Error).message;
      }
    }
    return { order, states, asOf: T };
  }
}
