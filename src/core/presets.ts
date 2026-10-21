// Named parameter sets. This is the only place (besides scale.ts for timeframes) that writes to P; a selftest enforces it.
import { P } from './params.js';

type Params = typeof P;
const DEFAULTS: Params = structuredClone(P);

export const PRESETS: Record<string, { note: string; set: Partial<Params> }> = {
  'kev-default': { note: 'the build defaults: levels from depth-checked swings, zones and reactions in H1 ATR, a touch counts if price then moves 1 ATR away', set: {} },
  'trade-check-spec': {
    note: "the user's SOL Trade Check page: levels are clusters of swing points within 0.10 x prior-day ATR(20); a break is a close 0.10 ATR through; a touch counts if 12+ bars from the last and price travelled 0.50 ATR away; space 0.50 ATR",
    set: { levelScaleD1: 1, scanSource: 'fractals', scanClusterAtr: 0.1, zoneHalfAtr: 0.1, touchRule: 'reaction', touchReactionAtr: 0.5, touchClusterBars: 12, retestMinExcursionAtr: 0.5 },
  },
  'one-hour-spec': {
    note: "the user's One Hour engine as recovered from its data: every 3-bar swing point within 0.10 x prior-day ATR counts as a touch if 12+ bars from the last; break = close 0.10 ATR through; space 0.50 ATR",
    set: { levelScaleD1: 1, scanSource: 'fractals', scanClusterAtr: 0.1, zoneHalfAtr: 0.1, touchRule: 'fractal', fractalTolAtr: 0.1, touchClusterBars: 12, retestMinExcursionAtr: 0.5 },
  },
};

/** Applies a preset (plus optional overrides) on top of the defaults and returns a function that restores the previous values. */
export function applyPreset(name: string, overrides: Partial<Params> = {}): () => void {
  const p = PRESETS[name];
  if (!p) throw new Error(`unknown preset "${name}"; available: ${Object.keys(PRESETS).join(', ')}`);
  const before = structuredClone(P);
  Object.assign(P, structuredClone(DEFAULTS), structuredClone(p.set), structuredClone(overrides));
  return () => { Object.assign(P, before); };
}
export function withPreset<T>(name: string, fn: () => T, overrides: Partial<Params> = {}): T {
  const restore = applyPreset(name, overrides);
  try { return fn(); } finally { restore(); }
}
