import { defineNode } from '../core/node.js';
import { ContextPort, ManagementPort, StopPort, TargetPort } from '../core/ports.js';
import { rule, src } from '../core/rule.js';
import { P } from '../core/params.js';
import type { Check } from '../core/types.js';
import type { z } from 'zod';

type Action = z.infer<typeof ManagementPort>['actions'][number];

export default defineNode({
  id: 'management',
  title: 'Trade management',
  stage: 'M',
  summary: 'Once a position is open, tests the exit and trailing rules against the bars since the fill: evolving R, broken-entry retests, closes beyond the first test, thrust-candle trails, and whether adding is allowed.',
  decideAt: 'post_fill',
  step: { n: 10, label: 'Management: exits, trails, adds' },
  inputs: { stop: StopPort, target: TargetPort, context: ContextPort },
  outputs: { mgmt: ManagementPort },
  rules: [
    rule('management.evolving-r', 'RR evolves through a trade. A 1R trade that is 0.5R half way through is still held; only when it gets "ridiculously low" (under 0.5R) does he consider closing. "Don\'t be a prick for a tick."', {
      codable: true, sources: [src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('management.broken-entry-retest-exit', 'If price closes under your entry, retests the entry from below as resistance, and then closes beneath the initial low, exit the trade.', {
      codable: true, sources: [src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('management.close-beyond-first-test', 'If the trade is a short and an H1 close comes above the highs of the first test of your resistance, you must close it. The same logic applies to a long.', {
      codable: true, sources: [src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('management.trail-beyond-thrust', 'When price breaks a level in your favour, trail the stop beyond the thrust candle that caused the break. These stops still have to make sense: no trailing directly into a level. If it fails, go back another candle.', {
      codable: true, kind: 'heuristic', sources: [src('webinar', 'curated/trade-management.md', '2024-09-24'), src('webinar', 'curated/trade-management.md', '2024-11-05')],
    }),
    rule('management.squeeze-point', 'After the second return to the entry level and price curving away, he trails beyond the "squeeze point": the high (low) price would reach if it reacted off the level again. A third comeback is unlikely to go well.', {
      status: 'observed', sources: [src('webinar', 'curated/trade-management.md', '2024-09-23'), src('webinar', 'curated/trade-management.md', '2024-09-25')],
    }),
    rule('management.is-this-trade-done', '"Is this trade done?" If price came very close to target and then fell back to the entry, it is over: take what is there. If TP is hit without a fill, close the order at once.', {
      sources: [src('webinar', 'curated/trade-management.md', '2024-09-23'), src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('management.rejection-at-fta-exit', 'Exit on a rejection candle at the first trouble area. Take the profits you have and do not hang around hoping the market comes back to your levels.', {
      status: 'observed', sources: [src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('management.trapped-exit-at-level', 'If you are trapped in a trade that clearly will not reach target, get out at the level that is trapping you, with a limit for as small a loss as possible.', {
      sources: [src('webinar', 'curated/trade-management.md', '2024-11-04')],
    }),
    rule('management.opposite-sweep-exit', 'Get out at a bullish sweep when you are short from a three-tap level and the S/R of the sweep is high: why stay when you are about 80% sure price will pump.', {
      status: 'observed', sources: [src('webinar', 'curated/trade-management.md', '2024-10-29')],
    }),
    rule('management.flatten-by-close', 'Forex positions are closed at the end of the week; FTSE and Bund trades at the end of the day, partly to avoid being gapped out.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('management.add-when-confirming', 'Add to a winner only when the market is confirming, the addition fits the plan, the upside is still asymmetric and risk stays controlled.', {
      codable: true, sources: [src('tweet', 'curated tweets', '2026-06-03'), src('tweet', 'curated tweets', '2026-05-01')],
      note: 'Coded as: at least 1R in profit and evolving R still at least 1.',
    }),
    rule('management.macd-divergence-exit', 'He has closed a trade on MACD divergence above a key level.', {
      status: 'observed', sources: [src('webinar', 'curated/trade-management.md', '2024-09-23')], note: 'MACD is not computed in this build.',
    }),
    rule('management.hold-through-news', 'Hold open trades through news; do not cut or lighten.', {
      sources: [src('webinar', 'user-pasted notes', '2024-10-03')],
    }),
  ],
  evaluate: (inp, v, setup) => {
    const c = v.h1, n = c.length, long = setup.dir === 'long';
    const hold = (reason: string) => ({ out: { mgmt: { evolvingR: null, canAdd: false, actions: [{ action: 'hold' as const, reason, rule: 'management.is-this-trade-done' }] } } });
    const entry = setup.entry, stop = inp.stop.stop, tp = setup.target ?? inp.target.chosen?.price ?? null;
    if (entry === undefined || stop === null || setup.fillAt === undefined || n === 0) return hold('needs entry, stop and fill time to manage a position');
    const fi = c.findIndex((b) => b.t >= Math.floor(setup.fillAt! / v.period) * v.period);
    if (fi < 0) return hold('fill time is not in the data');
    const last = c[n - 1]!, price = last.c, risk0 = Math.abs(entry - stop);
    const actions: Action[] = [], checks: Check[] = [];
    const riskNow = long ? price - stop : stop - price;
    if (riskNow <= 0) actions.push({ action: 'close', reason: 'price is at or beyond the stop', rule: 'management.is-this-trade-done' });
    const reward = tp === null ? null : long ? tp - price : price - tp;
    const evolving = tp !== null && riskNow > 0 ? Math.max(0, reward!) / riskNow : null;
    if (evolving !== null && evolving < P.evolvingRrClose) actions.push({ action: 'close', reason: `evolving R is ${evolving.toFixed(2)}, under ${P.evolvingRrClose}`, rule: 'management.evolving-r' });
    // closes under entry -> retest from below -> closes beyond the initial low
    let j1 = -1;
    for (let j = fi; j < n; j++) if (long ? c[j]!.c < entry : c[j]!.c > entry) { j1 = j; break; }
    if (j1 >= 0) {
      const initial = long ? Math.min(...c.slice(fi, j1 + 1).map((b) => b.l)) : Math.max(...c.slice(fi, j1 + 1).map((b) => b.h));
      let j2 = -1;
      for (let j = j1 + 1; j < n; j++) if (long ? c[j]!.h >= entry : c[j]!.l <= entry) { j2 = j; break; }
      if (j2 >= 0 && c.slice(j2 + 1).some((b) => (long ? b.c < initial : b.c > initial)))
        actions.push({ action: 'close', reason: 'closed beyond entry, retested it, then closed beyond the initial extreme', rule: 'management.broken-entry-retest-exit' });
    }
    const first = c[fi]!;
    if (c.slice(fi + 1).some((b) => (long ? b.c < first.l : b.c > first.h))) actions.push({ action: 'close', reason: 'an H1 close went beyond the extreme of the first test', rule: 'management.close-beyond-first-test' });
    const progress = risk0 > 0 ? (long ? price - entry : entry - price) / risk0 : 0;
    // trail beyond the thrust candle: largest favourable-closing bar of the last 8
    const recent = c.slice(Math.max(fi + 1, n - 8)).filter((b) => (long ? b.c > b.o : b.c < b.o));
    const thrust = [...recent].sort((a, b) => b.h - b.l - (a.h - a.l))[0];
    if (thrust && progress >= 0.5) {
      const cand = long ? thrust.l - 0.05 * (thrust.h - thrust.l) : thrust.h + 0.05 * (thrust.h - thrust.l);
      if (long ? cand > stop && cand < price : cand < stop && cand > price)
        actions.push({ action: 'trail', reason: `trail to ${cand.toFixed(4)}, just beyond the largest thrust candle; check it does not sit directly in front of a level`, rule: 'management.trail-beyond-thrust' });
    }
    const wd = new Date(v.asOf).getUTCDay(), hr = new Date(v.asOf).getUTCHours();
    if ((inp.context.flattenBy === 'end-of-week' && wd === 5 && hr >= 20) || (inp.context.flattenBy === 'end-of-day' && hr >= 20))
      actions.push({ action: 'close', reason: `flatten by ${inp.context.flattenBy}`, rule: 'management.flatten-by-close' });
    const canAdd = progress >= 1 && (evolving === null || evolving >= 1);
    checks.push({ rule: 'management.add-when-confirming', pass: canAdd, detail: `${progress.toFixed(2)}R in profit, evolving R ${evolving === null ? 'n/a' : evolving.toFixed(2)}` });
    if (!actions.length) actions.push({ action: 'hold', reason: 'no exit or trail rule triggered', rule: 'management.is-this-trade-done' });
    return { out: { mgmt: { evolvingR: evolving, canAdd, actions } }, checks };
  },
});
