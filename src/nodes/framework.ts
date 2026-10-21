import { defineNode } from '../core/node.js';
import { rule, src } from '../core/rule.js';

export default defineNode({
  id: 'framework',
  title: 'Direction / Objection / Location / Execution',
  stage: 'D',
  summary: 'The structure the trader\'s own journal uses for every trade. The graph\'s stages follow it: D (bias), O (what is in the way), L (the level), E (the order).',
  decideAt: 'prior_bars',
  inputs: {},
  outputs: {},
  rules: [
    rule('framework.doel', 'Newer journal entries are written as Direction (the bias and why), Objection (what stands in the way, such as a target inside a weekly level), Location (the level the trade is built on) and Execution (how the order is placed, usually the H1 LXPB).', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-engulfing.md', '2026-05-03'), src('journal', 'curated/lxpb-trades-pattern-failure.md', '2026-06-24')],
    }),
    rule('framework.stacked-lxpb', 'The passed-ATR trades stack LXPBs across weekly, daily and hourly together with "past ATR". The more timeframes share the same level, the stronger the setup.', {
      status: 'observed', kind: 'heuristic', sources: [src('journal', 'curated/lxpb-trades-passed-atr.md', '2024-12-16'), src('journal', 'curated/lxpb-trades-passed-atr.md', '2024-12-21')],
    }),
    rule('framework.follow-your-rules', 'Rules to live by: 1. always follow your rules; 2. always follow your rules; 3. always follow your rules; 4. know when to break the rules.', {
      kind: 'heuristic', sources: [src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('framework.eighty-twenty', 'About 20% of trades have a daily bias (use D1 for target, H1 for entry and stop); 80% go straight to H1 for target, entry and stop.', {
      sources: [src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('framework.order-of-operations', 'The order the method is applied in: 1. mark levels (W1/D1/H1). 2. Which side of the market is each level on (buy below, sell above). 3. Is it an inverse-last level that price respected last time. 4. Count swing touches. 5. Tier: 3+ blind, 2 needs one confluence, 1 needs two (bias, fib, ATR). 6. Space and momentum, not the immediate retest. 7. Limit order, stop where you would still be bullish, target at the first trouble area. 8. Pull the order if missed and price ran more than half way to target.', {
      status: 'inferred', sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @4:43'), src('transcript', 'levels webinar part 1, user-pasted transcript @7:47'), src('transcript', 'levels webinar part 1, user-pasted transcript @23:27'), src('transcript', 'levels webinar part 1, user-pasted transcript @25:30'), src('transcript', 'levels webinar part 1, user-pasted transcript @53:21'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:00:04')], note: 'Assembled from several places in the transcript; the trader gives the tiers as a summary twice but never lists the whole sequence in one go.',
    }),
    rule('framework.part-two-is-missing', 'This webinar covers only levels where price is going the OPPOSITE way to the last participants (the inverse). A second webinar covers levels where you go the SAME way (buying support, which includes the SFP) and gives a step-by-step "market blueprint" like a flow chart. That second part is not in the sources provided.', {
      status: 'observed', sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:22:08'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:22:50'), src('transcript', 'levels webinar part 1, user-pasted transcript @2:11')],
    }),
    rule('framework.limit-orders', 'Nearly 99% of his trades are limit orders, so he does not need to sit at the screen; a "blind" trade just means a resting limit order with a stop and a target.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @23:44'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:15:25')],
    }),
  ],
});
