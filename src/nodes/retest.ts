import { defineNode } from '../core/node.js';
import { AtrPort, BiasPort, LevelPort, RetestPort } from '../core/ports.js';
import { P } from '../core/params.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';

const T = (t: string) => src('transcript', `levels webinar part 1, user-pasted transcript @${t}`);

export default defineNode({
  id: 'retest',
  title: 'Retest quality and space',
  stage: 'L',
  summary: 'Judges the return to a broken level: was there space after the break (and not the immediate retest), did price come in with momentum, and has the order been swerved (price ran away without filling it).',
  decideAt: 'prior_bars',
  step: { n: 4, label: 'Space: did price break and keep going, and is this not the immediate retest?' },
  inputs: { level: LevelPort, atr: AtrPort, bias: BiasPort },
  outputs: { retest: RetestPort },
  rules: [
    rule('retest.needs-space', 'There must be space between the break and the retest: price breaks, closes beyond the level on the hourly, and KEEPS GOING. The higher it keeps going, the more space; the example shown is "about the minimum" he wants.', {
      sources: [T('25:30'), T('28:00')], note: 'He says he cannot quantify it ("never been able to say you need 10 bars"). The two coded tests below are proxies.',
    }),
    rule('retest.never-immediate', 'Never trade the immediate retest (the "buck kiss": the first close beyond the level followed by a bid). It is not reliable on the hourly, and he does not take it even when there is a daily bias.', {
      codable: true, sources: [T('26:03'), T('39:16')],
    }),
    rule('retest.needs-space-in-time', 'As a rough proxy for space: 5 bars is not enough and an 11-candle retest is "just about enough", roughly half a day before an H1 entry.', {
      codable: true, status: 'observed', kind: 'heuristic', sources: [src('webinar', 'curated/notes-images.md', '2024-09-16')],
    }),
    rule('retest.needs-space-in-price', 'Price must travel at least 0.5 x ATR clear of the level before a retest counts. Described as the single most important rule in the system, and it uses only bars that closed before the fill.', {
      codable: true, status: 'backtest-reported', sources: [src('backtest', 'user-supplied forensic page', '2026-09-13')],
    }),
    rule('retest.bias-relaxes-space', 'With a daily bias you do not need to be obsessed with the amount of space, but you still want the break to have happened and price to round back to the area.', {
      codable: true, sources: [T('38:49'), src('webinar', 'curated/notes-images.md', '2024-09-08')],
    }),
    rule('retest.clean-break-and-go', 'The nicest trades break through and just keep going. He does not like any interaction with the level after the break: not break, retest, go, and come back.', {
      sources: [T('29:30')],
    }),
    rule('retest.bounce-just-before-the-level', 'If price bounced just before the level (missed by a few ticks), did not fill the order, and came straight back, do not take the trade: the market is not done with that level.', {
      sources: [T('28:49'), T('59:00')],
    }),
    rule('retest.momentum-into-level', 'He likes momentum INTO the level. Buying a level the market is falling into hard is safer than buying one it drifts into slowly, though it is not a must. Reasons: pros take profits quickly after a fast drop, amateurs hold, and a sharp move can bounce like an elastic band.', {
      codable: true, kind: 'preference', sources: [T('26:41'), T('1:09:15'), T('1:10:37')],
    }),
    rule('retest.drift-in-asia-is-fine', 'A drift into the level is not a problem in the Asian session, where price drifts anyway. In London or the US session a drift would concern him.', {
      kind: 'preference', sources: [T('1:12:04')],
    }),
    rule('retest.rounded-over-sharp', 'A rounded retest is preferred over a sharp V-shaped one.', {
      codable: true, kind: 'preference', sources: [src('webinar', 'user-pasted notes', '2024-09-24'), src('lesson', 'curated/taking-a-trade-3-touch-level.md', '2024-10-21')],
    }),
    rule('retest.missed-the-level-half-way', 'If the order is missed and price bounces more than half way to the target, pull the order (a 10-15 pip bounce alone does not). The bounce creates a swing point, stops accumulate beyond it, and buying back there means buying as people stop out.', {
      codable: true, sources: [T('53:21'), T('54:04'), src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('retest.swerve-exception', 'Exception: with a three-touch level and a daily bias he leaves the order in through a swerve, because the daily target is far away.', {
      codable: true, status: 'observed', sources: [src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
    rule('retest.four-swerves', 'Four swerves of the level should cause the long to be cancelled.', {
      codable: true, status: 'observed', sources: [src('webinar', 'curated/trade-management.md', '2024-09-30')],
    }),
    rule('retest.naked', 'A level is "naked" while it has not been retested on the H1 since the break.', {
      codable: true, sources: [src('tweet', 'curated tweets', '2022-10-21'), src('tweet', 'curated tweets', '2026-05-19')],
    }),
    rule('retest.it-can-take-a-long-time', 'The market sometimes takes a long while to come back. It is still the same breakout; the level does not expire because time has passed.', {
      sources: [T('18:40'), T('19:17')],
    }),
    rule('retest.one-in-ten-misses', 'With pinpoint levels, about 1 order in 10 never fills because the level was too accurate. That is accepted.', {
      kind: 'heuristic', sources: [src('webinar', 'curated/trade-management.md', '2024-09-24')],
    }),
  ],
  conflicts: [
    {
      a: 'retest.momentum-into-level', b: 'retest.rounded-over-sharp',
      note: 'The transcript says he wants price to come into the level HARD; the 2024 notes say he prefers a ROUNDED retest over a sharp V. A hard move in and a rounded retest pull opposite ways. Both are preferences, not vetoes, so the node reports each and decides nothing.',
    },
    {
      a: 'retest.needs-space', b: 'retest.needs-space-in-price',
      note: 'The trader says space cannot be quantified and shows a chart. The 11-bar and 0.5 ATR figures are proxies from a note-taker and a backtest page; the same backtest material also reports that waiting 1 bar beat waiting 12. Treat the time proxy as unproven.',
    },
  ],
  evaluate: (inp, v, setup) => {
    const lv = inp.level, c = v.h1, n = c.length, long = setup.dir === 'long';
    const none = { retested: false, barsBreakToRetest: null, excursionAtr: 0, spaceOk: false, shape: 'none' as const, approachAtr: 0, momentum: 'none' as const, tooSoon: false, rallyAway: null, swerves: null, cancelOrder: false };
    if (!lv.found || lv.breakIdx === null) return { out: { retest: none } };
    const bi = lv.breakIdx, a = P.levelScaleD1 ? inp.atr.d1 : inp.atr.h1;
    let ri = -1;
    for (let j = bi + 1; j < n; j++) if (long ? c[j]!.l <= lv.zoneHi : c[j]!.h >= lv.zoneLo) { ri = j; break; }
    const retested = ri >= 0, end = retested ? ri : n;
    let exc = 0;
    for (let j = bi; j < end; j++) exc = Math.max(exc, long ? c[j]!.h - lv.zoneHi : lv.zoneLo - c[j]!.l);
    const excAtr = a > 0 ? exc / a : 0, bars = retested ? ri - bi : n - bi;
    const biasAligned = inp.bias.htf === setup.dir;
    const tooSoon = retested && bars < P.retestMinBarsHard;
    const timeOk = bars >= P.retestMinBars || biasAligned;
    const spaceOk = excAtr >= P.retestMinExcursionAtr && timeOk && !tooSoon;
    let shape: 'rounded' | 'sharp' | 'none' = 'none';
    if (retested) {
      const lead = c.slice(Math.max(bi + 1, ri - 8), ri);
      shape = lead.length >= 6 && lead.every((b) => b.h - b.l <= 1.5 * a) ? 'rounded' : 'sharp';
    }
    // momentum into the level: net move toward it over the last few bars before contact (or before now, if not yet touched)
    const stop = retested ? ri : n, win = c.slice(Math.max(bi + 1, stop - P.momentumBars), stop);
    const net = win.length ? (long ? win[0]!.o - win[win.length - 1]!.c : win[win.length - 1]!.c - win[0]!.o) : 0;
    const approachAtr = a > 0 ? net / a : 0;
    const momentum = !win.length ? 'none' as const : approachAtr >= P.momentumAtr ? 'hard' as const : 'drift' as const;
    let rally: boolean | null = null, swerves: number | null = null;
    if (setup.entry !== undefined && setup.target !== undefined && setup.placedAt !== undefined) {
      const after = c.filter((b) => b.t >= setup.placedAt!);
      const filled = after.some((b) => (long ? b.l <= setup.entry! : b.h >= setup.entry!));
      const dist = Math.abs(setup.target - setup.entry);
      let fav = 0, sw = 0, lastSw = -99;
      after.forEach((b, k) => {
        fav = Math.max(fav, long ? b.h - setup.entry! : setup.entry! - b.l);
        const gap = long ? b.l - setup.entry! : setup.entry! - b.h;
        if (gap > 0 && gap <= a && k - lastSw >= 6) { sw++; lastSw = k; }
      });
      rally = !filled && dist > 0 && fav / dist >= P.rallyAwayFraction;
      swerves = sw;
    }
    const exempt = biasAligned && lv.touches >= 3;
    const cancel = ((rally ?? false) && !exempt) || (swerves ?? 0) >= P.maxSwerves;
    const checks: Check[] = [
      { rule: 'retest.never-immediate', pass: !tooSoon, detail: retested ? `retest came ${bars} bar(s) after the break (never the very next bar)` : 'no retest yet' },
      { rule: 'retest.needs-space-in-price', pass: excAtr >= P.retestMinExcursionAtr, detail: `price travelled ${excAtr.toFixed(2)} ATR clear of the level (need ${P.retestMinExcursionAtr})` },
      { rule: 'retest.needs-space-in-time', pass: timeOk, detail: `${bars} bars from break to ${retested ? 'retest' : 'now'} (proxy: ${P.retestMinBars}${biasAligned ? ', waived by the aligned bias' : ''})` },
      { rule: 'retest.momentum-into-level', pass: momentum === 'hard' ? true : null, detail: momentum === 'none' ? 'no approach to measure' : `${approachAtr.toFixed(2)} ATR net move into the level over ${win.length} bars: ${momentum}${momentum === 'drift' ? ' (fine in the Asian session, a worry in London/US)' : ''}` },
      { rule: 'retest.rounded-over-sharp', pass: shape === 'sharp' ? null : true, detail: `approach was ${shape}` },
      { rule: 'retest.naked', pass: null, detail: retested ? 'level has been retested since the break (not naked)' : 'level is still naked (no retest since the break)' },
    ];
    if (rally !== null) checks.push({ rule: 'retest.missed-the-level-half-way', pass: !(rally && !exempt), detail: rally ? (exempt ? 'swerved, but excused by three touches plus bias' : 'order was missed and price ran more than half way to target: pull it') : 'not swerved' });
    if (swerves !== null) checks.push({ rule: 'retest.four-swerves', pass: swerves < P.maxSwerves, detail: `${swerves} swerve(s)` });
    return { out: { retest: { retested, barsBreakToRetest: retested ? ri - bi : null, excursionAtr: excAtr, spaceOk, shape, approachAtr, momentum, tooSoon, rallyAway: rally, swerves, cancelOrder: cancel } }, checks };
  },
});
