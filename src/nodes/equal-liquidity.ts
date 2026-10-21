import { defineNode } from '../core/node.js';
import { rule, src } from '../core/rule.js';

export default defineNode({
  id: 'equal-liquidity',
  title: 'Equal highs / lows',
  stage: 'O',
  summary: 'Equal highs or lows as a target and as a trade in their own right.',
  decideAt: 'prior_bars',
  inputs: {},
  outputs: {},
  rules: [
    rule('equal-liquidity.almost-always', '"I almost always try to get in to equal highs/lows." The current daily high has to go first to show it is ready, and a weekly level that has been ground into helps. No other confluences needed.', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-equal-swings.md', '2026-02-23')],
    }),
    rule('equal-liquidity.sweep-tolerance', 'A second high can count as sweeping the first by a fraction of a pip (0.3 pips in the example), with an H1 hammer.', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-equal-swings.md', '2026-03-12')],
    }),
    rule('equal-liquidity.as-a-target', 'Equal lows are used as the target for H1 shorts (and a double top is the destination in a long). Seen as "good targets" because stops sit there.', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-engulfing.md', '2026-02-12'), src('tweet', 'curated tweets', '2026-09-21')],
    }),
    rule('equal-liquidity.pattern-for-tops', 'General pattern for spotting tops: a large run-up, an indecision candle (doji) at the peak, a good pullback, then price running back up but failing to make a new high.', {
      kind: 'heuristic', status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-10-19')],
    }),
  ],
});
