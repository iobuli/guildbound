// File access for markets/<SYMBOL>/. The registry is simply the list of folders that contain a profile.json.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { fromCols, instrumentOf, type MarketProfile } from './market.js';
import { buildSession, type Session } from './session.js';
import { makeFeed, type Feed } from './series.js';
import type { Candle, Instrument } from './types.js';

export const MARKETS_DIR = fileURLToPath(new URL('../../markets/', import.meta.url));
const dir = (sym: string) => `${MARKETS_DIR}${sym}/`;

export function listMarkets(): string[] {
  if (!existsSync(MARKETS_DIR)) return [];
  return readdirSync(MARKETS_DIR).filter((d) => existsSync(`${dir(d)}profile.json`)).sort();
}

/** profile.json merged with an optional hand-written override.json (any field; nested objects merge one level deep). */
export function loadProfile(sym: string): MarketProfile {
  if (!existsSync(`${dir(sym)}profile.json`)) throw new Error(`no market "${sym}". Available: ${listMarkets().join(', ') || 'none (run kev ingest)'}`);
  const p = JSON.parse(readFileSync(`${dir(sym)}profile.json`, 'utf8')) as MarketProfile;
  const ovPath = `${dir(sym)}override.json`;
  if (existsSync(ovPath)) {
    const ov = JSON.parse(readFileSync(ovPath, 'utf8')) as Record<string, unknown>;
    for (const [k, v] of Object.entries(ov)) (p as unknown as Record<string, unknown>)[k] = v && typeof v === 'object' && !Array.isArray(v) ? { ...((p as unknown as Record<string, object>)[k] ?? {}), ...v } : v;
  }
  return p;
}

const cache = new Map<string, Candle[]>();
function readSeries(path: string): Candle[] {
  if (!cache.has(path)) cache.set(path, fromCols(JSON.parse(readFileSync(path, 'utf8'))));
  return cache.get(path)!;
}

export interface Market { profile: MarketProfile; h1: Candle[]; session: Session; instrument: Instrument; feed: () => Feed; ltf: () => { tf: number; c: Candle[] }[] }
/** Everything needed to run kev on one market. The finer series load only when asked for. */
export function openMarket(sym: string): Market {
  const profile = loadProfile(sym), h1 = readSeries(`${dir(sym)}${profile.data.h1}`);
  const session = buildSession(profile.session, h1);
  let feed: Feed | null = null, ltf: { tf: number; c: Candle[] }[] | null = null;
  return {
    profile, h1, session, instrument: instrumentOf(profile),
    feed: () => (feed ??= makeFeed(h1, { session })),
    ltf: () => (ltf ??= profile.data.ltf.map((x) => ({ tf: x.tf, c: readSeries(`${dir(sym)}${x.file}`) })).sort((a, b) => a.tf - b.tf)),
  };
}
