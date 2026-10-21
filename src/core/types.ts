import { z } from 'zod';

export interface Candle { t: number; o: number; h: number; l: number; c: number; v: number }
export type Dir = 'long' | 'short';

/**
 * When a quantity can first be computed. This is the whole causality model:
 *   prior_bars  from fully closed bars only
 *   bar_open    plus the open of the bar now forming
 *   fill_time   plus the live traded price at the moment an order fills (never the forming bar's H/L/C)
 *   bar_close   plus the bar that closes at the end of the fill bar
 *   post_fill   anything evaluated after the fill (management)
 */
export const KNOWABLE = ['prior_bars', 'bar_open', 'fill_time', 'bar_close', 'post_fill'] as const;
export type Knowable = (typeof KNOWABLE)[number];
export const rank = (k: Knowable) => KNOWABLE.indexOf(k);

export type SourceKind = 'transcript' | 'webinar' | 'lesson' | 'journal' | 'tweet' | 'backtest';
export interface SourceRef { kind: SourceKind; ref: string; date?: string }

/** stated = said as a rule; observed = repeatedly seen in trades; inferred = a note-taker's or builder's reading */
export type Status = 'stated' | 'observed' | 'inferred' | 'backtest-reported' | 'backtest-rejected';
export type RuleKind = 'hard' | 'preference' | 'heuristic';

export interface Rule {
  id: string; // `${nodeId}.${slug}`
  text: string;
  kind: RuleKind;
  status: Status;
  /** true when this node's evaluate() actually tests it */
  codable: boolean;
  sources: SourceRef[];
  note?: string;
}
export interface Conflict { a: string; b: string; note: string }
export interface Check { rule: string; pass: boolean | null; detail: string } // null = judgement call / not computable here

export type Stage = 'D' | 'O' | 'L' | 'E' | 'M' | 'R' | 'X'; // Direction, Objection, Location, Execution, Management, Risk, Context

export type InstrumentClass = 'fx-major' | 'fx-cross' | 'index' | 'bund' | 'metal' | 'crypto';
export interface Instrument { symbol: string; cls: InstrumentClass; pip: number }

export interface NewsEvent { t: number; impact: 'red' | 'orange' | 'yellow'; label: string }
export interface Now { t: number; price: number; dayOpen: number; weekOpen: number }

/** Everything a node may look at. Bars are CLOSED bars only (see Knowable); `now` is null for prior_bars nodes. */
export interface View {
  /** length of one base bar in ms (3.6e6 for H1) */
  period: number;
  /** start of the trading week in ms-of-epoch terms (see series.weekStart) */
  weekAnchor: number;
  asOf: number;
  phase: Knowable;
  instrument: Instrument;
  h1: Candle[];
  d1: Candle[];
  w1: Candle[];
  now: Now | null;
  calendar: NewsEvent[];
}

/** The question being asked of the graph: a proposed trade. Optional fields are used when present. */
export const SetupSchema = z.object({
  dir: z.enum(['long', 'short']),
  style: z.enum(['sr-flip', 'sfp', 'tuesday-rto']).default('sr-flip'),
  levelPrice: z.number().optional(),
  entry: z.number().optional(),
  stop: z.number().optional(),
  target: z.number().optional(),
  placedAt: z.number().optional(), // ms when the order was placed
  fillAt: z.number().optional(), // ms of the fill
  riskPct: z.number().default(0.5),
  equity: z.number().default(100000),
  highProbability: z.boolean().default(false),
});
export type Setup = z.infer<typeof SetupSchema>;
