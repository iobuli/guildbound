import type { RunResult } from './graph.js';

/**
 * Hierarchical state token (the Kronos-style idea: a coarse word for the regime, a fine word for what is happening in it).
 * Built only from nodes that do not depend on a proposed trade, so it can describe any bar.
 *   coarse (18) = higher-timeframe bias (3) x ATR state (3) x mode (2)
 *   fine   (40) = SFP state (5) x Tuesday setup (2) x session (4)
 */
export const N_COARSE = 18, N_FINE = 40;
const BIAS = ['neutral', 'long', 'short'] as const, ATRS = ['inside', 'passed-up', 'passed-down'] as const;
const SFPS = ['none', 'h1-long', 'h1-short', 'h1-long-wait', 'h1-short-wait'] as const, SESS = ['asian', 'london', 'ny', 'off'] as const;

export interface KevToken { coarse: number; fine: number; label: string }

export function encode(res: RunResult): KevToken | null {
  const st = res.states;
  const bias = st['daily-bias']?.out.bias as { htf: 'long' | 'short' | 'neutral' } | undefined;
  const gate = st['atr-gate']?.out.gate as { passedUp: boolean; passedDown: boolean } | undefined;
  const ctx = st.context?.out.context as { mode: 'with-bias' | 'no-bias'; session: 'asian' | 'london' | 'ny' | 'off' } | undefined;
  const sfp = st.sfp?.out.sfp as { h1: { dir: 'long' | 'short'; valid: boolean; entryMode: string; ageBars: number }[] } | undefined;
  const tue = st['tuesday-rto']?.out.tuesday as { valid: boolean } | undefined;
  if (!bias || !gate || !ctx || !sfp || !tue) return null;
  const b = BIAS.indexOf(bias.htf), a = gate.passedUp ? 1 : gate.passedDown ? 2 : 0, m = ctx.mode === 'with-bias' ? 1 : 0;
  const fresh = sfp.h1.find((s) => s.valid && s.ageBars <= 3);
  const s = !fresh ? 0 : fresh.dir === 'long' ? (fresh.entryMode === 'immediate' ? 1 : 3) : (fresh.entryMode === 'immediate' ? 2 : 4);
  const coarse = (b * 3 + a) * 2 + m, fine = (s * 2 + +tue.valid) * 4 + SESS.indexOf(ctx.session);
  return { coarse, fine, label: `${BIAS[b]}-bias / ${ATRS[a]} / ${m ? 'with-bias' : 'no-bias'} | sfp:${SFPS[s]} tue:${tue.valid ? 'yes' : 'no'} ${ctx.session}` };
}
