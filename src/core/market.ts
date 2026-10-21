// A market is a folder under markets/<SYMBOL>/ written by `kev ingest`. Everything market-specific (clock, session, costs, split,
// tick size, which files exist) lives in its profile.json; nothing else in the code names a market. Pure module: no fs, so the
// browser bundle can use it too (file access is in market-io.ts).
import type { Candle, Instrument, InstrumentClass } from './types.js';
import type { SessionSpec } from './session.js';

export interface SourceFile { file: string; tf: number; n: number; from: number; to: number; role: 'base' | 'ltf' | 'check' | 'duplicate' }
export interface MarketProfile {
  symbol: string;
  cls: InstrumentClass;
  tick: number;
  decimals: number;
  /** how the data clock relates to UTC; timestamps in markets/ are already converted to UTC */
  clock: { kind: 'fixed' | 'ny+7'; offsetHours: number; method: string };
  session: SessionSpec;
  costs: { spreadBps: number; slipR: number; note: string };
  /** in-sample is every year <= isEndYear */
  split: { isEndYear: number };
  data: { h1: string; ltf: { tf: number; file: string; from: number; to: number }[]; from: number; to: number; sources: SourceFile[] };
  /** facts found at ingest that change how results should be read */
  notes: string[];
}

export const instrumentOf = (p: MarketProfile): Instrument => ({ symbol: p.symbol, cls: p.cls, pip: p.cls.startsWith('fx') ? (p.symbol.includes('JPY') ? 0.01 : 0.0001) : p.tick });

/** Columnar candles: about half the size of an array of objects and quick to load. */
export interface Cols { fmt: 'kev-cols-1'; t0: number; step: number; dt: number[]; o: number[]; h: number[]; l: number[]; c: number[]; v: number[] }
export function toCols(c: readonly Candle[], step: number, decimals: number): Cols {
  const q = (x: number) => +x.toFixed(Math.min(10, decimals + 2));
  return { fmt: 'kev-cols-1', t0: c[0]?.t ?? 0, step, dt: c.map((b, i) => (i ? (b.t - c[i - 1]!.t) / step : 0)), o: c.map((b) => q(b.o)), h: c.map((b) => q(b.h)), l: c.map((b) => q(b.l)), c: c.map((b) => q(b.c)), v: c.map((b) => +b.v.toFixed(3)) };
}
export function fromCols(x: Cols): Candle[] {
  const out: Candle[] = new Array(x.o.length);
  let t = x.t0;
  for (let i = 0; i < x.o.length; i++) { t += x.dt[i]! * x.step; out[i] = { t, o: x.o[i]!, h: x.h[i]!, l: x.l[i]!, c: x.c[i]!, v: x.v[i]! }; }
  return out;
}
