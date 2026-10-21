import { defineNode } from '../core/node.js';
import { AtrGatePort, AtrPort } from '../core/ports.js';
import { P } from '../core/params.js';
import { dayStart } from '../core/series.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

export default defineNode({
  id: 'atr-gate',
  title: 'Daily ATR gate',
  stage: 'O',
  summary: 'Measures how far price has travelled from the HIGH or LOW OF THE DAY, in daily ATRs. If it has fallen an ATR from the day\'s high, selling is vetoed and buying gains a confluence (and the reverse).',
  step: { n: 3.2, label: 'Confluence: has the market already moved its daily ATR?' },
  decideAt: 'fill_time',
  inputs: { atr: AtrPort },
  outputs: { gate: AtrGatePort },
  rules: [
    rule('atr-gate.measure-from-the-days-extreme', 'The ATR level is measured from the HIGH (or low) OF THE DAY: with a 123-pip ATR and a day high of 128.31, the market has reached its ATR at 127.08. If it falls 101 pips from the high of the day into the level, that counts.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @45:00'), src('transcript', 'levels webinar part 1, user-pasted transcript @46:09'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:19:57')],
    }),
    rule('atr-gate.surpassed-atr-is-confluence', 'A market that is at or past its daily ATR coming into the level is a confluence: it has moved what it normally moves, so people take profits there and it is likely to turn.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @34:41'), src('transcript', 'levels webinar part 1, user-pasted transcript @45:17'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:16:27'), src('lesson', 'curated/taking-a-trade-1-touch-level.md', '2024-10-21')],
    }),
    rule('atr-gate.no-same-direction', 'Do not trade in the same direction once the market has passed its daily ATR. "70 points down on the day and the ATR is 70, I\'m not selling, even on a rounded retest of the mother of all levels."', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-10-03')],
    }),
    rule('atr-gate.close-enough', 'Being "close enough" to the ATR counts. In the EURCHF example the ATR line sat 16 pips below the level and still counted, because it matched the lower part of the level zone.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('transcript', 'levels webinar part 1, user-pasted transcript @47:01')], note: 'This build treats 90% of the ATR as close enough; the sources give no number.',
    }),
    rule('atr-gate.atr-trades', 'In the passed-ATR trades of the later journal he cares less about a weekly level sitting in the way, and a daily level is not needed if the H1 level is clean.', {
      status: 'observed', kind: 'heuristic', sources: [src('journal', 'curated/lxpb-trades-passed-atr.md', '2025-01-03'), src('journal', 'curated/lxpb-trades-passed-atr.md', '2026-02-09')],
    }),
  ],
  evaluate: (inp, v, setup) => {
    const now = v.now!, ds = dayStart(now.t);
    const today = v.h1.filter((b) => b.t >= ds);
    const hi = Math.max(now.dayOpen, now.price, ...today.map((b) => b.h)), lo = Math.min(now.dayOpen, now.price, ...today.map((b) => b.l));
    const a = inp.atr.d1;
    const fromHigh = a > 0 ? (hi - now.price) / a : 0, fromLow = a > 0 ? (now.price - lo) / a : 0;
    const down = fromHigh >= P.atrPassed, up = fromLow >= P.atrPassed;
    const moved = fromLow >= fromHigh ? fromLow : -fromHigh;
    const checks: Check[] = [{
      rule: 'atr-gate.no-same-direction',
      pass: setup.dir === 'long' ? !up : !down,
      detail: `price is ${fromHigh.toFixed(2)} ATR below the day's high and ${fromLow.toFixed(2)} ATR above its low; ${setup.dir === 'long' ? (up ? 'buying after an ATR up-move is vetoed' : 'ok to buy') : (down ? 'selling after an ATR down-move is vetoed' : 'ok to sell')}`,
    }];
    return { out: { gate: { movedAtr: moved, fromHighAtr: fromHigh, fromLowAtr: fromLow, passedUp: up, passedDown: down, blocks: { long: up, short: down }, supports: { long: down, short: up } } }, checks };
  },
});
