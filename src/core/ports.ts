import { z } from 'zod';

/** Typed contracts between nodes. They live in core so nodes can agree on shapes without importing each other. */
const dir = z.enum(['long', 'short']);
const dirN = z.enum(['long', 'short', 'neutral']);

export const AtrPort = z.object({ d1: z.number(), h1: z.number(), h1_24: z.number() });

export const SwingInfo = z.object({
  tf: z.enum(['h1', 'd1']), i: z.number(), t: z.number(), side: z.enum(['high', 'low']), price: z.number(),
  swingIn: z.number(), swingOut: z.number(), depth: z.number(), depthAtr: z.number(), deep: z.boolean(),
  sweptAt: z.number().nullable(),
});
export const SwingsPort = z.object({ h1: z.array(SwingInfo), d1: z.array(SwingInfo) });

export const SfpSignal = z.object({
  tf: z.enum(['h1', 'd1']), dir, swingPrice: z.number(), swingT: z.number(), sweepT: z.number(), sweepIdx: z.number(),
  sweepExtreme: z.number(), closePrice: z.number(), closeLoc: z.number(),
  tooSmall: z.boolean(), strongAgainst: z.boolean(), deep: z.boolean(),
  visitNo: z.number(), needsDivergence: z.boolean(), ageBars: z.number(), invalidated: z.boolean(),
  stopAtr: z.number(), stopAdvanced: z.number(), entryMode: z.enum(['immediate', 'wait-half-to-stop']), valid: z.boolean(),
});
export const SfpPort = z.object({ h1: z.array(SfpSignal), d1: z.array(SfpSignal) });

export const BiasSignal = z.object({ kind: z.enum(['pin', 'engulfing', 'sfp', 'idf']), dir, ageBars: z.number() });
export const BiasTf = z.object({ dir: dirN, signals: z.array(BiasSignal), cancelled: z.boolean(), ageBars: z.number().nullable(), fresh: z.boolean() });
export const BiasPort = z.object({ d1: BiasTf, w1: BiasTf, htf: dirN });

export const AtrGatePort = z.object({
  movedAtr: z.number(), fromHighAtr: z.number(), fromLowAtr: z.number(), passedUp: z.boolean(), passedDown: z.boolean(),
  blocks: z.object({ long: z.boolean(), short: z.boolean() }),
  supports: z.object({ long: z.boolean(), short: z.boolean() }),
});

export const LevelPort = z.object({
  found: z.boolean(), role: z.enum(['support', 'resistance']).nullable(),
  center: z.number(), zoneLo: z.number(), zoneHi: z.number(),
  breakIdx: z.number().nullable(), breakT: z.number().nullable(), barsSinceBreak: z.number(),
  touches: z.number(), lastTouchT: z.number().nullable(), lxpbPrice: z.number().nullable(), spanBars: z.number(),
  chopCrossings: z.number(), disrespected: z.boolean(), tradable: z.boolean(),
});

export const RetestPort = z.object({
  retested: z.boolean(), barsBreakToRetest: z.number().nullable(), excursionAtr: z.number(), spaceOk: z.boolean(),
  shape: z.enum(['rounded', 'sharp', 'none']), approachAtr: z.number(), momentum: z.enum(['hard', 'drift', 'none']), tooSoon: z.boolean(), rallyAway: z.boolean().nullable(), swerves: z.number().nullable(), cancelOrder: z.boolean(),
});

export const ConfluencePort = z.object({
  touches: z.number(), factors: z.object({ bias: z.boolean(), atr: z.boolean(), fib: z.boolean() }), fibDetail: z.string(),
  score: z.number(), needed: z.number(), meets: z.boolean(), blind: z.boolean(),
});

export const NewsPort = z.object({
  redNow: z.boolean(), minutesToNextRed: z.number().nullable(), minutesSinceLastRed: z.number().nullable(),
  noNewFills: z.boolean(), action: z.enum(['ok', 'pull-limit', 'keep-limit', 'reinsert-later']), holdIfInTrade: z.boolean(),
});

export const TuesdayPort = z.object({
  valid: z.boolean(), dir: dir.nullable(), entry: z.number().nullable(), stop: z.number().nullable(), target: z.number().nullable(),
  rr: z.number().nullable(), reasons: z.array(z.string()),
});

export const ContextPort = z.object({
  weekday: z.number(), hourUtc: z.number(), session: z.enum(['asian', 'london', 'ny', 'off']), fridayLate: z.boolean(),
  flattenBy: z.enum(['none', 'end-of-week', 'end-of-day']), mode: z.enum(['with-bias', 'no-bias']), conservativeTargets: z.boolean(),
});

export const StopPort = z.object({
  stop: z.number().nullable(), suggested: z.number().nullable(), method: z.string(), shifted: z.boolean(), riskDist: z.number(), ok: z.boolean(), squeezeLevel: z.boolean(),
});

export const TargetCandidate = z.object({ kind: z.enum(['htf', 'fta', 'recent', 'range-opposite']), price: z.number(), rr: z.number(), viable: z.boolean() });
export const TargetPort = z.object({ candidates: z.array(TargetCandidate), chosen: TargetCandidate.nullable(), minRr: z.number() });

export const OrderCandidate = z.object({
  style: z.enum(['sr-flip', 'sfp', 'tuesday-rto']), dir, price: z.number().nullable(), go: z.boolean(),
  vetoes: z.array(z.string()), notes: z.array(z.string()),
});
export const EntryPort = z.object({ candidates: z.array(OrderCandidate), primary: OrderCandidate.nullable() });

export const RiskPort = z.object({ riskAmount: z.number(), riskDistance: z.number(), size: z.number(), ok: z.boolean() });

export const ManagementPort = z.object({
  evolvingR: z.number().nullable(), canAdd: z.boolean(),
  actions: z.array(z.object({ action: z.enum(['hold', 'close', 'trail', 'cancel-order', 'tighten']), reason: z.string(), rule: z.string() })),
});

export type Atr = z.infer<typeof AtrPort>;
export type SwingInfoT = z.infer<typeof SwingInfo>;
export type SfpSignalT = z.infer<typeof SfpSignal>;
export type BiasTfT = z.infer<typeof BiasTf>;

export const MarketSidePort = z.object({
  relation: z.enum(['below', 'above', 'unknown']), requiredDir: dirN, valid: z.boolean(), detail: z.string(),
});

export const ScanRow = z.object({
  price: z.number(), dir, side: z.enum(['below', 'above']), distAtr: z.number(), touches: z.number(),
  tier: z.enum(['blind', 'needs-one-confluence', 'needs-two-confluences', 'none']), lxpbPrice: z.number().nullable(),
  barsSinceBreak: z.number(), spanBars: z.number(), tradable: z.boolean(), chopCrossings: z.number(),
});
export const LevelScanPort = z.object({ price: z.number(), rows: z.array(ScanRow) });

export const RegimePort = z.object({
  trend: z.enum(['up', 'down', 'range']), slopeAtr: z.number(), weekTrend: z.enum(['up', 'down', 'range']),
  rangePos: z.number(), volPct: z.number(), volState: z.enum(['low', 'normal', 'high']), atrPct: z.number(), drawdown: z.number(), days: z.number(),
});
