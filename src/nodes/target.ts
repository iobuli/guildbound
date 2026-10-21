import { defineNode } from '../core/node.js';
import { AtrPort, BiasPort, LevelPort, StopPort, SwingsPort, TargetPort, TuesdayPort } from '../core/ports.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

export default defineNode({
  id: 'target',
  title: 'Targets and first trouble area',
  stage: 'O',
  summary: 'Finds where the trade is likely to meet trouble (the FTA, the most recent respected level, the higher-timeframe target, or the other side of a range) and checks the reward-to-risk.',
  decideAt: 'prior_bars',
  step: { n: 7, label: 'Target: the first trouble area' },
  inputs: { stop: StopPort, level: LevelPort, swings: SwingsPort, bias: BiasPort, atr: AtrPort, tuesday: TuesdayPort },
  outputs: { target: TargetPort },
  rules: [
    rule('target.fta', 'Take profit at the First Trouble Area: the first place on the chart where price might meet trouble. Closing there is more consistent than trying to run every trade. In the 2012 webinar the example target was "the next trouble area, about a 30 pip move".', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-targets.md', '2024-10-22'), src('transcript', 'levels webinar part 1, user-pasted transcript @53:38')],
    }),
    rule('target.most-recent-over-first', 'Price tends to gravitate to the most recent level, so the first level in the way is not always the FTA. He often targets recent price action over older, more obvious levels.', {
      codable: true, kind: 'heuristic', status: 'observed', sources: [src('lesson', 'curated/taking-a-trade-targets.md', '2024-10-22'), src('webinar', 'curated/tp-selection.md', '2024-09-24'), src('journal', 'curated/lxpb-trades-passed-atr.md', '2025-04-03')],
    }),
    rule('target.ignore-tiny-swings', 'A tiny one-touch swing high is not the FTA. If it were larger or had more taps it would be.', {
      codable: true, status: 'observed', sources: [src('webinar', 'curated/tp-selection.md', '2024-09-24')], note: 'Swings under 0.5 H1 ATR deep are ignored here; the source gives no number.',
    }),
    rule('target.htf-bias-htf-target', 'With a daily bias the target is on the daily too, and an H1 level in the way is not worried about. "D1 for bias, D1 for TP."', {
      codable: true, sources: [src('webinar', 'curated/tp-selection.md', '2024-09-24')],
    }),
    rule('target.no-bias-fta-or-range', 'With no higher-timeframe bias, take profit at the FTA; with no FTA either, aim for the other side of the range. After a false break of a range, price usually goes to the other side.', {
      codable: true, sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24'), src('webinar', 'curated/tp-selection.md', '2024-09-24')],
    }),
    rule('target.inverse-last', 'Target the inverse-last level: where the market last found support (for a short) or resistance (for a long).', {
      sources: [src('webinar', 'curated/tp-selection.md', '2024-09-24')],
    }),
    rule('target.sections', 'With a daily bias he splits the take-profit across the hourly levels in the way (five levels: sell a fifth at each) and re-enters each fifth when the level flips and retests.', {
      status: 'observed', sources: [src('webinar', 'curated/tp-selection.md', '2024-10-18')],
    }),
    rule('target.minimum-rr', 'Minimum reward-to-risk is about 0.8R, and sometimes 0.5R in a high-probability setup. If the evolving R gets "ridiculously low" he closes.', {
      codable: true, sources: [src('webinar', 'user-pasted SFP notes', '2024-09-24'), src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('target.take-the-sure-thing', 'Take a smaller target when it has confluence on the left and the RR is already good enough: "take the sure bet".', {
      status: 'observed', kind: 'preference', sources: [src('journal', 'curated/lxpb-trades-passed-atr.md', '2025-01-03'), src('webinar', 'curated/tp-selection.md', '2024-09-24')],
    }),
    rule('target.look-further-left', 'Look further left than you think before picking an FTA: an older level that is still being respected can be the real one.', {
      status: 'observed', sources: [src('webinar', 'curated/tp-selection.md', '2024-09-24')],
    }),
    rule('target.far-targets-pay', 'Causal backtest: with the lookahead filters removed, win rate is about 29% and the system wants a far target, 3-5R, with a broad plateau. 1.0R loses money.', {
      status: 'backtest-reported', sources: [src('backtest', 'user-supplied forensic page', '2026-09-13')],
    }),
  ],
  conflicts: [{
    a: 'target.minimum-rr', b: 'target.far-targets-pay',
    note: 'The teacher\'s discretionary style is quick in-and-out trades near the first trouble area (0.8-1.2R). The causal backtest says the same entries only pay at 3-5R. They are different styles; do not mix their stops and targets.',
  }],
  evaluate: (inp, v, setup) => {
    const long = setup.dir === 'long', a1 = inp.atr.h1;
    const entry = setup.entry ?? inp.level.lxpbPrice ?? inp.tuesday.entry ?? setup.levelPrice ?? null;
    const empty = { out: { target: { candidates: [], chosen: null, minRr: setup.highProbability ? P.minRrHighProb : P.minRr } } };
    if (entry === null) return empty;
    const risk = inp.stop.riskDist, minRr = setup.highProbability ? P.minRrHighProb : P.minRr;
    const rr = (p: number) => (risk > 0 ? Math.abs(p - entry) / risk : 0);
    const cand = (kind: 'htf' | 'fta' | 'recent' | 'range-opposite', price: number) => ({ kind, price, rr: rr(price), viable: rr(price) >= minRr });
    const beyond = (p: number) => (long ? p > entry : p < entry);
    const h1 = inp.swings.h1.filter((s) => s.side === (long ? 'high' : 'low') && beyond(s.price) && s.depth >= P.ftaMinSwingAtr * a1);
    const out: ReturnType<typeof cand>[] = [];
    const aligned = inp.bias.htf === setup.dir;
    if (setup.style === 'tuesday-rto' && inp.tuesday.target !== null) out.push(cand('fta', inp.tuesday.target));
    const d1 = inp.swings.d1.filter((s) => s.side === (long ? 'high' : 'low') && beyond(s.price) && s.depthAtr >= 1).sort((x, y) => Math.abs(x.price - entry) - Math.abs(y.price - entry))[0];
    if (aligned && d1) out.push(cand('htf', d1.price));
    const fta = [...h1].sort((x, y) => Math.abs(x.price - entry) - Math.abs(y.price - entry))[0];
    if (fta) out.push(cand('fta', fta.price));
    const recent = [...h1].sort((x, y) => y.i - x.i)[0];
    if (recent && (!fta || recent.price !== fta.price)) out.push(cand('recent', recent.price));
    const win = v.h1.slice(-100);
    if (win.length) {
      const edge = long ? Math.max(...win.map((b) => b.h)) : Math.min(...win.map((b) => b.l));
      if (beyond(edge)) out.push(cand('range-opposite', edge));
    }
    const chosen = out.find((x) => x.kind === 'htf') ?? out.find((x) => x.kind === 'fta') ?? out.find((x) => x.kind === 'range-opposite') ?? null;
    const checks: Check[] = [];
    const tp = setup.target;
    if (tp !== undefined) checks.push({ rule: 'target.minimum-rr', pass: rr(tp) >= minRr, detail: `proposed target is ${rr(tp).toFixed(2)}R (minimum ${minRr})` });
    else if (chosen) checks.push({ rule: 'target.minimum-rr', pass: chosen.viable, detail: `${chosen.kind} target is ${chosen.rr.toFixed(2)}R (minimum ${minRr})` });
    return { out: { target: { candidates: out, chosen, minRr } }, checks };
  },
});
