import { defineNode } from '../core/node.js';
import { AtrPort, LevelPort, SfpPort, StopPort, SwingsPort, TuesdayPort } from '../core/ports.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

export default defineNode({
  id: 'stop',
  title: 'Stop placement',
  stage: 'E',
  summary: 'Suggests where the stop belongs (beyond the thrust candle, never into a level, wider on crosses) and audits a stop you propose against the same rules.',
  decideAt: 'prior_bars',
  step: { n: 6, label: 'Stop: beyond the thrust candle, where you would still be bullish' },
  inputs: { level: LevelPort, atr: AtrPort, swings: SwingsPort, sfp: SfpPort, tuesday: TuesdayPort },
  outputs: { stop: StopPort },
  rules: [
    rule('stop.beyond-thrust', 'On an S/R trade the stop goes beyond the thrust candle (the candle that broke the level).', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-30'), src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('stop.never-into-a-level', 'The stop may not sit directly in front of ATR, an S/R level or the next key fib, where price could squeeze to it and reverse. Move it to a candle further left (beyond the level).', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-30'), src('webinar', 'curated/notes-images.md', '2024-09-23'), src('journal', 'curated/lxpb-trades-passed-atr.md', '2024-12-29')],
      note: 'Read here as: if a swing of the right side lies within 0.3 H1 ATR beyond the stop, place the stop just beyond that swing.',
    }),
    rule('stop.daily-long-hourly-under', 'Long off a daily level with an hourly level just underneath: the stop goes under the hourly level, so it is not hit while price finds support there.', {
      sources: [src('webinar', 'user-pasted notes', '2024-09-25')],
    }),
    rule('stop.second-level-squeeze', 'He likes a second, minor level between entry and stop as a second layer of protection if price squeezes.', {
      codable: true, kind: 'preference', status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-09-10')],
    }),
    rule('stop.crosspairs-wider', 'On cross pairs the stop goes further out (above the S/R level rather than just above the thrust candle), because they squeeze more.', {
      codable: true, status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-09-23')], note: 'The 0.25 ATR extra is the builder\'s number.',
    }),
    rule('stop.zone-needs-whole-zone', 'If there is no clear thrust candle through the whole level zone, move the stop one candle to the left to be safe.', {
      status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-10-28')],
    }),
    rule('stop.protected-by-htf', 'He likes the stop to be protected by higher-timeframe levels. In one trade the stop could not go above the daily level and was protected only by the weekly one.', {
      status: 'observed', kind: 'preference', sources: [src('journal', 'curated/lxpb-trades-passed-atr.md', '2025-04-22'), src('journal', 'curated/lxpb-trades-passed-atr.md', '2025-11-13')],
    }),
    rule('stop.risky-stop-is-fine-if-idea-is-wrong', 'A stop over double lows is "risky", but if price gets there the idea is wrong anyway.', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-htf-level.md', '2026-02-19')],
    }),
    rule('stop.sfp-atr', 'SFP trades: stop at the sweep extreme plus the 24-period ATR. He also uses a tighter "gap" technique when it still clears the old high.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('webinar', 'curated/advanced-sfp.md', '2024-09-24')], note: 'The "gap SL technique" is mentioned but never described.',
    }),
    rule('stop.just-beyond-not-one-tick', 'For failed-breakdown trades on the M5 Bund: the stop goes just under the lows, "not 1 tick under, but just under somewhere"; no ATR or thrust candle needed.', {
      sources: [src('webinar', 'curated/notes-images.md', '2024-09-25')],
    }),
    rule('stop.min-distance', 'A minimum stop of 0.05% of price is used in the backtest.', {
      codable: true, status: 'backtest-reported', sources: [src('backtest', 'user-supplied twelve-bar page')],
    }),
    rule('stop.grace-two-bars', 'A 2-bar grace window at a 2x stop is now worth only about 0.05 Sharpe at a 4R target: keep it, do not lean on it.', {
      status: 'backtest-reported', sources: [src('backtest', 'user-supplied forensic page', '2026-09-13')],
    }),
    rule('stop.would-i-still-be-bullish', 'The stop question: am I stopping out into an area where I would STILL be bullish? Resistance flips to support, so look for resistance just beyond the level that price might squeeze to in the worst case, and put the stop just beneath it. Draw the stop and look at what you would be stopped out into.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:00:04'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:00:37'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:06:54')],
    }),
    rule('stop.as-tight-as-the-level-allows', 'Keep the stop as close to market as you can, but no tighter than the squeeze point. There is usually no need for a very wide stop if the level was chosen correctly, and these trades should not show much drawdown.', {
      sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:00:20'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:01:42'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:01:57')],
    }),
    rule('stop.candle-proxy', 'The usual stop is beneath the last candle that OPENED below and CLOSED above the level (the thrust candle). If that puts the stop into an area price could squeeze to, move to a candle further left and check again.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:01:00'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:01:18'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:07:09')],
    }),
    rule('stop.cross-definition', 'A "cross" is any pair without the USD in it. He uses a wide stop on crosses because they are not clean, and tells people to be careful with them.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:06:30'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:07:39'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:08:40')],
    }),
  ],
  evaluate: (inp, v, setup) => {
    const c = v.h1, a = inp.atr.h1, long = setup.dir === 'long', buf = P.stopBufferAtr * a;
    let suggested: number | null = null, method = 'none', shifted = false;
    let entry: number | null = setup.entry ?? null;
    if (setup.style === 'tuesday-rto') {
      suggested = inp.tuesday.stop; entry = entry ?? inp.tuesday.entry; method = 'other side of the pinbar';
    } else if (setup.style === 'sfp') {
      const s = inp.sfp.h1.find((x) => x.dir === setup.dir && x.valid) ?? inp.sfp.h1.find((x) => x.dir === setup.dir);
      if (s) { suggested = s.stopAtr; entry = entry ?? s.closePrice; method = 'sweep extreme + ATR24'; }
    } else if (inp.level.found && inp.level.breakIdx !== null) {
      entry = entry ?? inp.level.lxpbPrice ?? setup.levelPrice ?? null;
      const th = c[inp.level.breakIdx]!;
      let cand = long ? th.l - buf : th.h + buf;
      method = 'beyond the thrust candle';
      const win = P.stopBlockerAtr * a;
      const near = inp.swings.h1.filter((s) => s.side === (long ? 'low' : 'high') && (long ? s.price < cand && s.price >= cand - win : s.price > cand && s.price <= cand + win));
      if (near.length) {
        const b = long ? Math.min(...near.map((s) => s.price)) : Math.max(...near.map((s) => s.price));
        cand = long ? b - buf : b + buf; shifted = true; method = 'beyond the thrust candle, moved past a nearby level';
      }
      if (v.instrument.cls === 'fx-cross') { cand += (long ? -1 : 1) * P.crossPairBufferAtr * a; method += ' + cross-pair buffer'; }
      suggested = cand;
    }
    const stop = setup.stop ?? suggested;
    const riskDist = entry !== null && stop !== null ? Math.abs(entry - stop) : 0;
    const sideOk = entry !== null && stop !== null && (long ? stop < entry : stop > entry);
    const ok = sideOk && entry !== null && riskDist / entry >= P.minStopPct;
    const squeeze = entry !== null && stop !== null && inp.swings.h1.some((s) => s.side === (long ? 'low' : 'high') && (long ? s.price < entry! && s.price > stop! : s.price > entry! && s.price < stop!));
    const checks: Check[] = [];
    if (setup.stop !== undefined && suggested !== null)
      checks.push({ rule: setup.style === 'sfp' ? 'stop.sfp-atr' : 'stop.beyond-thrust', pass: long ? setup.stop <= suggested : setup.stop >= suggested, detail: `proposed ${setup.stop} vs suggested ${suggested.toFixed(4)} (${method})` });
    if (stop !== null) checks.push({ rule: 'stop.min-distance', pass: ok, detail: sideOk ? `risk is ${(100 * riskDist / (entry ?? 1)).toFixed(3)}% of price` : 'stop is on the wrong side of entry' });
    if (entry !== null && stop !== null) checks.push({ rule: 'stop.second-level-squeeze', pass: squeeze ? true : null, detail: squeeze ? 'a second level sits between entry and stop' : 'no second level between entry and stop (preference only)' });
    return { out: { stop: { stop, suggested, method, shifted, riskDist, ok, squeezeLevel: squeeze } }, checks };
  },
});
