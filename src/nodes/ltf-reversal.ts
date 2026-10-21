import { defineNode } from '../core/node.js';
import { rule, src } from '../core/rule.js';

export default defineNode({
  id: 'ltf-reversal',
  title: 'M5 failed-breakdown reversal (Bund)',
  stage: 'E',
  summary: 'A lower-timeframe reversal he trades a lot on the M5 Bund. It is not an SFP.',
  decideAt: 'prior_bars',
  inputs: {},
  outputs: {},
  rules: [
    rule('ltf-reversal.pattern', 'Price closes through the lows three times but immediately goes bid each time. "If the market is making fresh lows it should be a weak market, but it is immediately going bid." Weakness keeps getting bought.', {
      sources: [src('webinar', 'curated/notes-images.md', '2024-09-25')],
    }),
    rule('ltf-reversal.needs-divergence', 'It needs MACD divergence, the same as the sweep technique.', {
      sources: [src('webinar', 'curated/notes-images.md', '2024-09-25')],
    }),
    rule('ltf-reversal.entry', 'Entry is a market buy at the first reversal candle after the third breakdown, with the stop just under the lows. No ATR or thrust candle is needed.', {
      sources: [src('webinar', 'curated/notes-images.md', '2024-09-25')],
    }),
    rule('ltf-reversal.m5-habits', 'M5 levels are marked and traded the same way as H1; M5 SFPs use a 12-period ATR; he does not hold M5 through news; the M5 stop trail can differ (a head-and-shoulders shoulder instead of the thrust candle).', {
      status: 'observed', sources: [src('webinar', 'curated/levels.md', '2024-09-28'), src('webinar', 'curated/trade-management.md', '2024-10-18')],
    }),
  ],
});
