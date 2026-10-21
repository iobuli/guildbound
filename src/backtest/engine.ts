import { graph } from '../kev.js';
import { H, makeFeed, SOL, type Feed } from '../core/series.js';
import type { Session } from '../core/session.js';
import { SetupSchema, type Candle, type Instrument } from '../core/types.js';

/** step / maxHold / cooldown are in HOURS so they mean the same thing on M30, H1 and H4 bases. */
export interface BtConfig { start: number; end: number; step: number; maxHold: number; cooldown: number; slipR: number; spreadBps: number; seed: number; period: number; session?: Session; inst: Instrument; holdB: number; heldExit: boolean; minTier: number; entryAtLevel: boolean }
export const TARGETS = [1, 2, 3, 4] as const;
export const KEYS = ['1', '2', '3', '4', 'fta'] as const;
export type RKey = (typeof KEYS)[number];

export interface Row { price: number; dir: 'long' | 'short'; touches: number; tier: string; lxpbPrice: number | null; distAtr: number }
export interface Scan { j: number; lastClose: number; atrH1: number; rows: Row[] }

export interface Trade {
  i: number; scanJ: number; t: number; year: number; day: number;
  dir: 'long' | 'short'; tier: string; touches: number; level: number; fill: number; stop: number | null; risk: number; riskAtr: number; orderDist: number;
  ok: boolean; go: boolean; vetoes: string[]; checks: Record<string, boolean | null>; ftaR: number | null;
  r: Partial<Record<RKey, number>>; x: Partial<Record<RKey, number>>; placebo: boolean;
}

/**
 * Settle one filled order against the bars AFTER the decision. Conservative: if the stop and a target are both inside one bar the stop
 * is taken first, and if the fill bar's own adverse extreme reaches the stop the trade is a loss (we cannot see inside the bar).
 */
export function settleFull(c: Candle[], j: number, long: boolean, entry: number, stop: number, fta: number | null, maxHold: number, slip: number, heldLevel?: number) {
  const risk = Math.abs(entry - stop), r: Partial<Record<RKey, number>> = {}, x: Partial<Record<RKey, number>> = {};
  const tp: Record<string, number | null> = { fta: fta !== null && (long ? fta > entry : fta < entry) ? fta : null };
  for (const k of TARGETS) tp[String(k)] = long ? entry + k * risk : entry - k * risk;
  const keys = KEYS.filter((k) => tp[k] !== null) as RKey[];
  if (long ? c[j]!.l <= stop : c[j]!.h >= stop) { for (const k of keys) { r[k] = -1 - slip; x[k] = j; } return { r, x }; }
  // optional "level held" exit: if the fill bar CLOSES on the wrong side of the level, flatten at that close (knowable only after the bar ends)
  if (heldLevel !== undefined && (long ? c[j]!.c < heldLevel : c[j]!.c > heldLevel)) {
    const rr = (long ? c[j]!.c - entry : entry - c[j]!.c) / risk - slip;
    for (const k of keys) { r[k] = rr; x[k] = j; }
    return { r, x };
  }
  const left = new Set<RKey>(keys);
  for (let t = j + 1; t <= j + maxHold && t < c.length && left.size; t++) {
    const b = c[t]!, stopped = long ? b.l <= stop : b.h >= stop;
    for (const k of [...left]) {
      const p = tp[k]!, hit = long ? b.h >= p : b.l <= p;
      if (stopped) { r[k] = -1 - slip; x[k] = t; left.delete(k); }
      else if (hit) { r[k] = Math.abs(p - entry) / risk - slip; x[k] = t; left.delete(k); }
    }
  }
  const end = Math.min(c.length - 1, j + maxHold), last = c[end]!;
  for (const k of left) { r[k] = (long ? last.c - entry : entry - last.c) / risk - slip; x[k] = end; }
  return { r, x };
}
/**
 * Settle one filled order against the bars AFTER the decision. Conservative: if the stop and a target are both inside one bar the stop
 * is taken first, and if the fill bar's own adverse extreme reaches the stop the trade is a loss (we cannot see inside the bar).
 */
export function settle(c: Candle[], j: number, long: boolean, entry: number, stop: number, fta: number | null, maxHold: number, slip: number) {
  return settleFull(c, j, long, entry, stop, fta, maxHold, slip).r;
}

const flat = (res: ReturnType<typeof graph.run>) => {
  const m: Record<string, boolean | null> = {};
  for (const id of res.order) for (const ch of res.states[id]!.checks) m[ch.rule] = ch.pass === false || m[ch.rule] === false ? false : ch.pass === null || m[ch.rule] === null ? null : true;
  return m;
};

export function runBacktest(candles: Candle[], cfg: Partial<BtConfig> & { start: number; end: number }) {
  const C: BtConfig = { step: 3, maxHold: 120, cooldown: 24, slipR: 0.1, spreadBps: 0, seed: 7, period: H, inst: SOL, holdB: 0, heldExit: false, minTier: 1, entryAtLevel: false, ...cfg };
  const bars = (hours: number) => Math.max(1, Math.round((hours * H) / C.period));
  const stepB = bars(C.step), coolB = bars(C.cooldown);
  C.holdB = bars(C.maxHold);
  const c = candles, feed: Feed = makeFeed(c, { period: C.period, session: C.session });
  const trades: Trade[] = [], scans: Scan[] = [], cool = new Map<string, number>();
  let scan: Scan | null = null, last = -1e9;
  for (let j = C.start; j < Math.min(C.end, c.length - C.holdB - 2); j++) {
    if (j - last >= stepB) {
      const res = graph.run(feed, c[j]!.t, SetupSchema.parse({ dir: 'long' }), { instrument: C.inst });
      const rows = ((res.states['level-scan']?.out.levels as { rows: Row[] } | undefined)?.rows ?? []).filter((r) => r.tier !== 'none' && r.lxpbPrice !== null);
      scan = { j, lastClose: c[j - 1]!.c, atrH1: (res.states.atr?.out.atr as { h1: number } | undefined)?.h1 ?? 0, rows };
      scans.push(scan); last = j;
    }
    const s = scan!, bar = c[j]!;
    for (const L of s.rows) {
      if (L.touches < C.minTier) continue;
      const long = L.dir === 'long', px = C.entryAtLevel ? L.price : L.lxpbPrice!;
      if (long ? !(px < s.lastClose) : !(px > s.lastClose)) continue; // must be a resting order on the right side at placement
      if (!(long ? bar.l <= px : bar.h >= px)) continue;
      const key = `${L.dir}|${Math.round(px / (0.5 * s.atrH1))}`;
      if ((cool.get(key) ?? -1) > j) continue;
      cool.set(key, j + coolB);
      const fill = long ? Math.min(px, bar.o) : Math.max(px, bar.o);
      const T = bar.t + C.period / 2;
      const res = graph.run(feed, T, SetupSchema.parse({ dir: L.dir, style: 'sr-flip', levelPrice: L.price, entry: fill }), { instrument: C.inst, price: fill });
      const entry = res.states.entry?.out.entry as { primary: { go: boolean; vetoes: string[] } | null } | undefined;
      const st = res.states.stop?.out.stop as { stop: number | null; ok: boolean; riskDist: number } | undefined;
      const tg = res.states.target?.out.target as { chosen: { price: number; rr: number } | null } | undefined;
      const ok = !!st && st.ok && st.stop !== null && st.riskDist > 0 && st.riskDist / fill < 0.15;
      const t: Trade = {
        i: j, scanJ: s.j, t: bar.t, year: new Date(bar.t).getUTCFullYear(), day: Math.floor(bar.t / 86_400_000), dir: L.dir, tier: L.tier, touches: L.touches, level: L.price, fill,
        stop: st?.stop ?? null, risk: st?.riskDist ?? 0, riskAtr: s.atrH1 > 0 && st ? st.riskDist / s.atrH1 : 0, orderDist: Math.abs(px - s.lastClose), ok,
        go: entry?.primary?.go ?? false, vetoes: entry?.primary?.vetoes ?? [], checks: flat(res), ftaR: tg?.chosen?.rr ?? null, r: {}, x: {}, placebo: false,
      };
      if (ok) { const slip = C.slipR + (C.spreadBps * 1e-4 * fill) / st!.riskDist; ({ r: t.r, x: t.x } = settleFull(c, j, long, fill, st!.stop!, tg?.chosen?.price ?? null, C.holdB, slip, C.heldExit ? L.price : undefined)); }
      trades.push(t);
    }
  }
  return { trades, scans, cfg: C };
}

/** One placebo per real order: same side, same distance from price (x0.5-1.5), same risk in ATR, same window to get filled. */
export function placebos(c: Candle[], real: Trade[], scans: Map<number, Scan>, cfg: BtConfig): Trade[] {
  let s = (cfg.seed * 2654435761) >>> 0;
  const rand = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  const out: Trade[] = [];
  for (const t of real) {
    if (!t.ok) continue;
    const sc = scans.get(t.scanJ);
    if (!sc) continue;
    const long = t.dir === 'long', d = t.orderDist * (0.5 + rand()), px = long ? sc.lastClose - d : sc.lastClose + d;
    let j = -1;
    for (let k = sc.j; k <= t.i; k++) if (long ? c[k]!.l <= px : c[k]!.h >= px) { j = k; break; }
    if (j < 0) continue;
    const fill = long ? Math.min(px, c[j]!.o) : Math.max(px, c[j]!.o), risk = t.riskAtr * sc.atrH1, stop = long ? fill - risk : fill + risk;
    if (!(risk > 0)) continue;
    const slip = cfg.slipR + (cfg.spreadBps * 1e-4 * fill) / risk, sr = settleFull(c, j, long, fill, stop, null, cfg.holdB, slip, cfg.heldExit ? px : undefined);
    out.push({ ...t, i: j, fill, level: px, stop, risk, ok: true, go: false, vetoes: [], checks: {}, ftaR: null, tier: 'placebo', r: sr.r, x: sr.x, placebo: true });
  }
  return out;
}
