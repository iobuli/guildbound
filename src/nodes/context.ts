import { defineNode } from '../core/node.js';
import { BiasPort, ContextPort } from '../core/ports.js';
import { rule, src } from '../core/rule.js';

export default defineNode({
  id: 'context',
  title: 'Time, session and mode',
  stage: 'X',
  summary: 'Works out the session, whether it is a late-Friday or flatten-by-close situation, and whether the trade is a with-bias trade or a quick no-bias scalp.',
  decideAt: 'prior_bars',
  step: { n: 5.1, label: 'Context: session, day of week, flatten-by-close' },
  inputs: { bias: BiasPort },
  outputs: { context: ContextPort },
  rules: [
    rule('context.friday-afternoon', 'Context matters: at 2pm on a Friday price is not going to move much, so without a strong bias targets should be conservative.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24')], note: 'Treated as Friday from 13:00 UTC; the source gives only "2pm".',
    }),
    rule('context.flatten-by-close', 'Forex positions are closed at the end of the week. FTSE and Bund trades are closed at the end of the day.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('context.no-bias-means-quick', 'Without a daily bias the trade is in and out quickly. 80% of his trades are like this at 0.8-1R on average.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('context.crosspairs-squeeze', 'Be careful on cross pairs (anything not paired with USD). They seem to squeeze more, so the stop may need to be safer.', {
      sources: [src('webinar', 'user-pasted notes', '2024-09-25'), src('webinar', 'curated/notes-images.md', '2024-09-23')],
    }),
    rule('context.ltf-against-htf', 'Do not trade M5 against the H1 bias, just as H1 against the D1 bias is avoided.', {
      sources: [src('webinar', 'user-pasted notes', '2024-10-18')],
    }),
    rule('context.markets-traded', 'Markets traded: forex majors and crosses (GBPUSD, EURUSD, USDCHF, USDCAD, USDJPY, NZDUSD, AUDUSD, EURCHF, EURGBP, EURJPY, GBPJPY), Bund, spot gold and silver, crude oil, DAX and SPX futures. Later posts also mention Eurostoxx 50, ES and silver.', {
      status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-09-30'), src('tweet', 'curated tweets', '2026-09-21')],
    }),
    rule('context.crypto-not-covered', 'None of the sources cover crypto. Applying the method to SOL assumes forex-style behaviour carries over.', {
      status: 'inferred', kind: 'heuristic', sources: [src('webinar', 'curated/notes-images.md', '2024-09-30')], note: 'This is the biggest untested assumption in the system.',
    }),
  ],
  evaluate: (inp, v) => {
    const d = new Date(v.asOf), wd = d.getUTCDay(), hr = d.getUTCHours();
    const session = (hr < 7 ? 'asian' : hr < 12 ? 'london' : hr < 21 ? 'ny' : 'off') as 'asian' | 'london' | 'ny' | 'off';
    const cls = v.instrument.cls;
    const flattenBy = (cls.startsWith('fx') || cls === 'metal' ? 'end-of-week' : cls === 'index' || cls === 'bund' ? 'end-of-day' : 'none') as 'none' | 'end-of-week' | 'end-of-day';
    const mode = (inp.bias.htf !== 'neutral' ? 'with-bias' : 'no-bias') as 'with-bias' | 'no-bias';
    const fridayLate = wd === 5 && hr >= 13;
    return { out: { context: { weekday: wd, hourUtc: hr, session, fridayLate, flattenBy, mode, conservativeTargets: fridayLate && mode === 'no-bias' } } };
  },
});
