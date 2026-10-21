import { defineNode } from '../core/node.js';
import { AtrPort, NewsPort } from '../core/ports.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

export default defineNode({
  id: 'news',
  title: 'News handling',
  stage: 'X',
  summary: 'Keeps resting orders from filling into red news: no fills 15 minutes either side, pull a limit if price is close to it, leave it if price is far away, and hold anything already open.',
  decideAt: 'fill_time',
  step: { n: 5, label: 'News: no fills inside red news; pull a nearby limit and put it back after' },
  inputs: { atr: AtrPort },
  outputs: { news: NewsPort },
  rules: [
    rule('news.no-fills-window', 'Red-folder news: no trading 15 minutes before or after.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-10-03')],
    }),
    rule('news.hold-open-trades', 'If already in a trade at the release, hold it. Do not lighten up or cut; keep the stop where the technicals put it.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('webinar', 'user-pasted notes', '2024-10-03')],
    }),
    rule('news.pull-limit-when-close', 'If price is close to the level at the release (about 10-20 pips on EURUSD), take the order out and put it back after the news. 2012 webinar: give the news five minutes. 2024 notes: about ten.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:18:01'), src('webinar', 'curated/notes-images.md', '2024-09-19'), src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('news.leave-it-when-far', 'If price is far away (about 60 pips on big news), do not remove the limit. At 20+ pips away the news may even be what tags you in with a retest.', {
      codable: true, sources: [src('webinar', 'curated/notes-images.md', '2024-09-19'), src('webinar', 'user-pasted notes', '2024-10-03')],
    }),
    rule('news.no-m5-through-news', 'He does not like holding an M5 trade through news.', {
      sources: [src('webinar', 'user-pasted notes', '2024-10-18')],
    }),
    rule('news.non-fx-calibration', 'Pip thresholds only make sense for forex. For other instruments this build substitutes ATR multiples (pull within 1 H1 ATR, leave beyond 3).', {
      codable: true, status: 'inferred', kind: 'heuristic', sources: [src('webinar', 'curated/notes-images.md', '2024-09-19')],
    }),
  ],
  conflicts: [{ a: 'news.pull-limit-when-close', b: 'news.no-fills-window', note: 'The 2012 webinar reinserts the order five minutes after the release; the 2024 notes say ten; the red-news window itself is fifteen minutes either side. This build uses five for "reinsert" and fifteen for "no fills".' }],
  evaluate: (inp, v, setup) => {
    const now = v.now!, reds = v.calendar.filter((e) => e.impact === 'red');
    const dts = reds.map((e) => (e.t - now.t) / 60000);
    const next = dts.filter((d) => d > 0).sort((a, b) => a - b)[0] ?? null;
    const since = dts.filter((d) => d <= 0).map((d) => -d).sort((a, b) => a - b)[0] ?? null;
    const redNow = dts.some((d) => Math.abs(d) <= P.newsWindowMin);
    const entry = setup.entry ?? setup.levelPrice;
    const dist = entry ? Math.abs(now.price - entry) : null;
    const fx = v.instrument.cls.startsWith('fx');
    const near = dist !== null && (fx ? dist / v.instrument.pip <= P.newsPullPips : dist / (inp.atr.h1 || 1) <= P.newsPullAtr);
    const far = dist !== null && (fx ? dist / v.instrument.pip >= P.newsKeepPips : dist / (inp.atr.h1 || 1) >= P.newsKeepAtr);
    let action: 'ok' | 'pull-limit' | 'keep-limit' | 'reinsert-later' = 'ok';
    if (next !== null && next <= P.newsWindowMin) action = near ? 'pull-limit' : 'keep-limit';
    else if (since !== null && since <= P.newsReinsertMin && near) action = 'reinsert-later';
    void far;
    const checks: Check[] = [{ rule: 'news.no-fills-window', pass: !redNow, detail: redNow ? `red news within ${P.newsWindowMin} minutes` : 'no red news within 15 minutes' }];
    return { out: { news: { redNow, minutesToNextRed: next, minutesSinceLastRed: since, noNewFills: redNow, action, holdIfInTrade: true } }, checks };
  },
});
