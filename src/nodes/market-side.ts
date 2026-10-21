import { defineNode } from '../core/node.js';
import { MarketSidePort } from '../core/ports.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

const T = (t: string) => src('transcript', `levels webinar part 1, user-pasted transcript @${t}`);

export default defineNode({
  id: 'market-side',
  title: 'Which side of the market is the level on?',
  stage: 'D',
  summary: 'The first question in the method: he always buys below the market and sells above it. A level below price can only be a buy; a level above price can only be a sell. That fixes the direction before anything else is looked at.',
  decideAt: 'prior_bars',
  step: { n: 1, label: 'Which side of the market is the level on? (buy below, sell above)' },
  inputs: {},
  outputs: { side: MarketSidePort },
  rules: [
    rule('market-side.buy-below-sell-above', '"I\'m always buying below market and I\'m always selling above market." So the trade is always counter to the short-term trend: buying a dip or selling a rally. A level below price is a buy, a level above is a sell.', {
      codable: true, sources: [T('4:43'), T('5:08'), T('1:08:16')], note: 'Applies to resting limit orders at levels. SFP entries at the candle close are market orders and are exempt.',
    }),
    rule('market-side.draw-levels-on-three-timeframes', 'Levels are drawn on the weekly, daily and hourly charts, and the hourly shows all of them. The question is then which of those to trade.', {
      sources: [T('2:44')],
    }),
    rule('market-side.homework', 'Homework given: when price rallies, look at the areas above market that were support last; when it falls, look at the areas below market that were resistance last, and trade off those.', {
      status: 'observed', sources: [T('1:08:00')],
    }),
    rule('market-side.nothing-on-the-right-side', 'If no level on the right side of the market qualifies, there is no trade. In the gold example every level above market was resistance last, so there was nothing to sell until price rallied into one.', {
      status: 'observed', sources: [T('21:02'), T('1:04:06')],
    }),
  ],
  evaluate: (_i, v, setup) => {
    const price = v.h1.at(-1)?.c, centre = setup.levelPrice ?? setup.entry;
    if (price === undefined || centre === undefined)
      return { out: { side: { relation: 'unknown' as const, requiredDir: 'neutral' as const, valid: true, detail: 'need a level price and a last close to judge' } } };
    const relation = (centre < price ? 'below' : 'above') as 'below' | 'above', required = (relation === 'below' ? 'long' : 'short') as 'long' | 'short';
    const applies = setup.style === 'sr-flip', valid = !applies || setup.dir === required;
    const detail = `level ${centre} is ${relation} the last close ${price}: that is a ${required}${applies ? '' : ' (not enforced for this entry style)'}`;
    const checks: Check[] = applies ? [{ rule: 'market-side.buy-below-sell-above', pass: valid, detail: valid ? detail : `${detail}, but the proposed trade is a ${setup.dir}` }] : [];
    return { out: { side: { relation, requiredDir: required, valid, detail } }, checks };
  },
});
