// Trading-day and trading-week boundaries for a market. Every daily or weekly quantity (D1 bars, D1 ATR, day open, week open,
// "previous day", Tuesday rules, sessions) goes through here, so no node ever calls getUTCDay/getUTCHours on its own.
import type { Candle } from './types.js';

export const H = 3_600_000, D = 24 * H, W = 7 * D;

/**
 * 'ny-close': the day runs 17:00 to 17:00 New York (DST-aware). It is the convention the sources use ("9pm, an hour before the daily close",
 *             "since 22:00 on Monday", both UK time), and the standard for FX, metals and index CFDs.
 * 'utc':      the day runs 00:00 to 00:00 UTC. It is the exchange convention for crypto.
 */
export type DayBoundary = 'ny-close' | 'utc';
export interface SessionSpec {
  day: DayBoundary;
  /** a trading day that starts after a 24h+ gap with this many hours or fewer left in it is merged into the next day (Sunday stubs) */
  stubHours: number;
  /** clock for session names and "22:00 on Monday" style rules */
  traderTz: 'Europe/London' | 'UTC';
}
export const defaultSpec = (cls: string): SessionSpec => ({ day: cls === 'crypto' ? 'utc' : 'ny-close', stubHours: 6, traderTz: 'Europe/London' });

// ---- daylight saving, hand-rolled so it is fast and deterministic (checked against Intl in the selftest) ----
const sundayOnOrAfter = (y: number, m: number, d: number) => { const t = Date.UTC(y, m, d); return t + ((7 - new Date(t).getUTCDay()) % 7) * D; };
const lastSunday = (y: number, m: number) => { const t = Date.UTC(y, m + 1, 0); return t - new Date(t).getUTCDay() * D; };
const usCache = new Map<number, [number, number]>(), euCache = new Map<number, [number, number]>();
function usDst(t: number): boolean {
  const y = new Date(t).getUTCFullYear();
  let r = usCache.get(y);
  if (!r) {
    // 2007+: second Sunday of March 02:00 EST (07:00 UTC) to first Sunday of November 02:00 EDT (06:00 UTC); 1987-2006: first Sunday of April to last Sunday of October
    r = y >= 2007 ? [sundayOnOrAfter(y, 2, 8) + 7 * H, sundayOnOrAfter(y, 10, 1) + 6 * H] : [sundayOnOrAfter(y, 3, 1) + 7 * H, lastSunday(y, 9) + 6 * H];
    usCache.set(y, r);
  }
  return t >= r[0] && t < r[1];
}
function euDst(t: number): boolean {
  const y = new Date(t).getUTCFullYear();
  let r = euCache.get(y);
  if (!r) { r = [lastSunday(y, 2) + H, lastSunday(y, 9) + H]; euCache.set(y, r); }
  return t >= r[0] && t < r[1];
}
export const nyOffsetH = (t: number) => (usDst(t) ? -4 : -5);
export const londonOffsetH = (t: number) => (euDst(t) ? 1 : 0);

export interface Session {
  spec: SessionSpec;
  /** the trading date a time belongs to, as UTC-midnight ms of that date's name (after stub merging) */
  label(t: number): number;
  /** UTC instant the (merged) trading day containing t began */
  dayStart(t: number): number;
  /** UTC instant that trading day ends */
  dayEnd(t: number): number;
  /** Monday-based trading week containing t: its first instant */
  weekStart(t: number): number;
  weekEnd(t: number): number;
  /** weekday of the trading day, 0 = Sunday ... 6 = Saturday */
  weekday(t: number): number;
  /** the trader's local clock (London by default) */
  local(t: number): { hour: number; weekday: number };
  /** UTC instant of a London-local (or UTC) wall-clock hour on a given date label */
  wallClock(label: number, hour: number): number;
  /** labels of stub days that were merged into the following day */
  merged: ReadonlySet<number>;
}

/** Raw (unmerged) day arithmetic for a boundary. */
function dayMath(day: DayBoundary) {
  if (day === 'utc') return { label: (t: number) => Math.floor(t / D) * D, startOf: (label: number) => label };
  // NY close: the trading date is the New York calendar date of (local time + 7h), so Monday runs Sunday 17:00 -> Monday 17:00 NY
  const label = (t: number) => Math.floor((t + nyOffsetH(t) * H + 7 * H) / D) * D;
  const startOf = (lab: number) => {
    const local = lab - 7 * H; // 17:00 local on the previous date, written as if UTC
    let u = local - nyOffsetH(local + 5 * H) * H;
    if (nyOffsetH(u) !== nyOffsetH(local + 5 * H)) u = local - nyOffsetH(u) * H;
    return u;
  };
  return { label, startOf };
}

/**
 * Builds the session for one market. `bars` (the execution series) is only used to find stub days: a day whose first bar comes after
 * a 24h+ gap with <= stubHours left before the day ends. That is known at the stub's first bar, so the merge is causal.
 */
export function buildSession(spec: SessionSpec, bars: readonly Candle[] = []): Session {
  const m = dayMath(spec.day);
  const merged = new Set<number>();
  for (let i = 0; i < bars.length; i++) {
    const t = bars[i]!.t, gap = i === 0 ? 0 : t - bars[i - 1]!.t;
    if (gap < D) continue;
    const lab = m.label(t), end = m.startOf(lab + D);
    if (end - t <= spec.stubHours * H) merged.add(lab);
  }
  const label = (t: number) => { const l = m.label(t); return merged.has(l) ? l + D : l; };
  const dayStart = (t: number) => { const l = label(t); return merged.has(l - D) ? m.startOf(l - D) : m.startOf(l); };
  const dayEnd = (t: number) => m.startOf(label(t) + D);
  const weekday = (t: number) => new Date(label(t)).getUTCDay();
  const mondayLabel = (t: number) => { const l = label(t); return l - ((new Date(l).getUTCDay() + 6) % 7) * D; };
  const weekStart = (t: number) => { const mon = mondayLabel(t); return merged.has(mon - D) ? m.startOf(mon - D) : m.startOf(mon); };
  const weekEnd = (t: number) => m.startOf(mondayLabel(t) + 7 * D);
  const off = (t: number) => (spec.traderTz === 'Europe/London' ? londonOffsetH(t) : 0);
  const local = (t: number) => { const d = new Date(t + off(t) * H); return { hour: d.getUTCHours(), weekday: d.getUTCDay() }; };
  const wallClock = (lab: number, hour: number) => { const guess = lab + hour * H; return guess - off(guess - H) * H; };
  return { spec, label, dayStart, dayEnd, weekStart, weekEnd, weekday, local, wallClock, merged };
}

/** Aggregates base bars into session days or weeks. Each bar carries `e`, the instant it closes, so a 25h merged day is never "closed" early. */
export function aggregateSession(c: readonly Candle[], s: Session, unit: 'day' | 'week'): Candle[] {
  const start = unit === 'day' ? s.dayStart : s.weekStart, end = unit === 'day' ? s.dayEnd : s.weekEnd;
  const out: Candle[] = [];
  for (const b of c) {
    const k = start(b.t), last = out[out.length - 1];
    if (last && last.t === k) { last.h = Math.max(last.h, b.h); last.l = Math.min(last.l, b.l); last.c = b.c; last.v += b.v; }
    else out.push({ t: k, o: b.o, h: b.h, l: b.l, c: b.c, v: b.v, e: end(b.t) });
  }
  return out;
}
