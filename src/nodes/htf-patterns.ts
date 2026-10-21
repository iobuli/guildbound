import { defineNode } from '../core/node.js';
import { rule, src } from '../core/rule.js';

const J = (f: string, d: string) => src('journal', `curated/${f}`, d);

export default defineNode({
  id: 'htf-patterns',
  title: 'Higher-timeframe patterns from the journal',
  stage: 'D',
  summary: 'Patterns the journal uses for direction and location that are not defined precisely enough to code: pin failure, hammer return, equal-swing targets, drift of doom.',
  decideAt: 'prior_bars',
  inputs: {},
  outputs: {},
  rules: [
    rule('htf-patterns.pin-failure', 'A pinbar that fails (the daily or weekly closes beyond the pin\'s head) is a direction signal the other way, used with an H1 LXPB at the head level.', {
      status: 'observed', sources: [J('lxpb-trades-pattern-failure.md', '2024-12-21'), J('lxpb-trades-pattern-failure.md', '2026-05-19')],
    }),
    rule('htf-patterns.return-to-hammer', 'After a weekly hammer or pin, the trade is a return to the hammer\'s head, entered at the H1 LXPB confluent with it.', {
      status: 'observed', sources: [J('lxpb-trades-return-to-hammer.md', '2024-12-22'), J('lxpb-trades-return-to-hammer.md', '2025-01-06')],
    }),
    rule('htf-patterns.htf-sponsorship', 'A level backed by a higher-timeframe level ("HTF sponsorship") can be a no-brainer even if the weekly low is not cleanly closed under, and the target may then be the next HTF level.', {
      status: 'observed', sources: [J('lxpb-trades-return-to-hammer.md', '2025-05-02'), J('lxpb-trades-htf-level.md', '2026-03-03')],
    }),
    rule('htf-patterns.stop-then-add', 'Failure patterns often give a second entry on a second LXPB toward the original target, now with no FTA in the way.', {
      status: 'observed', sources: [J('lxpb-trades-pattern-failure.md', '2026-02-06'), J('lxpb-trades-pattern-failure.md', '2026-06-24')],
    }),
    rule('htf-patterns.undefined-terms', 'KPI, HoH, LoH and IWL are used without definition. Plausible readings: KPI = key price inflection, HoH / LoH = the head / low of a hammer or pin, IWL = inside-week low. None of these is confirmed.', {
      status: 'inferred', kind: 'heuristic', sources: [J('lxpb-trades-engulfing.md', '2026-05-03'), J('lxpb-trades-return-to-hammer.md', '2025-01-06')],
    }),
    rule('htf-patterns.eod-spike-day-after', 'After a spike candle, look for a short the next day; one of the best places to get in is the previous day\'s high. After a big down day a full reversal the next day is suspect for shorts.', {
      status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-11-02'), src('webinar', 'curated/notes-images.md', '2024-11-09')],
    }),
    rule('htf-patterns.pin-high-close', 'Price closing below the high of a pin is usually a good sign it will go to the pin\'s low.', {
      status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-11-03')],
    }),
    rule('htf-patterns.key-levels-shallow-pullbacks', 'When key levels are near (many highs plus an all-time high), deep pullbacks are rare and shallow ones to the LXPB are more common.', {
      kind: 'heuristic', status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-11-02')],
    }),
  ],
});
