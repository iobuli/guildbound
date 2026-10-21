import { defineNode } from '../core/node.js';
import { AtrGatePort, BiasPort, ConfluencePort, ContextPort, EntryPort, LevelPort, MarketSidePort, NewsPort, RetestPort, SfpPort, TuesdayPort, OrderCandidate } from '../core/ports.js';
import { rule, src } from '../core/rule.js';
import type { Check } from '../core/types.js';
import type { z } from 'zod';

type Cand = z.infer<typeof OrderCandidate>;

export default defineNode({
  id: 'entry',
  title: 'Entry decision',
  stage: 'E',
  summary: 'Combines the location, retest, confluence, ATR gate, news and bias into a go / no-go for each entry style, with every veto spelled out and the order price.',
  decideAt: 'fill_time',
  step: { n: 9, label: 'Entry: every veto clears, then the order goes in' },
  inputs: { side: MarketSidePort, level: LevelPort, retest: RetestPort, confluence: ConfluencePort, gate: AtrGatePort, news: NewsPort, sfp: SfpPort, bias: BiasPort, tuesday: TuesdayPort, context: ContextPort },
  outputs: { entry: EntryPort },
  rules: [
    rule('entry.limit-at-lxpb', 'S/R entries are limit orders placed at the last high/low before the breakout, in the previous role\'s direction: bid the LHPB after an upside break, offer the LLPB after a downside break.', {
      codable: true, sources: [src('webinar', 'curated/levels.md', '2024-09-24'), src('lesson', 'curated/taking-a-trade-3-touch-level.md', '2024-10-21')],
    }),
    rule('entry.blind-with-three-touches', 'A level with three or more touches is traded blind, with no extra confluence.', {
      codable: true, sources: [src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('entry.vetoes-stack', 'All of these must clear before an S/R entry: the level is on the right side of the market, it is tradeable, it has its confluences, there was space and it is not the immediate retest, it has not been swerved, the market has not already passed its daily ATR in the trade direction, and it is not inside red news.', {
      codable: true, status: 'inferred', sources: [src('lesson', 'curated/taking-a-trade-1-touch-level.md', '2024-10-21'), src('webinar', 'user-pasted notes', '2024-10-03')],
    }),
    rule('entry.prefer-the-level-before-the-trap', 'Between two levels, prefer the one that comes in before the level price is trapped at, and the one closer to the great higher-timeframe level. This also avoids swerves.', {
      status: 'observed', kind: 'preference', sources: [src('journal', 'curated/lxpb-trades-pattern-failure.md', '2025-01-06'), src('journal', 'curated/lxpb-trades-return-to-hammer.md', '2025-01-03')],
    }),
    rule('entry.limit-after-acceptance', 'Newer practice: enter by limit after a close and acceptance beyond the prior level, at the nearest naked level to the SFP low. Do not blindly sell prior highs; wait to see the price action.', {
      status: 'observed', sources: [src('tweet', 'curated tweets', '2026-06-03'), src('tweet', 'curated tweets', '2022-10-21')],
    }),
    rule('entry.offer-the-retest-of-the-spike', 'After a spike, wait for a daily close back inside, offer the retest of the spike high if it stays naked on H1, and bid the spike low for the exit.', {
      status: 'observed', sources: [src('tweet', 'curated tweets', '2026-05-19')],
    }),
    rule('entry.blind-bid-then-add', 'Blind-bid the D1 level, then add on the H1 price action. The first position then has a near-breakeven stop and total risk stays at 1R.', {
      status: 'observed', sources: [src('tweet', 'curated tweets', '2026-05-01')],
    }),
    rule('entry.re-sell-the-same-level', 'Selling a flipped level again on the retest after taking profits there means never being offside during the whole trending move.', {
      kind: 'heuristic', sources: [src('webinar', 'curated/tp-selection.md', '2024-09-24')],
    }),
    rule('entry.entry-types-when-no-lxpb', 'When there is no LXPB the entry can be an H1 pin or a spike candle at the higher-timeframe level.', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-passed-atr.md', '2024-12-16'), src('journal', 'curated/lxpb-trades-return-to-hammer.md', '2024-12-22')],
    }),
    rule('entry.pull-on-drift-of-doom', 'Pull a limit when price stair-steps down to it ("drift of doom"), even when the first trouble area is not too close.', {
      status: 'observed', sources: [src('journal', 'curated/lxpb-trades-engulfing.md', '2026-02-26'), src('journal', 'curated/lxpb-trades-return-to-hammer.md', '2025-05-02')],
    }),
    rule('entry.counter-bias-warning', 'Trading against a fresh higher-timeframe bias is allowed only with good levels and a quick exit.', {
      codable: true, sources: [src('webinar', 'curated/trade-management.md', '2024-10-18')],
    }),
  ],
  evaluate: (inp, v, setup) => {
    const dir = setup.dir;
    const common = (c: Cand) => {
      if (inp.gate.blocks[dir]) c.vetoes.push('market has already passed the daily ATR in this direction');
      if (inp.news.noNewFills) c.vetoes.push('inside the red-news window: no new fills');
      if (inp.bias.htf !== 'neutral' && inp.bias.htf !== dir) c.notes.push(`against the ${inp.bias.htf} higher-timeframe bias: needs good levels and a quick exit`);
      if (inp.context.conservativeTargets) c.notes.push('late-Friday, no-bias context: keep targets conservative');
      if (inp.context.flattenBy !== 'none') c.notes.push(`instrument is flattened by ${inp.context.flattenBy}`);
      c.go = c.vetoes.length === 0;
      return c;
    };
    // S/R flip
    const sr: Cand = { style: 'sr-flip', dir, price: setup.entry ?? inp.level.lxpbPrice, go: false, vetoes: [], notes: [] };
    if (!inp.side.valid) sr.vetoes.push(inp.side.detail);
    if (!inp.level.found) sr.vetoes.push('no flipped level found for this price and direction');
    else {
      if (!inp.level.tradable) sr.vetoes.push(inp.level.touches < 1 ? 'no respected touch in the level\'s previous role' : 'price is chopping through the level');
      if (!inp.confluence.meets) sr.vetoes.push(`only ${inp.confluence.score}/3 confluences`);
      if (inp.retest.tooSoon) sr.vetoes.push('this is the immediate retest, which he never takes');
      else if (!inp.retest.spaceOk) sr.vetoes.push('not enough space between the break and the retest');
      if (inp.retest.cancelOrder) sr.vetoes.push('level has been swerved: pull the order');
      if (inp.confluence.blind) sr.notes.push('three or more touches: can be traded blind');
    }
    common(sr);
    // SFP
    const s = inp.sfp.h1.find((x) => x.dir === dir && x.valid) ?? inp.sfp.h1.find((x) => x.dir === dir);
    const sf: Cand = { style: 'sfp', dir, price: setup.entry ?? s?.closePrice ?? null, go: false, vetoes: [], notes: [] };
    if (!s) sf.vetoes.push('no recent H1 sweep-and-close for this direction');
    else {
      if (!s.deep) sf.vetoes.push('the swept swing is not deep enough');
      if (s.tooSmall) sf.vetoes.push('the sweep candle is too small to mean anything');
      if (s.visitNo > 3) sf.vetoes.push(`this is visit ${s.visitNo} to the area: the 4th is a no`);
      if (s.invalidated) sf.vetoes.push('a candle has closed beyond the SFP candle');
      if (s.ageBars > 48) sf.vetoes.push('the swing is more than about 2 days old');
      if (s.needsDivergence) sf.notes.push('3rd visit: needs a MACD divergence (not checked here)');
      if (s.entryMode === 'wait-half-to-stop') sf.notes.push('wide bar closing near its extreme: do not enter at the close; wait for price to go about half way to the ATR stop, then place a stop order at the original SFP close');
    }
    common(sf);
    // Tuesday
    const tu: Cand = { style: 'tuesday-rto', dir, price: inp.tuesday.entry, go: false, vetoes: [], notes: [] };
    if (!inp.tuesday.valid) tu.vetoes.push(...(inp.tuesday.reasons.length ? inp.tuesday.reasons : ['setup not present']));
    else if (inp.tuesday.dir !== dir) tu.vetoes.push(`the Tuesday setup points ${inp.tuesday.dir}, not ${dir}`);
    common(tu);
    const candidates = [sr, sf, tu];
    const primary = candidates.find((c) => c.style === setup.style) ?? null;
    const checks: Check[] = [];
    if (primary) {
      checks.push({ rule: 'entry.vetoes-stack', pass: primary.go, detail: primary.go ? 'every veto cleared' : primary.vetoes.join('; ') });
      if (primary.notes.some((n) => n.startsWith('against'))) checks.push({ rule: 'entry.counter-bias-warning', pass: null, detail: 'trade is against the higher-timeframe bias' });
    }
    void v;
    return { out: { entry: { candidates, primary } }, checks };
  },
});
