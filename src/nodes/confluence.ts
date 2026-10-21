import { defineNode } from '../core/node.js';
import { AtrGatePort, AtrPort, BiasPort, ConfluencePort, LevelPort, SwingsPort } from '../core/ports.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

export default defineNode({
  id: 'confluence',
  title: 'Three-confluence rule',
  stage: 'L',
  summary: 'Every S/R trade needs 3 confluences. Each respected touch is one; the rest come from the daily bias, a passed daily ATR, or a fib retracement that lines up with the level.',
  decideAt: 'fill_time',
  step: { n: 3, label: 'Tier by touches: 3+ blind, 2 needs one confluence, 1 needs two (daily bias, fib, ATR)' },
  inputs: { level: LevelPort, bias: BiasPort, gate: AtrGatePort, atr: AtrPort, swings: SwingsPort },
  outputs: { confluence: ConfluencePort },
  rules: [
    rule('confluence.three-needed', 'Every support/resistance trade needs 3 confluences.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-1-touch-level.md', '2024-10-21'), src('lesson', 'curated/taking-a-trade-2-touch-level.md', '2024-10-21'), src('lesson', 'curated/taking-a-trade-3-touch-level.md', '2024-10-21')],
    }),
    rule('confluence.touch-tiers', '3 or more touches: trade the level blind. 2 touches: one more confluence. 1 touch: two more.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-1-touch-level.md', '2024-10-21'), src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('confluence.the-extra-factors', 'The extra confluences are: daily bias, a surpassed daily ATR, or a fib retracement.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-2-touch-level.md', '2024-10-21')],
    }),
    rule('confluence.fib-within-ten-percent-atr', 'A fib counts only if it is within 10% of the day\'s ATR from the level (ATR 100: within 10 pips; ATR 50: within 5).', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-30')],
    }),
    rule('confluence.fib-removed-past-618', 'If price pulls back further than the 61.8% fib, remove the fib: it no longer counts.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-10-05')],
    }),
    rule('confluence.fib-from-daily-swings', 'Fibs are drawn from daily swing points, only "deep key levels". He likes fibs from different areas to line up.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-10-05'), src('webinar', 'user-pasted notes', '2024-10-06')],
      note: 'This build uses the latest D1 swing with at least 1 ATR of depth as "major".',
    }),
    rule('confluence.bias-expires', 'A daily-engulfing confluence lapses if no entry is given within a couple of days. It reverts to a standard level that needs its other confluence.', {
      codable: true, sources: [src('webinar', 'curated/notes-images.md', '2024-09-10')],
    }),
    rule('confluence.bias-loosens-levels', 'With a daily bias the hourly level does not have to be great: "I just want in", but still a decent level.', {
      kind: 'preference', sources: [src('webinar', 'curated/notes-images.md', '2024-09-08'), src('webinar', 'curated/tp-selection.md', '2024-09-24')],
    }),
    rule('confluence.tiers-from-the-transcript', 'The summary he gives twice: 3 or more touches, trade it blind (just a limit order with a stop and a target). 2 touches, one factor of confluence. 1 touch, two factors. The less significant the level, the more you need to tell you price will bounce.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @23:35'), src('transcript', 'levels webinar part 1, user-pasted transcript @24:25'), src('transcript', 'levels webinar part 1, user-pasted transcript @56:12'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:15:09'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:16:03')],
    }),
    rule('confluence.bias-is-a-bar-pattern', 'The daily bias used as a confluence must be a bar pattern on the daily chart: a pinbar, a swing failure (breaks a swing low and closes above, or breaks a swing high and closes below) or an engulfing candle. "It\'s in a strong uptrend" is not enough.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @35:16'), src('transcript', 'levels webinar part 1, user-pasted transcript @36:03'), src('transcript', 'levels webinar part 1, user-pasted transcript @37:47')],
    }),
    rule('confluence.only-three-factors', 'There are three factors of confluence: a daily bias, a Fibonacci level, and the market at or past its ATR. A moving average was mentioned in passing as an example of "something else" but is not one of the three.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @25:07'), src('transcript', 'levels webinar part 1, user-pasted transcript @34:09'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:16:11')],
    }),
    rule('confluence.fib-38-50-61-only', 'Only the 38%, 50% and 61.8% fibs are used. The 78% is not.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:20:30'), src('transcript', 'levels webinar part 1, user-pasted transcript @1:19:02')],
    }),
    rule('confluence.fib-from-major-daily-swing', 'Draw the fib from a major swing point, preferably on the daily chart rather than the hourly, from the low to the highest point before the market began its turn.', {
      codable: true, sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @1:16:35'), src('transcript', 'levels webinar part 1, user-pasted transcript @47:52')],
    }),
    rule('confluence.level-gets-the-order', 'When the fib and the ATR line sit close to the level, the order goes at the LEVEL, because he attaches the most importance to levels. The stop goes beyond the fib and the ATR.', {
      sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @49:09')],
    }),
    rule('confluence.worked-examples', 'Silver: two-touch level + daily bias (pinbar that is also a swing failure) = tradeable. EURCHF: two-touch level + ATR (16 pips below) + 61% fib on the same spot = "perfect confluence". Silver neckline retest: two-touch + 38% fib = tradeable, but skipped because price bounced just before it.', {
      status: 'observed', sources: [src('transcript', 'levels webinar part 1, user-pasted transcript @33:03'), src('transcript', 'levels webinar part 1, user-pasted transcript @38:11'), src('transcript', 'levels webinar part 1, user-pasted transcript @48:27'), src('transcript', 'levels webinar part 1, user-pasted transcript @57:37')],
    }),
  ],
  evaluate: (inp, v, setup) => {
    const touches = inp.level.touches, dir = setup.dir;
    const biasOn = inp.bias.htf === dir && (inp.bias.d1.fresh || inp.bias.w1.fresh);
    const atrOn = inp.gate.supports[dir];
    // fib from the latest major D1 swing leg
    let fibOn = false, detail = 'no fib leg found';
    const sw = inp.swings.d1.filter((s) => s.depthAtr >= 1);
    const centre = setup.levelPrice ?? inp.level.center;
    const d1 = v.d1;
    if (sw.length && d1.length && inp.atr.d1 > 0 && centre) {
      const wantHigh = dir === 'long'; // long: retracement of an up-leg from a major low to a major high
      const ext = [...sw].reverse().find((s) => s.side === (wantHigh ? 'high' : 'low'));
      const base = ext && [...inp.swings.d1].reverse().find((s) => s.side === (wantHigh ? 'low' : 'high') && s.i < ext.i);
      if (ext && base) {
        const hi = wantHigh ? ext.price : base.price, lo = wantHigh ? base.price : ext.price, span = hi - lo;
        const limit = wantHigh ? hi - P.fibMaxRetrace * span : lo + P.fibMaxRetrace * span;
        const since = d1.filter((b) => b.t > ext.t);
        const extreme = wantHigh ? Math.min(...since.map((b) => b.l), v.now!.price) : Math.max(...since.map((b) => b.h), v.now!.price);
        const valid = wantHigh ? extreme > limit : extreme < limit;
        const hit = P.fibLevels.map((r) => ({ r, p: wantHigh ? hi - r * span : lo + r * span })).find((f) => Math.abs(centre - f.p) <= P.fibProximityOfD1Atr * inp.atr.d1);
        fibOn = valid && !!hit;
        detail = !valid ? `fib removed: pulled back past ${P.fibMaxRetrace}` : hit ? `level is within 10% of D1 ATR of the ${hit.r} fib (${hit.p.toFixed(2)})` : 'no fib level close enough';
      }
    }
    const score = Math.min(touches, P.confluenceNeeded) + +biasOn + +atrOn + +fibOn;
    const meets = score >= P.confluenceNeeded;
    const checks: Check[] = [{ rule: 'confluence.three-needed', pass: meets, detail: `${Math.min(touches, 3)} touch(es) + bias ${biasOn} + ATR ${atrOn} + fib ${fibOn} = ${score}/3` }];
    return { out: { confluence: { touches, factors: { bias: biasOn, atr: atrOn, fib: fibOn }, fibDetail: detail, score, needed: Math.max(0, P.confluenceNeeded - score), meets, blind: touches >= 3 } }, checks };
  },
});
