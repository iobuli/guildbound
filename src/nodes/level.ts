import { defineNode } from '../core/node.js';
import { AtrPort, LevelPort } from '../core/ports.js';
import { analyzeLevel } from '../core/levelmath.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

export default defineNode({
  id: 'level',
  title: 'Support / resistance level',
  stage: 'L',
  summary: 'Takes the level the trade is built on and reconstructs its history: where it broke, how many times it was respected in its previous role ("opposite last"), the last high/low before the breakout (LXPB), and whether price is still respecting it.',
  decideAt: 'prior_bars',
  step: { n: 2, label: 'Is it an inverse-last level that price respected last time, and how many swing touches?' },
  inputs: { atr: AtrPort },
  outputs: { level: LevelPort },
  rules: [
    rule('level.inverse-last-only', 'Trade only levels that last did the OPPOSITE of what you are about to do. To buy below market, the level must have been RESISTANCE last; to sell above market, it must have been SUPPORT last. Levels that last acted in your own direction are a different, harder trade.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @7:47'), src('transcript', 'levels webinar part 1, user-pasted transcript @8:31'), src('transcript', 'levels webinar part 1, user-pasted transcript @14:15')],
    }),
    rule('level.look-left-at-the-last-interaction', 'Always look at the most recent interaction with the level. If price washed through it recently (traded in and out and ignored it), do not trade it, however well it respected the level in the past.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @6:44'), src('transcript', 'levels webinar part 1, user-pasted transcript @7:12'), src('transcript', 'levels webinar part 1, user-pasted transcript @21:39'), src('webinar', 'curated/levels.md', '2024-09-26')],
    }),
    rule('level.touch-is-a-swing-touch', 'Count swing touches, not candles. A cluster of candles on the level that produces ONE swing away is one touch (three candles touching = one). Only touches that cause a decent swing away count.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @15:39'), src('transcript', 'levels webinar part 1, user-pasted transcript @16:38'), src('transcript', 'levels webinar part 1, user-pasted transcript @17:06'), src('transcript', 'levels webinar part 1, user-pasted transcript @33:12')],
    }),
    rule('level.touches-must-be-separate-in-time', 'The swings should be deep and separated by time and space; the further price swings away, the longer it takes to come back. Touches that sit close together with no reaction between them count as one.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @15:39'), src('transcript', 'levels webinar part 1, user-pasted transcript @33:12'), src('webinar', 'curated/levels.md', '2024-09-30')],
    }),
    rule('level.time-matters', 'The longer a level takes to form, the stronger it is: a level built over a week beats one built over a trading day. A level formed over two days can still be traded if the touches are clean (discretionary).', {
      codable: true, kind: 'preference', sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @17:45'), src('transcript', 'levels webinar part 1, user-pasted transcript @30:49')],
    }),
    rule('level.previous-role-only', 'Do not count the touches from before the level flipped. A level that was resistance, then support once, then comes back as resistance is a ONE-touch level.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:13:06'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:14:37'), src('webinar', 'curated/notes-images.md', '2024-09-10')],
    }),
    rule('level.broken-level-is-still-a-breakout', 'A level that was broken and has not been revisited is still a breakout to trade, even if you have forgotten about it. It does not matter how long it takes to come back.', {
      status: 'observed', sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @18:40'), src('transcript', 'levels webinar part 1, user-pasted transcript @19:17'), src('transcript', 'levels webinar part 1, user-pasted transcript @20:14')],
    }),
    rule('level.lxpb', 'Place the order at the last high (low) before the breakout, the LHPB / LLPB. Price tends to gravitate to it. With several candidate lines, take the one at the most recent area.', {
      codable: true, sources: [src('webinar', 'curated/levels.md', '2024-09-24'), src('webinar', 'curated/levels.md', '2024-09-30'), src('webinar', 'curated/notes-images.md', '2024-09-23')],
    }),
    rule('level.recent-price-action-wins', 'Recent price action outweighs older price action when drawing the line: the line sits where the recent touches are.', {
      sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('webinar', 'curated/levels.md', '2024-09-25')],
    }),
    rule('level.untradeable-when-disrespected', 'A level becomes untradeable when the latest touch is not respected by price action.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-25'), src('transcript', 'levels webinar part 1, user-pasted transcript @7:02')],
    }),
    rule('level.chopping-through', 'Do not draw a level that price is currently chopping through, even if it was a good level further left. If it is disrespected repeatedly but was respected recently, keep it but be cautious.', {
      codable: true, sources: [src('webinar', 'curated/levels.md', '2024-09-26'), src('webinar', 'curated/levels.md', '2024-09-27')],
      note: 'Computed as a count of crossings in the last 40 bars (4 or more = chopping).',
    }),
    rule('level.trendlines-are-levels', 'Trend lines are treated as levels, with the same premise (hit multiple times, breaks, retests), not as confluence. He does not have a great love of them.', {
      sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:20:40')],
    }),
    rule('level.crumbles-if-retested', 'A level that keeps being tested is likely to crumble. On a successful trade it often shifts quickly after the second touch.', {
      kind: 'heuristic', status: 'observed', sources: [src('webinar', 'curated/levels.md', '2024-09-25')],
    }),
    rule('level.zone-not-line', 'A level is a zone. Price has only broken it when it breaks the whole zone, not the first line.', {
      status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-10-28')],
    }),
    rule('level.how-far-back', 'He looks far back for levels, in one case using a level from February 2022 for a trade in September 2024.', {
      status: 'observed', kind: 'heuristic', sources: [src('webinar', 'curated/levels.md', '2024-09-24'), src('journal', 'curated/lxpb-trades-passed-atr.md', '2025-01-06')],
    }),
    rule('level.pick-llpb-when-stacked', 'With a weekly resistance and a second resistance slightly above it, put the order at the LLPB.', {
      sources: [src('webinar', 'curated/levels.md', '2024-09-28')],
    }),
  ],
  evaluate: (inp, v, setup) => {
    const centre = setup.levelPrice;
    if (!centre) return { out: { level: analyzeLevel([], 0, setup.dir, 0) } };
    const level = analyzeLevel(v.h1, centre, setup.dir, P.levelScaleD1 ? inp.atr.d1 : inp.atr.h1);
    if (!level.found) return { out: { level }, checks: [{ rule: 'level.opposite-last', pass: false, detail: 'price has not broken through this level yet, so there is no flipped level to count' }] };
    const checks: Check[] = [
      { rule: 'level.opposite-last', pass: level.touches >= 1, detail: `${level.touches} swing touch(es) in the level's previous role` },
      { rule: 'level.chopping-through', pass: !level.disrespected, detail: `${level.chopCrossings} crossings of the level in the last 40 bars` },
      { rule: 'level.time-matters', pass: level.spanBars >= 24 ? true : null, detail: `the counted touches span ${level.spanBars} bars (${(level.spanBars / 24).toFixed(1)} days); a level built over a week is stronger than one built in a day` },
    ];
    return { out: { level }, checks };
  },
});
