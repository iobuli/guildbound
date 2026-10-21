import { defineNode } from '../core/node.js';
import { AtrPort, SfpPort, SwingsPort, type SfpSignalT } from '../core/ports.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Candle, Check } from '../core/types.js';
import type { SwingInfoT } from '../core/ports.js';

function signalsFor(tf: 'h1' | 'd1', c: Candle[], swings: SwingInfoT[], atrTf: number, atr24: number): SfpSignalT[] {
  const last = c.length - 1, ageMax = tf === 'h1' ? P.sfpValidH1Bars : P.sfpValidD1Bars;
  const out: SfpSignalT[] = [];
  for (const s of swings) {
    if (s.sweptAt === null || s.sweptAt < last - ageMax) continue;
    const b = c[s.sweptAt]!, isHigh = s.side === 'high';
    if (isHigh ? !(b.c < s.price) : !(b.c > s.price)) continue; // closed through = breakout, not a failure
    const range = b.h - b.l, tol = P.zoneHalfAtr * atrTf;
    const closeLoc = range > 0 ? (isHigh ? (b.h - b.c) / range : (b.c - b.l) / range) : 0.5;
    const tooSmall = range < P.sfpMinBarAtr * atrTf;
    const wide = range >= P.sfpWideBarAtr * atrTf;
    const strongAgainst = wide && closeLoc <= P.sfpCloseNearExtreme;
    // visit count: swing creation is visit 1, each separate return to the area is another, the sweep is the last
    let returns = 0, inZone = true;
    for (let j = s.i + 1; j < s.sweptAt; j++) {
      const near = isHigh ? c[j]!.h >= s.price - tol : c[j]!.l <= s.price + tol;
      if (near) { if (!inZone) { returns++; inZone = true; } } else inZone = false;
    }
    const visitNo = 1 + returns + 1;
    let invalidated = false;
    for (let j = s.sweptAt + 1; j <= last; j++) if (isHigh ? c[j]!.c > b.h : c[j]!.c < b.l) invalidated = true;
    const ageBars = last - s.sweptAt;
    const sweepExtreme = isHigh ? b.h : b.l;
    out.push({
      tf, dir: isHigh ? 'short' : 'long', swingPrice: s.price, swingT: s.t, sweepT: b.t, sweepIdx: s.sweptAt, sweepExtreme, closePrice: b.c, closeLoc,
      tooSmall, strongAgainst, deep: s.deep, visitNo, needsDivergence: visitNo === P.sfpMaxVisits, ageBars, invalidated,
      stopAtr: isHigh ? sweepExtreme + atr24 : sweepExtreme - atr24, stopAdvanced: sweepExtreme,
      entryMode: strongAgainst ? 'wait-half-to-stop' : 'immediate',
      valid: s.deep && !tooSmall && visitNo <= P.sfpMaxVisits && !invalidated && ageBars <= ageMax,
    });
  }
  return out.sort((a, b) => b.sweepIdx - a.sweepIdx);
}

export default defineNode({
  id: 'sfp',
  title: 'Swing failure pattern (SFP)',
  stage: 'L',
  summary: 'Finds sweeps of deep swings that close back inside, applies the visit-count, size and invalidation conditions, and says how the entry and stop should be set.',
  decideAt: 'prior_bars',
  step: { n: 0.2, label: 'SFP detector' },
  inputs: { atr: AtrPort, swings: SwingsPort },
  outputs: { sfp: SfpPort },
  rules: [
    rule('sfp.definition', 'An SFP is essentially a false break of a high or low, but with strict criteria for what it must look like and when it is traded.', {
      sources: [src('tweet', 'curated tweets', '2026-05-22')],
    }),
    rule('sfp.deep-swing-only', 'Only sweeps of significant (deep) swings count. Sweeping a shallow swing is ignored.', {
      codable: true, sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24'), src('lesson', 'curated/taking-a-trade-h1-sweep.md', '2024-10-22')],
    }),
    rule('sfp.first-sweep-only', 'Only the first sweep of a level is tradeable. The second or any later sweep is invalid. A fresh sweep in a new area is fine.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-h1-sweep.md', '2024-10-22')],
    }),
    rule('sfp.visit-count', 'Count visits to the area: the 1st creates the swing, the 2nd needs a sweep, the 3rd needs a sweep AND a MACD divergence, the 4th is a no. Tests on the daily count toward the hourly.', {
      codable: true, sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24'), src('webinar', 'curated/advanced-sfp.md', '2024-09-26')],
      note: 'The MACD divergence is flagged (needsDivergence) but not computed here.',
    }),
    rule('sfp.time-and-space', 'There must be time and space between the original swing and the SFP, as with an S/R retest. Swing levels stay valid for about 2 days.', {
      codable: true, sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24')],
    }),
    rule('sfp.enter-at-close', 'On an H1 sweep that closes back inside, sell (buy) at the current price the moment the candle closes.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-h1-sweep.md', '2024-10-22')],
    }),
    rule('sfp.strong-close-wait', 'If the sweep bar is wide and closes within 20% of its extreme, do not enter at the close. Draw the ATR stop; if price moves about half way toward it and turns back, place a stop order at the original SFP close.', {
      codable: true, sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24'), src('webinar', 'user-pasted SFP notes', '2024-09-24')],
    }),
    rule('sfp.stop-atr24', 'Stop = the sweep extreme plus the 24-period ATR. The "advanced" entry uses the sweep extreme itself.', {
      codable: true, sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24')],
    }),
    rule('sfp.invalidation', 'If a candle closes beyond the SFP candle itself, the SFP is invalid and an open trade is exited at once.', {
      codable: true, sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24')],
    }),
    rule('sfp.small-bars-meaningless', 'A sweep candle that is tiny (a 6-pip Asian-session bar) is unlikely to mean anything.', {
      codable: true, status: 'inferred', sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24')], note: 'The 0.5 ATR cut-off is the builder\'s; the source gives only the example.',
    }),
    rule('sfp.distrust-session-close', 'On the FTSE he does not trust SFPs forming at the close of the market (volatility from closing positions). Less of a concern in forex.', {
      sources: [src('webinar', 'curated/advanced-sfp.md', '2024-09-24')],
    }),
    rule('sfp.ramp-into-sweep', 'A short is not ideal if price ramps up bullishly into the sweep. Better if the SFP candle breaks back under prior support, for example as a bearish engulfing.', {
      kind: 'preference', status: 'observed', sources: [src('webinar', 'curated/advanced-sfp.md', '2025-01-21')],
    }),
    rule('sfp.opposite-sfp-closes-trades', 'An H1 SFP against an open trade is a reason to close it, but give price an hour or two to show its hand.', {
      sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24')],
    }),
    rule('sfp.enter-at-the-swept-level', 'Newer practice: enter at the swept level itself or the nearest naked level, not blindly at the SFP close. "Stopped out right on the low that was SFP\'d, which is where you should have been looking to enter."', {
      status: 'observed', sources: [src('tweet', 'curated tweets', '2026-07-01'), src('tweet', 'curated tweets', '2022-10-21')],
    }),
    rule('sfp.average-rr', 'Average RR on SFP trades is about 1.2 with a high hit rate; he takes them from 0.8R and sometimes 0.5R when probability is high.', {
      kind: 'heuristic', sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24')], note: 'Hit rate (~90%) is a self-reported figure, not tested here.',
    }),
    rule('sfp.not-at-inverse-levels', 'An SFP is not relevant at a level that is the inverse last: an SFP presumes the level was support (resistance) last, and that is the other kind of trade. If you get an SFP at a level, you are in the same-direction case.', {
      sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @56:53')],
    }),
  ],
  conflicts: [{
    a: 'sfp.enter-at-close', b: 'sfp.enter-at-the-swept-level',
    note: 'The 2024 course teaches selling at the SFP candle close. The 2022 and 2026 posts prefer entering at the swept level / nearest naked level. Both are kept; the entry node reports whichever the setup style asks for.',
  }],
  evaluate: (inp, v, setup) => {
    const out = { h1: signalsFor('h1', v.h1, inp.swings.h1, inp.atr.h1, inp.atr.h1_24), d1: signalsFor('d1', v.d1, inp.swings.d1, inp.atr.d1, inp.atr.d1) };
    const checks: Check[] = [];
    if (setup.style === 'sfp') {
      const m = out.h1.find((s) => s.dir === setup.dir);
      checks.push(m
        ? { rule: 'sfp.deep-swing-only', pass: m.deep, detail: `swing depth ${m.deep ? 'is' : 'is NOT'} deep` }
        : { rule: 'sfp.deep-swing-only', pass: false, detail: `no recent H1 sweep-and-close for a ${setup.dir}` });
      if (m) {
        checks.push({ rule: 'sfp.visit-count', pass: m.visitNo <= P.sfpMaxVisits, detail: `visit ${m.visitNo}${m.needsDivergence ? ' (needs MACD divergence, not checked)' : ''}` });
        checks.push({ rule: 'sfp.invalidation', pass: !m.invalidated, detail: m.invalidated ? 'a later close went beyond the sweep candle' : 'not invalidated' });
        checks.push({ rule: 'sfp.small-bars-meaningless', pass: !m.tooSmall, detail: m.tooSmall ? 'sweep bar is tiny' : 'bar size ok' });
        checks.push({ rule: 'sfp.time-and-space', pass: m.ageBars <= P.sfpValidH1Bars, detail: `${m.ageBars} bars old (limit ${P.sfpValidH1Bars})` });
      }
    }
    return { out: { sfp: out }, checks };
  },
});
