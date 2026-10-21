import { GOLD, MONDAY, SOL, SUNDAY_NOON } from './series.js';
import type { Instrument } from './types.js';

/** Everything that differs between markets, in one place. Costs follow the user's backtest page for gold (1.2 bps spread + 0.10R slippage). */
export interface InstrumentConfig { instrument: Instrument; dataFile: string; weekAnchor: number; spreadBps: number; slipR: number; note: string }

export const INSTRUMENTS: Record<string, InstrumentConfig> = {
  SOLUSDT: { instrument: SOL, dataFile: 'SOLUSDT_1h.json', weekAnchor: MONDAY, spreadBps: 0, slipR: 0.1, note: 'crypto trades 24/7, so the week starts Monday 00:00 UTC; spread cost not modelled' },
  XAUUSD: { instrument: GOLD, dataFile: 'XAUUSD_h1.json', weekAnchor: SUNDAY_NOON, spreadBps: 1.2, slipR: 0.1, note: 'broker-server clock read as UTC; Sunday-evening bars open the week; closed Saturday' },
};
