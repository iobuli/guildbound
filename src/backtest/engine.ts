// The causal harness, for any market. Every `step` hours the graph marks levels from closed bars; each becomes a resting limit order.
// When a later bar trades through the price the order fills, the graph is asked about the trade at that instant (bars before the fill
// bar plus the fill price only), and the trade is settled forward. Gating (GO, space, touches) is reported from fields every trade
// carries, so one run answers all of them. Placebos are random orders with the same side, distance, window and risk.
import { graph } from '../kev.js';
import { H, makeFeed, SOL, type Feed } from '../core/series.js';
import type { Session } from '../core/session.js';
import { SetupSchema, type Candle, type Instrument } from '../core/types.js';
import { settleOrder, type Ltf } from './settle.js';

/** step / maxHold / cooldown are in HOURS. */
export interface BtConfig { start: number; end: number; step: number; maxHold: number; cooldown: number; slipR: number; spreadBps: number; seed: number; period: number; session?: Session; inst: Instrument; holdB: number; heldExit: boolean; minTier: number; entryAtLevel: boolean; ltf?: Ltf }
export const TARGETS = [1, 2, 3, 4] as const;
export const KEYS = ['1', '2', '3', '4', 'fta'] as const;
export type RKey = (typeof KEYS)[number];
/** fixed-risk outcomes: stop S x prior-day ATR beyond the entry, target T x that risk (the user's pages use the daily ATR as the yardstick) */
export const FIXED = [[0.5, 1], [0.5, 2], [0.5, 4], [1, 2], [1, 4]] as const;
export const fixedKey = (s: number, t: number) => `S${s}_T${t}`;

export interface Row { price: number; dir: 'long' | 'short'; touches: number; tier: string; lxpbPrice: number | null; distAtr: number; barsSinceBreak: number }
export interface Scan { j: number; lastClose: number; atrH1: number; atrD1: number; rows: Row[] }

export interface Trade {
  i: number; scanJ: number; t: number; year: number; day: number;
  dir: 'long' | 'short'; tier: string; touches: number; level: number; px: number; fill: number; stop: number | null; risk: number; riskAtr: number; orderDist: number;
  atrD1: number; barsSinceBreak: number;
  ok: boolean; go: boolean; vetoes: string[]; checks: Record<string, boolean | null>; ftaR: number | null;
  /** price travelled clear of the level between the break and the fill, in ATR, from bars BEFORE the fill bar */
  farAtr: number; spaceOk: boolean;
  /** the fill bar closed back through the order price: known only after the fill bar ends (used to show the lookahead, never to trade) */
  closedThrough: boolean;
  r: Record<string, number>; x: Record<string, number>; resolvedBy: 'h1' | 'ltf'; placebo: boolean;
  /** the same outcomes settled with H1 bars only (conservative rules); present when finer bars were available */
  rc?: Record<string, number>;
}

function outcomes(c: readonly Candle[], j: number, long: boolean, fill: number, stop: number | null, fta: number | null, atrD1: number, C: BtConfig, heldLevel?: number) {
  const r: Record<string, number> = {}, x: Record<string, number> = {};
  let resolvedBy: 'h1' | 'ltf' = 'h1';
  const cost = (risk: number) => C.slipR + (C.spreadBps * 1e-4 * fill) / risk;
  if (stop !== null) {
    const risk = Math.abs(fill - stop), tg: Record<string, number> = {};
    for (const k of TARGETS) tg[String(k)] = long ? fill + k * risk : fill - k * risk;
    if (fta !== null) tg.fta = fta;
    const s = settleOrder(c, j, long, fill, stop, tg, C.holdB, cost(risk), { ltf: C.ltf, heldLevel });
    Object.assign(r, s.r); Object.assign(x, s.x); if (s.resolvedBy === 'ltf') resolvedBy = 'ltf';
  }
  if (atrD1 > 0) for (const S of [0.5, 1]) {
    const risk = S * atrD1, st = long ? fill - risk : fill + risk, tg: Record<string, number> = {};
    for (const [s, t] of FIXED) if (s === S) tg[fixedKey(s, t)] = long ? fill + t * risk : fill - t * risk;
    const z = settleOrder(c, j, long, fill, st, tg, C.holdB, cost(risk), { ltf: C.ltf, heldLevel });
    Object.assign(r, z.r); Object.assign(x, z.x); if (z.resolvedBy === 'ltf') resolvedBy = 'ltf';
  }
  return { r, x, resolvedBy };
}

/** Kept for the selftests: conservative settlement with structural targets 1-4R and FTA. */
export function settleFull(c: Candle[], j: number, long: boolean, entry: number, stop: number, fta: number | null, maxHold: number, slip: number, heldLevel?: number) {
  const risk = Math.abs(entry - stop), tg: Record<string, number> = {};
  for (const k of TARGETS) tg[String(k)] = long ? entry + k * risk : entry - k * risk;
  if (fta !== null) tg.fta = fta;
  const s = settleOrder(c, j, long, entry, stop, tg, maxHold, slip, { heldLevel });
  for (const k of Object.keys(s.r)) if (Number.isNaN(s.r[k]!)) { delete s.r[k]; delete s.x[k]; }
  return { r: s.r as Partial<Record<RKey, number>>, x: s.x as Partial<Record<RKey, number>> };
}
export function settle(c: Candle[], j: number, long: boolean, entry: number, stop: number, fta: number | null, maxHold: number, slip: number) {
  return settleFull(c, j, long, entry, stop, fta, maxHold, slip).r;
}

const flat = (res: ReturnType<typeof graph.run>) => {
  const m: Record<string, boolean | null> = {};
  for (const id of res.order) for (const ch of res.states[id]!.checks) m[ch.rule] = ch.pass === false || m[ch.rule] === false ? false : ch.pass === null || m[ch.rule] === null ? null : true;
  return m;
};

export function runBacktest(candles: Candle[], cfg: Partial<BtConfig> & { start: number; end: number }, onProgress?: (j: number) => void) {
  const C: BtConfig = { step: 3, maxHold: 120, cooldown: 24, slipR: 0.1, spreadBps: 0, seed: 7, period: H, inst: SOL, holdB: 0, heldExit: false, minTier: 1, entryAtLevel: false, ...cfg };
  const bars = (hours: number) => Math.max(1, Math.round((hours * H) / C.period));
  const stepB = bars(C.step), coolB = bars(C.cooldown);
  C.holdB = bars(C.maxHold);
  const c = candles, feed: Feed = makeFeed(c, { period: C.period, session: C.session });
  const trades: Trade[] = [], scans: Scan[] = [], cool = new Map<string, number>();
  let scan: Scan | null = null, last = -1e9;
  for (let j = Math.max(1, C.start); j < Math.min(C.end, c.length - C.holdB - 2); j++) {
    if (j - last >= stepB) {
      const res = graph.run(feed, c[j]!.t, SetupSchema.parse({ dir: 'long' }), { instrument: C.inst });
      const rows = ((res.states['level-scan']?.out.levels as { rows: Row[] } | undefined)?.rows ?? []).filter((r) => r.tier !== 'none' && r.lxpbPrice !== null);
      const atr = res.states.atr?.out.atr as { h1: number; d1: number } | undefined;
      scan = { j, lastClose: c[j - 1]!.c, atrH1: atr?.h1 ?? 0, atrD1: atr?.d1 ?? 0, rows };
      scans.push(scan); last = j;
      if (onProgress && scans.length % 500 === 0) onProgress(j);
    }
    const s = scan!, bar = c[j]!;
    for (const L of s.rows) {
      if (L.touches < C.minTier) continue;
      const long = L.dir === 'long', px = C.entryAtLevel ? L.price : L.lxpbPrice!;
      if (long ? !(px < s.lastClose) : !(px > s.lastClose)) continue; // a resting order on the right side at placement
      if (!(long ? bar.l <= px : bar.h >= px)) continue;
      const key = `${L.dir}|${Math.round(px / (0.5 * (s.atrH1 || 1)))}`;
      if ((cool.get(key) ?? -1) > j) continue;
      cool.set(key, j + coolB);
      const fill = long ? Math.min(px, bar.o) : Math.max(px, bar.o);
      const T = bar.t + C.period / 2;
      const res = graph.run(feed, T, SetupSchema.parse({ dir: L.dir, style: 'sr-flip', levelPrice: L.price, entry: fill }), { instrument: C.inst, price: fill });
      const entry = res.states.entry?.out.entry as { primary: { go: boolean; vetoes: string[] } | null } | undefined;
      const st = res.states.stop?.out.stop as { stop: number | null; ok: boolean; riskDist: number } | undefined;
      const tg = res.states.target?.out.target as { chosen: { price: number; rr: number } | null } | undefined;
      const rt = res.states.retest?.out.retest as { excursionAtr: number; spaceOk: boolean } | undefined;
      const ok = !!st && st.ok && st.stop !== null && st.riskDist > 0 && st.riskDist / fill < 0.15;
      const o = outcomes(c, j, long, fill, ok ? st!.stop : null, tg?.chosen?.price ?? null, s.atrD1, C, C.heldExit ? L.price : undefined);
      const oc = C.ltf ? outcomes(c, j, long, fill, ok ? st!.stop : null, tg?.chosen?.price ?? null, s.atrD1, { ...C, ltf: undefined }, C.heldExit ? L.price : undefined) : null;
      trades.push({
        i: j, scanJ: s.j, t: bar.t, year: new Date(bar.t).getUTCFullYear(), day: Math.floor(bar.t / 86_400_000), dir: L.dir, tier: L.tier, touches: L.touches, level: L.price, px, fill,
        stop: st?.stop ?? null, risk: st?.riskDist ?? 0, riskAtr: s.atrH1 > 0 && st ? st.riskDist / s.atrH1 : 0, orderDist: Math.abs(px - s.lastClose), atrD1: s.atrD1, barsSinceBreak: L.barsSinceBreak,
        ok, go: entry?.primary?.go ?? false, vetoes: entry?.primary?.vetoes ?? [], checks: flat(res), ftaR: tg?.chosen?.rr ?? null,
        farAtr: rt?.excursionAtr ?? 0, spaceOk: rt?.spaceOk ?? false, closedThrough: long ? bar.c < px : bar.c > px,
        r: o.r, x: o.x, resolvedBy: o.resolvedBy, placebo: false, ...(oc ? { rc: oc.r } : {}),
      });
    }
  }
  return { trades, scans, cfg: C };
}

/** One placebo per real fill: same side, distance from price x0.5-1.5, same fill window, same structural risk and the same fixed-ATR stops. */
export function placebos(c: Candle[], real: Trade[], scans: Map<number, Scan>, cfg: BtConfig, reps = 1): Trade[] {
  let s = (cfg.seed * 2654435761) >>> 0;
  const rand = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  const out: Trade[] = [];
  for (let rep = 0; rep < reps; rep++) for (const t of real) {
    const sc = scans.get(t.scanJ);
    if (!sc) continue;
    const long = t.dir === 'long', d = t.orderDist * (0.5 + rand()), px = long ? sc.lastClose - d : sc.lastClose + d;
    let j = -1;
    for (let k = sc.j; k <= t.i; k++) if (long ? c[k]!.l <= px : c[k]!.h >= px) { j = k; break; }
    if (j < 0) continue;
    const fill = long ? Math.min(px, c[j]!.o) : Math.max(px, c[j]!.o), risk = t.riskAtr * sc.atrH1, stop = t.ok && risk > 0 ? (long ? fill - risk : fill + risk) : null;
    const o = outcomes(c, j, long, fill, stop, null, sc.atrD1, cfg, cfg.heldExit ? px : undefined);
    const oc = cfg.ltf ? outcomes(c, j, long, fill, stop, null, sc.atrD1, { ...cfg, ltf: undefined }, cfg.heldExit ? px : undefined) : null;
    out.push({ ...t, ...(oc ? { rc: oc.r } : {}), i: j, t: c[j]!.t, year: new Date(c[j]!.t).getUTCFullYear(), day: Math.floor(c[j]!.t / 86_400_000), fill, level: px, px, stop, risk, go: false, vetoes: [], checks: {}, ftaR: null, tier: 'placebo', closedThrough: long ? c[j]!.c < px : c[j]!.c > px, r: o.r, x: o.x, resolvedBy: o.resolvedBy, placebo: true });
  }
  return out;
}
