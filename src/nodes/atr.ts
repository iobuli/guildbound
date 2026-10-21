import { defineNode } from '../core/node.js';
import { AtrPort } from '../core/ports.js';
import { wilderAtr } from '../core/ta.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';

export default defineNode({
  id: 'atr',
  title: 'ATR yardsticks',
  stage: 'X',
  summary: 'Measures volatility on D1 and H1. Almost every other node sizes things in ATR, so this is the only place ATR is computed.',
  decideAt: 'prior_bars',
  step: { n: 0, label: 'ATR yardsticks' },
  inputs: {},
  outputs: { atr: AtrPort },
  rules: [
    rule('atr.d1-is-the-yardstick', 'The daily ATR is the yardstick for what counts as a deep swing, how close a fib must be, and whether the market has already "passed ATR".', {
      sources: [src('lesson', 'curated/taking-a-trade-h1-sweep.md', '2024-10-22'), src('webinar', 'curated/advanced-sfp.md', '2024-09-24')],
    }),
    rule('atr.h1-24-for-sfp-stop', 'The H1 SFP stop uses the 24-period ATR taken at the close of the sweep candle.', {
      codable: true, sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24')],
    }),
    rule('atr.m5-uses-12', 'M5 SFPs use a 12-period ATR instead.', { sources: [src('webinar', 'curated/levels.md', '2024-09-28')], note: 'No M5 data in this build, so not computed.' }),
    rule('atr.d1-period-20', 'The daily ATR is a 20-period ATR, because there are about 20 trading days in a month: it averages how far the market moves in a day over the last month.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @43:34')]
    }),
    rule('atr.use-the-day-before', 'Read the ATR of the day BEFORE the day in question (hover over the previous daily bar), not the day that triggers the trade.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @44:21')], note: 'Closed daily bars only, so the forming day is never included. This also keeps it causal.',
    }),
    rule('atr.h1-length-unspecified', 'No source gives an H1 ATR length. This build uses Wilder 14 as a scale only (zone width, swing size). The 24-period value is stated for the SFP stop.', {
      status: 'inferred', kind: 'heuristic', codable: true, sources: [src('webinar', 'curated/advanced-sfp.md')],
    }),
  ],
  evaluate: (_i, v) => ({ out: { atr: { d1: wilderAtr(v.d1, P.atrD1N), h1: wilderAtr(v.h1, P.atrN), h1_24: wilderAtr(v.h1, P.atrSfpH1) } } }),
});
