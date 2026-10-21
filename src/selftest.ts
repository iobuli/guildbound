import { readFileSync, readdirSync } from 'node:fs';
import { z } from 'zod';
import { graph, nodes } from './kev.js';
import { Graph } from './core/graph.js';
import { defineNode } from './core/node.js';
import { H, D, MONDAY, makeFeed, viewAt, SOL, lastClosed } from './core/series.js';
import { SetupSchema, type Candle, type View, type Setup } from './core/types.js';
import { wilderAtr } from './core/ta.js';
import type { SfpSignalT } from './core/ports.js';
import atrN from './nodes/atr.js';
import swingDepth from './nodes/swing-depth.js';
import sfpN from './nodes/sfp.js';
import biasN from './nodes/daily-bias.js';
import gateN from './nodes/atr-gate.js';
import levelN from './nodes/level.js';
import retestN from './nodes/retest.js';
import confN from './nodes/confluence.js';
import newsN from './nodes/news.js';
import tueN from './nodes/tuesday-rto.js';
import stopN from './nodes/stop.js';
import targetN from './nodes/target.js';
import mgmtN from './nodes/trade-management.js';
import sideN from './nodes/market-side.js';
import scanN from './nodes/level-scan.js';
import { runBacktest, settle, settleFull } from './backtest/engine.js';
import { scaleForPeriod } from './core/scale.js';
import { P } from './core/params.js';
import { parseMt4 } from './data/mt4.js';
import { aggregate } from './core/series.js';
import { aggregateSession, buildSession, defaultSpec, londonOffsetH, nyOffsetH } from './core/session.js';
import { listMarkets, openMarket } from './core/market-io.js';
import { loadFile } from './data/load.js';
import { bucket } from './data/profile.js';
import { applyPreset, withPreset } from './core/presets.js';
import { analyzeLevel } from './core/levelmath.js';
import { ltfIndex, settleOrder } from './backtest/settle.js';
import { interpret } from './interpret.js';
import { fate, mergeLevels, type LevelId } from './study/atlas.js';
import { writeFileSync, existsSync, rmSync } from 'node:fs';

let fails = 0, passes = 0;
const ok = (name: string, cond: boolean, detail = '') => { cond ? passes++ : fails++; console.log(`${cond ? 'PASS' : 'FAIL'}  ${name}${detail ? '  ' + detail : ''}`); };

// ---------- helpers ----------
const setup = (o: Partial<Setup> = {}): Setup => SetupSchema.parse({ dir: 'long', ...o });
const bar = (t: number, o: number, h: number, l: number, c: number): Candle => ({ t, o, h, l, c, v: 1 });
const view = (o: Partial<View> = {}): View => ({ period: H, session: buildSession(defaultSpec('crypto')), asOf: 0, phase: 'prior_bars', instrument: SOL, h1: [], d1: [], w1: [], now: null, calendar: [], ...o });
/** piecewise-linear H1 path with a small fixed range, so ATR is about 2 * wiggle */
function path(pts: [number, number][], wiggle = 0.3, t0 = Date.UTC(2026, 0, 5)): Candle[] {
  const out: Candle[] = [];
  for (let k = 0; k < pts.length - 1; k++) {
    const [i0, p0] = pts[k]!, [i1, p1] = pts[k + 1]!;
    for (let i = i0; i < i1; i++) {
      const o = p0 + ((p1 - p0) * (i - i0)) / (i1 - i0), c = p0 + ((p1 - p0) * (i + 1 - i0)) / (i1 - i0);
      out.push(bar(t0 + i * H, o, Math.max(o, c) + wiggle, Math.min(o, c) - wiggle, c));
    }
  }
  return out;
}
const sig = (o: Partial<SfpSignalT>): SfpSignalT => ({ tf: 'd1', dir: 'short', swingPrice: 0, swingT: 0, sweepT: 0, sweepIdx: 0, sweepExtreme: 0, closePrice: 0, closeLoc: 0.5, tooSmall: false, strongAgainst: false, deep: true, visitNo: 2, needsDivergence: false, ageBars: 0, invalidated: false, stopAtr: 0, stopAdvanced: 0, entryMode: 'immediate', valid: true, ...o });
const NO_SFP = { h1: [] as SfpSignalT[], d1: [] as SfpSignalT[] };
const ATR = { d1: 8, h1: 1, h1_24: 1 };

// ---------- 1. structure ----------
const errs = graph.validate();
ok('graph validates (wiring, contracts, rule provenance, no cycles, causality)', errs.length === 0, errs.join(' | '));

const src = (f: string) => readFileSync(new URL(`./nodes/${f}`, import.meta.url), 'utf8');
const bad: string[] = [];
for (const f of readdirSync(new URL('./nodes/', import.meta.url)).filter((x) => x.endsWith('.ts')))
  for (const m of src(f).matchAll(/from\s+'([^']+)'/g)) if (!(m[1]!.startsWith('../core/') || m[1] === 'zod')) bad.push(`${f} imports ${m[1]}`);
ok('node isolation: node files import only core/ and zod', bad.length === 0, bad.join(' | '));

// a lookahead node is rejected by construction
const peek = defineNode({ id: 'level-held', title: 'peek', stage: 'L', summary: '', decideAt: 'bar_close', inputs: {}, outputs: { held: z.boolean() }, rules: [] });
const user = defineNode({ id: 'consumer', title: 'consumer', stage: 'E', summary: '', decideAt: 'fill_time', inputs: { held: z.boolean() }, outputs: {}, rules: [] });
const bogus = new Graph([peek, user], [{ from: 'level-held.held', to: 'consumer.held' }]).validate();
ok('a filter that reads the bar close is rejected for an order that fills mid-bar', bogus.some((e) => e.startsWith('CAUSALITY')), bogus.join(' | '));

// ---------- 2. views cannot see the forming bar ----------
const sol: Candle[] = JSON.parse(readFileSync(new URL('../data/SOLUSDT_1h.json', import.meta.url), 'utf8'));
const feed = makeFeed(sol);
const Tmid = sol[40000]!.t + 37 * 60000;
const vFill = viewAt(feed, Tmid, 'fill_time', SOL, { price: sol[40000]!.o }), vClose = viewAt(feed, Tmid, 'bar_close', SOL);
ok('fill_time view ends at the last CLOSED bar', vFill.h1.at(-1)!.t + H <= Tmid && vFill.h1.at(-1)!.t === sol[39999]!.t);
ok('bar_close view includes the bar that closes the fill hour', vClose.h1.at(-1)!.t === sol[40000]!.t);
ok('D1 and W1 views contain only closed bars', vFill.d1.at(-1)!.t + D <= Tmid && vFill.w1.every((b) => b.t + 7 * D <= Tmid));

// ---------- 3. each node alone, on hand-built bars ----------
{ // atr
  const c = path([[0, 100], [200, 100]], 0.5);
  const r = atrN.evaluate!({}, view({ h1: c, d1: c }), setup()).out.atr;
  ok('atr: flat bars with 1.0 range give ATR 1.0', Math.abs(r.h1 - 1) < 1e-6 && Math.abs(r.h1_24 - 1) < 1e-6, JSON.stringify(r));
}
{ // swing-depth -> sfp
  const mk = (extraReturns: number) => {
    const pts: [number, number][] = [[0, 100], [100, 100], [120, 110], [135, 100], [150, 100]];
    let c = path(pts, 0.3);
    // returns toward 110 without sweeping it
    let idx = 150;
    for (let r = 0; r < extraReturns; r++) { c = c.concat(path([[idx, 100], [idx + 6, 109.9], [idx + 12, 100]], 0.3).map((b, k) => ({ ...b, t: c[0]!.t + (idx + k) * H }))); idx += 12; }
    const n = c.length;
    const t0 = c[0]!.t;
    for (let k = 0; k < 4; k++) c.push(bar(t0 + (n + k) * H, 100, 100.6, 99.4, 100));
    c.push(bar(t0 + (n + 4) * H, 109, 111, 108.9, 109)); // sweeps 110 and closes back under it
    c.push(bar(t0 + (n + 5) * H, 109, 109.3, 108.4, 108.8));
    for (let k = 6; k < 9; k++) c.push(bar(t0 + (n + k) * H, 108.8, 109.1, 108.5, 108.8));
    return c;
  };
  for (const [ret, label, expectValid] of [[0, 'clean first sweep', true], [2, 'sweep on the 4th visit', false]] as const) {
    const c = mk(ret), v = view({ h1: c, d1: [] });
    const sw = swingDepth.evaluate!({ atr: ATR }, v, setup()).out.swings;
    const s = sfpN.evaluate!({ atr: ATR, swings: sw }, v, setup({ dir: 'short', style: 'sfp' })).out.sfp.h1.find((x) => Math.abs(x.swingPrice - 110.3) < 0.5);
    ok(`sfp: ${label} -> short SFP, valid=${expectValid}`, !!s && s.dir === 'short' && s.valid === expectValid && s.visitNo === (ret === 0 ? 2 : 4), s ? `visit ${s.visitNo}, deep ${s.deep}, valid ${s.valid}` : 'no signal');
  }
}
{ // daily-bias
  const d = (o: number, h: number, l: number, c: number, k: number) => bar(k * D, o, h, l, c);
  const base = [d(100, 101, 99, 100, 0), d(100, 101, 97, 98, 1), d(97.5, 101.5, 97.2, 101, 2)]; // bearish then bullish engulfing
  const r = biasN.evaluate!({ sfp: NO_SFP }, view({ d1: base }), setup()).out.bias;
  ok('daily-bias: bullish engulfing -> long, fresh', r.d1.dir === 'long' && r.d1.fresh && r.d1.signals.some((s) => s.kind === 'engulfing'), JSON.stringify(r.d1));
  const cross = biasN.evaluate!({ sfp: { ...NO_SFP, d1: [sig({ dir: 'short', sweepIdx: 2, valid: true })] } }, view({ d1: base }), setup()).out.bias;
  ok('daily-bias: opposing SFP on the same bar cancels (crossbar) -> neutral', cross.d1.dir === 'neutral' && cross.d1.cancelled, JSON.stringify(cross.d1));
  const old = biasN.evaluate!({ sfp: NO_SFP }, view({ d1: [...base, d(101.5, 102.2, 101.0, 101.9, 3), d(101.9, 102.8, 101.7, 102.5, 4), d(102.5, 103.4, 102.3, 103.1, 5)] }), setup()).out.bias;
  ok('daily-bias: an engulfing 3 bars ago has expired', old.d1.dir === 'neutral', JSON.stringify(old.d1.dir));
}
{ // atr-gate: measured from the HIGH / LOW OF THE DAY, not the open
  const day = Date.UTC(2026, 5, 3);
  const today = [bar(day, 99, 100, 98.5, 99.5), bar(day + H, 99.5, 99.8, 96, 96.5), bar(day + 2 * H, 96.5, 97, 93, 93.5)];
  const v = view({ phase: 'fill_time', h1: today, now: { t: day + 3 * H + 20 * 60000, price: 90.4, dayOpen: 99, weekOpen: 99 } });
  const g = gateN.evaluate!({ atr: { ...ATR, d1: 10 } }, v, setup({ dir: 'short' }));
  ok('atr-gate: 9.6 below the day\'s high with a 10 ATR vetoes a short and supports a long', g.out.gate.blocks.short && g.out.gate.supports.long && g.checks![0]!.pass === false, `from high ${g.out.gate.fromHighAtr.toFixed(2)} ATR`);
  const open = gateN.evaluate!({ atr: { ...ATR, d1: 10 } }, view({ phase: 'fill_time', h1: today, now: { t: day + 3 * H, price: 97.6, dayOpen: 99, weekOpen: 99 } }), setup({ dir: 'short' }));
  ok('atr-gate: the same move measured from the OPEN would be only 0.14 ATR, but from the high it is 0.24 and no veto', !open.out.gate.blocks.short, `from high ${open.out.gate.fromHighAtr.toFixed(2)}`);
}
{ // news
  const now = Date.UTC(2026, 5, 3, 12, 20), v = view({ phase: 'fill_time', instrument: { symbol: 'EURUSD', cls: 'fx-major', pip: 0.0001 }, now: { t: now, price: 1.1010, dayOpen: 1.1, weekOpen: 1.1 }, calendar: [{ t: now + 8 * 60000, impact: 'red', label: 'NFP' }] });
  const r = newsN.evaluate!({ atr: { d1: 0.008, h1: 0.0008, h1_24: 0.0008 } }, v, setup({ entry: 1.1000 })).out.news;
  ok('news: red news in 8 min with price 10 pips from the limit -> no fills, pull the limit', r.noNewFills && r.action === 'pull-limit' && r.holdIfInTrade, JSON.stringify(r));
  const far = newsN.evaluate!({ atr: { d1: 0.008, h1: 0.0008, h1_24: 0.0008 } }, { ...v, now: { ...v.now!, price: 1.1080 } }, setup({ entry: 1.1000 })).out.news;
  ok('news: price 80 pips away -> leave the limit', far.action === 'keep-limit', far.action);
}
{ // tuesday-rto: Monday 00:00 open 100, price stays above, Tuesday bearish pin, target = weekly open
  const mon = Date.UTC(2026, 9, 19); // Monday
  const c: Candle[] = [];
  for (let k = 0; k < 34; k++) c.push(bar(mon + k * H, 110, 110.6, 109.4, 110));
  c[0] = bar(mon, 100, 110.6, 99.5, 110); // the weekly open candle itself touches 100 (Monday 00:00)
  for (let k = 1; k < 34; k++) c[k] = bar(mon + k * H, 110, 110.6, 109.4, 110);
  c[33] = bar(mon + 33 * H, 109.6, 110.4, 109.4, 109.7); // Tuesday 09:00 bar: tiny body? make it a bearish pin
  c[33] = bar(mon + 33 * H, 109.7, 112, 109.5, 109.6);
  const r = tueN.evaluate!({}, view({ asOf: mon + 34 * H, h1: c }), setup({ dir: 'short', style: 'tuesday-rto' })).out.tuesday;
  ok('tuesday-rto: untouched weekly open + bearish pin -> short toward the weekly open', r.valid && r.dir === 'short' && r.target === 100 && (r.rr ?? 0) >= 1, JSON.stringify(r));
  const wed = tueN.evaluate!({}, view({ asOf: mon + 58 * H, h1: c }), setup({ dir: 'short', style: 'tuesday-rto' })).out.tuesday;
  ok('tuesday-rto: not valid on a Wednesday', !wed.valid && wed.reasons[0]!.includes('Tuesday'));
}
{ // level -> retest on a hand-built flipped resistance (3 rejections, break, run away, pullback)
  const c = path([[0, 95], [45, 95], [50, 100], [55, 96], [85, 96], [90, 100], [95, 96], [125, 96], [130, 100], [135, 96], [165, 96], [170, 101], [176, 105], [195, 106], [210, 102]], 0.3);
  const v = view({ h1: c });
  const lv = levelN.evaluate!({ atr: { d1: 8, h1: wilderAtr(c), h1_24: wilderAtr(c) } }, v, setup({ levelPrice: 100.1 }));
  ok('level: 3 respected rejections before the break are counted ("opposite last")', lv.out.level.found && lv.out.level.touches === 3 && lv.out.level.role === 'support', `touches ${lv.out.level.touches}, break @${lv.out.level.breakIdx}`);
  ok('level: LXPB is the high of the last pre-break touch', lv.out.level.lxpbPrice !== null && Math.abs(lv.out.level.lxpbPrice - 100.3) < 0.6, `lxpb ${lv.out.level.lxpbPrice}`);
  const biasNone = { d1: { dir: 'neutral' as const, signals: [], cancelled: false, ageBars: null, fresh: false }, w1: { dir: 'neutral' as const, signals: [], cancelled: false, ageBars: null, fresh: false }, htf: 'neutral' as const };
  const rt = retestN.evaluate!({ level: lv.out.level, atr: { d1: 8, h1: wilderAtr(c), h1_24: wilderAtr(c) }, bias: biasNone }, v, setup({ levelPrice: 100.1 }));
  ok('retest: ran far from the level for long enough and is still naked', rt.out.retest.spaceOk && !rt.out.retest.retested, JSON.stringify(rt.out.retest));
  const quick = path([[0, 95], [45, 95], [50, 100], [55, 96], [85, 96], [90, 100], [95, 96], [125, 96], [130, 100], [135, 96], [165, 96], [170, 101], [173, 102], [176, 100.2]], 0.3);
  const lq = levelN.evaluate!({ atr: { d1: 8, h1: wilderAtr(quick), h1_24: 1 } }, view({ h1: quick }), setup({ levelPrice: 100.1 }));
  const rq = retestN.evaluate!({ level: lq.out.level, atr: { d1: 8, h1: wilderAtr(quick), h1_24: 1 }, bias: biasNone }, view({ h1: quick }), setup({ levelPrice: 100.1 }));
  ok('retest: a retest only 6 bars after the break fails the space rule', !rq.out.retest.spaceOk && rq.out.retest.retested, JSON.stringify(rq.out.retest));
  // stop: thrust candle low, moved past a nearby swing
  const thrust = c[lv.out.level.breakIdx!]!;
  const near = { tf: 'h1' as const, i: 1, t: 0, side: 'low' as const, price: thrust.l - 0.15, swingIn: 1, swingOut: 1, depth: 1, depthAtr: 0.1, deep: false, sweptAt: null };
  const st = stopN.evaluate!({ level: lv.out.level, atr: { d1: 8, h1: 1, h1_24: 1 }, swings: { h1: [near], d1: [] }, sfp: NO_SFP, tuesday: { valid: false, dir: null, entry: null, stop: null, target: null, rr: null, reasons: [] } }, v, setup({ levelPrice: 100.1, entry: 100.3 })).out.stop;
  ok('stop: placed beyond the thrust candle and moved past a level sitting just beyond it', st.shifted && st.suggested !== null && st.suggested < thrust.l - 0.15, `thrust low ${thrust.l.toFixed(2)}, suggested ${st.suggested?.toFixed(2)}`);
  ok('stop: audit flags a proposed stop that is inside the suggested one', stopN.evaluate!({ level: lv.out.level, atr: { d1: 8, h1: 1, h1_24: 1 }, swings: { h1: [], d1: [] }, sfp: NO_SFP, tuesday: { valid: false, dir: null, entry: null, stop: null, target: null, rr: null, reasons: [] } }, v, setup({ levelPrice: 100.1, entry: 100.3, stop: thrust.l + 0.5 })).checks!.some((x) => x.rule === 'stop.beyond-thrust' && x.pass === false));
}
{ // confluence tiers
  const lvl = (touches: number) => ({ found: true, role: 'support' as const, center: 100, zoneLo: 99.7, zoneHi: 100.3, breakIdx: 5, breakT: 0, barsSinceBreak: 20, touches, lastTouchT: 0, lxpbPrice: 100.2, spanBars: 40, chopCrossings: 0, disrespected: false, tradable: true });
  const bias = (dir: 'long' | 'neutral') => ({ d1: { dir, signals: [], cancelled: false, ageBars: dir === 'long' ? 0 : null, fresh: dir === 'long' }, w1: { dir: 'neutral' as const, signals: [], cancelled: false, ageBars: null, fresh: false }, htf: dir });
  const gate = (down: boolean) => ({ movedAtr: down ? -1 : 0, fromHighAtr: down ? 1 : 0, fromLowAtr: 0, passedUp: false, passedDown: down, blocks: { long: false, short: down }, supports: { long: down, short: false } });
  const v = view({ phase: 'fill_time', d1: [bar(0, 100, 101, 99, 100)], now: { t: 0, price: 100, dayOpen: 100, weekOpen: 100 } });
  const run = (t: number, b: 'long' | 'neutral', g: boolean) => confN.evaluate!({ level: lvl(t), bias: bias(b), gate: gate(g), atr: ATR, swings: { h1: [], d1: [] } }, v, setup({ levelPrice: 100 })).out.confluence;
  ok('confluence: 3 touches is enough on its own (blind)', run(3, 'neutral', false).meets && run(3, 'neutral', false).blind);
  ok('confluence: 2 touches need one more', !run(2, 'neutral', false).meets && run(2, 'neutral', false).needed === 1 && run(2, 'long', false).meets);
  ok('confluence: 1 touch + daily bias + passed ATR = 3', run(1, 'long', true).meets && !run(1, 'long', false).meets);
}
{ // target
  const sw = (i: number, p: number) => ({ tf: 'h1' as const, i, t: i, side: 'high' as const, price: p, swingIn: 2, swingOut: 2, depth: 2, depthAtr: 0.25, deep: false, sweptAt: null });
  const lv = { found: true, role: 'support' as const, center: 100, zoneLo: 99.7, zoneHi: 100.3, breakIdx: 5, breakT: 0, barsSinceBreak: 20, touches: 3, lastTouchT: 0, lxpbPrice: 100, spanBars: 40, chopCrossings: 0, disrespected: false, tradable: true };
  const tp = targetN.evaluate!({ stop: { stop: 99, suggested: 99, method: '', shifted: false, riskDist: 1, ok: true, squeezeLevel: false }, level: lv, swings: { h1: [sw(10, 102), sw(30, 106)], d1: [] }, bias: { d1: { dir: 'neutral', signals: [], cancelled: false, ageBars: null, fresh: false }, w1: { dir: 'neutral', signals: [], cancelled: false, ageBars: null, fresh: false }, htf: 'neutral' }, atr: ATR, tuesday: { valid: false, dir: null, entry: null, stop: null, target: null, rr: null, reasons: [] } }, view({ h1: path([[0, 100], [120, 100]]) }), setup({ entry: 100 })).out.target;
  ok('target: no bias -> FTA is the nearest meaningful swing, most-recent offered as alternative', tp.chosen?.kind === 'fta' && tp.chosen.price === 102 && tp.candidates.some((c) => c.kind === 'recent' && c.price === 106) && tp.chosen.rr === 2, JSON.stringify(tp.chosen));
}
{ // management: closes under entry, retests it, then closes under the initial low -> exit
  const t0 = Date.UTC(2026, 5, 1, 10);
  const c = [bar(t0 - H, 100, 100.4, 99.6, 100.2), bar(t0, 100.1, 100.3, 99.8, 100.1), bar(t0 + H, 100, 100, 99.3, 99.5), bar(t0 + 2 * H, 99.5, 100.1, 99.4, 99.8), bar(t0 + 3 * H, 99.7, 99.8, 98.9, 99.0)];
  const r = mgmtN.evaluate!({ stop: { stop: 98.5, suggested: 98.5, method: '', shifted: false, riskDist: 1.5, ok: true, squeezeLevel: false }, target: { candidates: [], chosen: null, minRr: 0.8 }, context: { weekday: 3, hourUtc: 14, session: 'ny', fridayLate: false, flattenBy: 'none', mode: 'no-bias', conservativeTargets: false } }, view({ phase: 'post_fill', asOf: t0 + 4 * H, h1: c }), setup({ entry: 100, target: 103, fillAt: t0 + 600000 })).out.mgmt;
  ok('management: broken-entry retest then close below the initial low -> close', r.actions.some((a) => a.action === 'close' && a.rule === 'management.broken-entry-retest-exit'), JSON.stringify(r.actions.map((a) => a.rule)));
}

{ // market-side: buy below, sell above
  const c = path([[0, 100], [100, 100]], 0.3), v = view({ h1: c });
  const below = sideN.evaluate!({}, v, setup({ dir: 'long', levelPrice: 95 }));
  const wrong = sideN.evaluate!({}, v, setup({ dir: 'short', levelPrice: 95 }));
  ok('market-side: a level below price is a buy, and selling it is rejected', below.out.side.valid && below.out.side.requiredDir === 'long' && !wrong.out.side.valid && wrong.checks![0]!.pass === false);
  ok('market-side: not enforced for market-order SFP entries', sideN.evaluate!({}, v, setup({ dir: 'short', levelPrice: 95, style: 'sfp' })).out.side.valid);
}
{ // level-scan finds the flipped level and its touches without being told where it is
  const c = path([[0, 95], [45, 95], [50, 100], [55, 96], [85, 96], [90, 100], [95, 96], [125, 96], [130, 100], [135, 96], [165, 96], [170, 101], [176, 105], [195, 106], [210, 102]], 0.3);
  const a = { d1: 8, h1: wilderAtr(c), h1_24: 1 };
  const sw = swingDepth.evaluate!({ atr: a }, view({ h1: c }), setup()).out.swings;
  const rows = scanN.evaluate!({ atr: a, swings: sw }, view({ h1: c }), setup()).out.levels.rows;
  const hit = rows.find((r) => Math.abs(r.price - 100.3) < 0.8);
  ok('level-scan: finds the flipped resistance near 100 with 3 touches, a buy, tier "blind"', !!hit && hit.touches === 3 && hit.dir === 'long' && hit.tier === 'blind', JSON.stringify(rows.map((r) => [+r.price.toFixed(1), r.touches, r.tier])));
}
{ // the immediate retest is never taken, even with an aligned bias
  const quick = path([[0, 95], [45, 95], [50, 100], [55, 96], [85, 96], [90, 100], [95, 96], [125, 96], [130, 100], [135, 96], [165, 96], [166, 96], [167, 101.5], [168, 100.2]], 0.3);
  const a = { d1: 8, h1: wilderAtr(quick), h1_24: 1 }, v = view({ h1: quick });
  const lv = levelN.evaluate!({ atr: a }, v, setup({ levelPrice: 100.1 })).out.level;
  const longBias = { d1: { dir: 'long' as const, signals: [], cancelled: false, ageBars: 0, fresh: true }, w1: { dir: 'neutral' as const, signals: [], cancelled: false, ageBars: null, fresh: false }, htf: 'long' as const };
  const r = retestN.evaluate!({ level: lv, atr: a, bias: longBias }, v, setup({ levelPrice: 100.1 })).out.retest;
  ok('retest: a retest on the very next bars is refused even with an aligned daily bias', r.tooSoon && !r.spaceOk, JSON.stringify({ bars: r.barsBreakToRetest, tooSoon: r.tooSoon }));
}
{ // the graph evaluates in the trader's order, and flow() reports it in that order
  const ord = graph.order(), at = (id: string) => ord.indexOf(id);
  ok('order of operations: side < level < confluence < retest < news < stop < target < entry < management', ['market-side', 'level', 'confluence', 'retest', 'news', 'stop', 'target', 'entry', 'management'].every((id, k, a) => k === 0 || at(a[k - 1]!) < at(id)), ord.filter((id) => graph.byId.get(id)!.step).join(' > '));
  const T = sol[41000]!.t + 37 * 60000, price = sol[41000]!.o;
  const r = graph.run(feed, T, setup({ dir: 'long', levelPrice: Math.round(price) }), { instrument: SOL, price });
  const f = graph.flow(r);
  ok('flow(): steps come out sorted by the method\'s step number and name the first blocker', f.steps.every((s, k) => k === 0 || f.steps[k - 1]!.step <= s.step) && (f.firstBlocker === null || f.steps.indexOf(f.firstBlocker) === f.steps.findIndex((s) => s.status === 'fail')), `${f.steps.length} steps, first blocker: ${f.firstBlocker?.label ?? 'none'}`);
}

// ---------- 4. the graph on real data, and the lookahead audit ----------
const entryT = (k: number) => sol[k]!.t + 37 * 60000;
const roundLevel = (p: number) => Math.round(p / 2) * 2;
const sample = [20000, 26000, 30500, 34000, 38000, 41000, 44000, 47000, 50000, 52000];
let full = 0, errors: string[] = [];
const strip = (r: ReturnType<Graph['run']>, upTo: string[]) => JSON.stringify(Object.fromEntries(upTo.map((id) => [id, r.states[id]])));
const phaseOf = (id: string) => graph.byId.get(id)!.decideAt;
const early = nodes.filter((n) => ['prior_bars', 'bar_open', 'fill_time'].includes(n.decideAt) && n.evaluate).map((n) => n.id);
let truncOk = 0, tamperOk = 0;
for (const k of sample) {
  const T = entryT(k), price = sol[k]!.o * 1.0005;
  const s = setup({ dir: k % 2 ? 'long' : 'short', levelPrice: roundLevel(price) });
  const cal = [{ t: T + 9 * 60000, impact: 'red' as const, label: 'x' }];
  const a = graph.run(feed, T, s, { instrument: SOL, price, calendar: cal });
  for (const id of a.order) { const st = a.states[id]!; if (st.error) errors.push(`${id}: ${st.error}`); if (st.ran) full++; }
  // (1) delete every bar after the fill hour: all nodes must be unchanged
  const cut = makeFeed(sol.filter((b) => b.t < Math.ceil(T / H) * H));
  const b = graph.run(cut, T, s, { instrument: SOL, price, calendar: cal });
  if (strip(a, a.order) === strip(b, a.order)) truncOk++; else console.log('  truncation changed', a.order.filter((id) => JSON.stringify(a.states[id]) !== JSON.stringify(b.states[id])));
  // (2) tamper the forming bar's high/low/close: nodes that decide at or before fill_time must be unchanged
  const forming = sol.findIndex((x) => x.t === Math.floor(T / H) * H);
  const tampered = sol.map((x, i) => (i === forming ? { ...x, h: x.h * 3, l: x.l / 3, c: x.c * 2 } : x));
  const c = graph.run(makeFeed(tampered), T, s, { instrument: SOL, price, calendar: cal });
  if (strip(a, early) === strip(c, early)) tamperOk++; else console.log('  tamper changed', early.filter((id) => JSON.stringify(a.states[id]) !== JSON.stringify(c.states[id])));
}
ok('graph runs end-to-end on SOL with every contract satisfied', errors.length === 0, errors.slice(0, 3).join(' | '));
ok(`lookahead audit 1: deleting all future bars changes nothing (${truncOk}/${sample.length} timestamps)`, truncOk === sample.length);
ok(`lookahead audit 2: tampering with the forming bar's H/L/C changes no node that decides by fill time (${tamperOk}/${sample.length})`, tamperOk === sample.length);

// nodes that decide AFTER the fill are allowed to see the fill bar; show they actually do
{
  const k = 41000, T = entryT(k), price = sol[k]!.o;
  const s = setup({ dir: 'long', levelPrice: roundLevel(price), entry: price, fillAt: T });
  const A = graph.run(feed, T, s, { instrument: SOL, price }), forming = k;
  const B = graph.run(makeFeed(sol.map((x, i) => (i === forming ? { ...x, c: x.c * 0.5, l: x.l * 0.5 } : x))), T, s, { instrument: SOL, price });
  ok('management (post_fill) does react to the fill bar, as it is allowed to', JSON.stringify(A.states.management) !== JSON.stringify(B.states.management));
}

// ---------- 4b. the backtest harness ----------
{
  // settle(): stop wins a same-bar tie; a fill bar that already reaches the stop is a loss; targets pay k R minus costs
  const b = (o: number, h: number, l: number, c: number, i: number) => bar(i * H, o, h, l, c);
  const tie = settle([b(100, 100, 99, 100, 0), b(100, 103, 98.5, 102, 1)], 0, true, 100, 99, null, 5, 0.1);
  ok('settle: stop and 2R target inside the same bar -> stop first', tie['2']! === -1.1 && tie['1']! === -1.1, JSON.stringify(tie));
  const win = settle([b(100, 100.2, 99.5, 100, 0), b(100, 101.5, 99.6, 101.4, 1), b(101.4, 102.2, 101, 102, 2)], 0, true, 100, 99, null, 5, 0.1);
  ok('settle: 1R then 2R targets pay 1R and 2R minus the 0.10R cost', Math.abs(win['1']! - 0.9) < 1e-9 && Math.abs(win['2']! - 1.9) < 1e-9, JSON.stringify(win));
  const gap = settle([b(100, 100.1, 98.9, 99.5, 0), b(99.5, 105, 99, 104, 1)], 0, true, 100, 99, null, 5, 0.1);
  ok('settle: a fill bar whose own low reaches the stop is a loss, whatever happens next', gap['4']! === -1.1);
  // no lookahead: the same decisions and outcomes come out whether or not later bars exist
  const a = runBacktest(sol.slice(0, 42400), { start: 41000, end: 1e9 }), z = runBacktest(sol.slice(0, 43800), { start: 41000, end: 1e9 });
  const k = (t: { i: number; dir: string; tier: string; level: number; fill: number; go: boolean; vetoes: string[]; r: object }) => JSON.stringify([t.i, t.dir, t.tier, +t.level.toFixed(6), +t.fill.toFixed(6), t.go, t.vetoes, t.r]);
  const lim = Math.max(...a.trades.map((t) => t.i)), A = a.trades.map(k), B = z.trades.filter((t) => t.i <= lim).map(k);
  ok(`backtest harness has no lookahead: ${A.length} trades identical with and without later bars`, A.length > 20 && A.length === B.length && A.every((x, n) => x === B[n]));
}

// ---------- 4c. gold data, timeframes, week anchor, held-exit ----------
{
  const b = (o: number, h: number, l: number, c: number, i: number) => bar(i * H, o, h, l, c);
  const held = settleFull([b(100, 100.2, 99.5, 99.6, 0), b(99.6, 105, 99.5, 104, 1)], 0, true, 100, 99, null, 5, 0.1, 99.8);
  ok('held-exit: a fill bar that closes on the wrong side of the level flattens at that close for all targets', Math.abs(held.r['4']! - (-0.5)) < 1e-9 && held.x['4'] === 0, JSON.stringify(held.r));
  const kept = settleFull([b(100, 100.2, 99.5, 99.9, 0), b(99.9, 105, 99.8, 104, 1)], 0, true, 100, 99, null, 5, 0.1, 99.8);
  ok('held-exit: a fill bar that closes back above the level stays in the trade', kept.r['4']! > 0);
  const m30 = scaleForPeriod(H / 2), a = P.retestMinBars; m30();
  const h4 = scaleForPeriod(4 * H), c4 = P.retestMinBars, lb4 = P.lookback.h1; h4();
  ok('scaleForPeriod: 11 H1 bars of space = 22 M30 bars = 3 H4 bars, and values are restored', a === 22 && c4 === 3 && lb4 === 200 && P.retestMinBars === 11 && P.lookback.h1 === 800, `${a} ${c4} ${lb4} ${P.retestMinBars}`);
  // ---- sessions: daylight saving, NY-close days, Sunday stubs, causality of merged days ----
  const tz = (zone: string, t: number) => { const p = new Intl.DateTimeFormat('en-US', { timeZone: zone, hourCycle: 'h23', year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric' }).formatToParts(new Date(t)); const g = (k: string) => +p.find((x) => x.type === k)!.value; return Math.round((Date.UTC(g('year'), g('month') - 1, g('day'), g('hour')) - Math.floor(t / H) * H) / H); };
  let dstBad = 0; for (let t = Date.UTC(2005, 0, 1); t < Date.UTC(2027, 0, 1); t += 7 * H + 13 * 60_000) { if (tz('America/New_York', t) !== nyOffsetH(t)) dstBad++; if (tz('Europe/London', t) !== londonOffsetH(t)) dstBad++; }
  ok('session: hand-rolled New York and London daylight saving agree with Intl for every 7h13m step 2005-2026', dstBad === 0, `${dstBad} disagreements`);
  const ny = buildSession(defaultSpec('metal'));
  ok('session: a NY-close day starts 21:00 UTC in July and 22:00 UTC in January, and Sunday 18:00 NY opens Monday', ny.dayStart(Date.UTC(2026, 6, 15, 12)) === Date.UTC(2026, 6, 14, 21) && ny.dayStart(Date.UTC(2026, 0, 15, 12)) === Date.UTC(2026, 0, 14, 22) && ny.weekday(Date.UTC(2026, 6, 26, 22)) === 1 && ny.weekStart(Date.UTC(2026, 6, 28, 9)) === Date.UTC(2026, 6, 26, 21));
  const stubBars = [Date.UTC(2019, 0, 4, 20), Date.UTC(2019, 0, 4, 21), Date.UTC(2019, 0, 6, 22), Date.UTC(2019, 0, 6, 23), Date.UTC(2019, 0, 7, 0), Date.UTC(2019, 0, 7, 1)].map((t, i) => ({ t, o: 10 + i, h: 11 + i, l: 9 + i, c: 10.5 + i, v: 1 }));
  const us = buildSession(defaultSpec('crypto'), stubBars), d1s = aggregateSession(stubBars, us, 'day');
  ok('session: a 2h Sunday reopening after a weekend close is merged into Monday (no stub daily bar), and that day only closes at Monday 24:00', d1s.length === 2 && d1s[1]!.t === Date.UTC(2019, 0, 6) && d1s[1]!.e === Date.UTC(2019, 0, 8) && us.weekday(Date.UTC(2019, 0, 6, 22)) === 1 && lastClosed(d1s, 86_400_000, Date.UTC(2019, 0, 7, 23)) === 0, JSON.stringify(d1s.map((x) => [new Date(x.t).toISOString(), x.e && new Date(x.e).toISOString()])));
  if (listMarkets().includes('XAUUSD')) {
    const gm = openMarket('XAUUSD'), gd = gm.feed().d1, per = new Map<number, number>();
    for (const b of gm.h1) per.set(gm.session.dayStart(b.t), (per.get(gm.session.dayStart(b.t)) ?? 0) + 1);
    const short = [...per.values()].filter((n) => n < 6).length, sundays = gd.filter((b) => gm.session.weekday(b.t + 12 * H) === 0).length;
    ok('session: gold on NY-close days has no Sunday daily bars and under 1% of days shorter than 6 hours (was 16.5% Sunday stubs on UTC days)', sundays === 0 && short / per.size < 0.01, `${per.size} days, ${short} short, ${sundays} Sunday bars`);
  }
  if (listMarkets().includes('ETHUSD')) {
    const em = openMarket('ETHUSD'), lt = em.ltf(), m30 = lt.find((x) => x.tf === 30 * 60_000), m15 = lt.find((x) => x.tf === 15 * 60_000);
    if (m30 && m15) {
      const a = bucket(m30.c, H), b = bucket(m15.c, H); let cmp = 0, bad = 0;
      for (const [k, x] of a) { const y = b.get(k); if (!y || x.n !== 2 || y.n !== 4) continue; cmp++; if (Math.max(Math.abs(x.o - y.o), Math.abs(x.h - y.h), Math.abs(x.l - y.l), Math.abs(x.c - y.c)) > 1e-9) bad++; }
      ok('ingest: ETH H1 built from M30 equals H1 built from M15 wherever both are complete', cmp > 40_000 && bad === 0, `${cmp} hours compared, ${bad} differ`);
    }
    const stubDays = [...em.session.merged].filter((l) => new Date(l).getUTCFullYear() <= 2019).length;
    ok('session: ETH Sunday reopenings in its weekend-closed years (2018-2019) are merged into Monday', stubDays > 80, `${stubDays} merged`);
  }
  // ---- any-CSV sniffing ----
  const tmp = (name: string, body: string) => { const u = new URL(`../out/_sniff_${name}`, import.meta.url); writeFileSync(u, body); const r = loadFile(u.pathname); rmSync(u.pathname, { force: true }); return r; };
  const k1 = tmp('mt4.csv', '2020.01.02,01:00,1.1,1.2,1.0,1.15,10\r\n2020.01.02,02:00,1.15,1.3,1.1,1.2,11\r\n2020.01.02,03:00,1.2,1.25,1.1,1.12,9\r\n2020.01.02,04:00,1.12,1.3,1.1,1.2,9\r\n2020.01.02,05:00,1.2,1.3,1.1,1.25,9\r\n');
  const k2 = tmp('binance.csv', '1577926800000,7200.1,7210,7190,7205,12,1577930399999\n1577930400000,7205,7215,7200,7210,10,1577933999999\n1577934000000,7210,7212,7201,7202,9,1577937599999\n');
  const k3 = tmp('hdr.csv', 'Date;Open;High;Low;Close;Volume\n31.12.2024 23:59:59;1,5;1,7;1,4;1,6;100\n30.12.2024 23:59:59;1,4;1,6;1,3;1,5;90\n29.12.2024 23:59:59;1,3;1,5;1,2;1,4;80\n');
  ok('sniff: MT4 date+time CSV, Binance kline CSV (epoch ms), and a day-first, decimal-comma, newest-first, end-stamped file all load', k1.candles.length === 5 && k1.tf === H && k1.candles[0]!.t === Date.UTC(2020, 0, 2, 1) && k2.candles.length === 3 && k2.tf === H && k2.candles[0]!.c === 7205 && k3.candles.length === 3 && k3.problems.descending && k3.problems.endStamped === 3 && k3.candles[0]!.t === Date.UTC(2024, 11, 29) && k3.candles[2]!.c === 1.6, JSON.stringify([k1.layout.name, k2.layout.name, k3.layout.name, k3.candles[0]]));
  const f = new URL('../out/_mt4test.csv', import.meta.url); /* removed after the test */
  writeFileSync(f, '2020-01-02 01:00\t1500.0\t1501.0\t1499.0\t1500.5\t10\n2020-01-02 02:00\t1500.5\t1500.0\t1500.9\t1500.2\t11\n2020-01-02 02:00\t1\t2\t3\t4\t5\nbad row\n');
  const pm = parseMt4(f.pathname);
  rmSync(f.pathname, { force: true });
  ok('mt4 parser: reads tab-separated bars, repairs an impossible high/low, drops duplicates and junk', pm.candles.length === 2 && pm.problems.repairedOhlc === 1 && pm.problems.duplicates === 1 && pm.problems.badRows === 1 && pm.candles[1]!.h >= pm.candles[1]!.l, JSON.stringify(pm.problems));
  const g = new URL('../data/XAUUSD_h1.json', import.meta.url), gd = new URL('../data/XAUUSD_d1.json', import.meta.url);
  if (existsSync(g) && existsSync(gd)) {
    const h1g: Candle[] = JSON.parse(readFileSync(g, 'utf8')), d1g: Candle[] = JSON.parse(readFileSync(gd, 'utf8'));
    const agg = new Map(aggregate(h1g, D).map((x) => [x.t, x]));
    let bad = 0; for (const x of d1g) { const y = agg.get(x.t); if (!y || Math.abs(y.h - x.h) > 0.5 || Math.abs(y.l - x.l) > 0.5 || Math.abs(y.c - x.c) > 0.5) bad++; }
    ok('gold: daily candles built from the H1 file match the separate D1 file on every day', bad === 0 && d1g.length > 5000, `${d1g.length} days, ${bad} mismatches`);
  }
}

// ---------- 5. knowledge base hygiene ----------
const rules = nodes.flatMap((n) => n.rules);
const by = (s: string) => rules.filter((r) => r.status === s).length;
ok(`knowledge base: ${rules.length} rules across ${nodes.length} nodes, all with provenance`, rules.every((r) => r.sources.length > 0) && rules.length > 100,
  `${by('stated')} stated, ${by('observed')} observed, ${by('inferred')} inferred, ${by('backtest-reported')} backtest-reported, ${by('backtest-rejected')} backtest-rejected; ${rules.filter((r) => r.codable).length} coded`);
// ---- presets, the fractal touch rule, finer-bar settlement ----
{
  const before = JSON.stringify(P);
  const restore = applyPreset('one-hour-spec');
  const during = { rule: P.touchRule, scale: P.levelScaleD1, gap: P.touchClusterBars };
  restore();
  ok('presets: one-hour-spec switches the parser to the fractal rule on the daily-ATR scale, and restore puts every value back', during.rule === 'fractal' && during.scale === 1 && during.gap === 12 && JSON.stringify(P) === before);
  const srcFiles = (dir: string): string[] => readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? srcFiles(`${dir}/${e.name}`) : e.name.endsWith('.ts') ? [`${dir}/${e.name}`] : []));
  const root = new URL('.', import.meta.url).pathname.replace(/\/$/, '');
  const writers = srcFiles(root).filter((f) => !/core\/(presets|scale)\.ts$|selftest\.ts$/.test(f) && /Object\.assign\(P\b|\bP\.[A-Za-z0-9_]+(\.[a-z0-9]+)?\s*=[^=]|\bP\[[^\]]+\]\s*=[^=]/.test(readFileSync(f, 'utf8')));
  ok('presets: nothing outside presets.ts and scale.ts writes the shared parameters', writers.length === 0, writers.join(', '));
  // fractal rule: four 3-bar swing highs at 100 (bars 70, 76, 95, 120), then a break above. 76 is only 6 bars after 70 so it is not a new touch.
  const fc: Candle[] = Array.from({ length: 160 }, (_, i) => ({ t: i * H, o: 95, h: 96, l: 94, c: 95, v: 1 }));
  for (const k of [70, 76, 95, 120]) fc[k] = { ...fc[k]!, h: 100.02, c: 96 };
  for (let k = 140; k < 160; k++) fc[k] = { ...fc[k]!, o: 101, h: 102, l: 100.6, c: 101.5 };
  const fr = withPreset('one-hour-spec', () => analyzeLevel(fc, 100, 'long', 2));
  ok('fractal rule: swing points at the level count once per 12 bars (70, 95, 120 -> 3; 76 is too close to 70)', fr.found && fr.touches === 3 && fr.breakIdx === 140, JSON.stringify({ found: fr.found, touches: fr.touches, br: fr.breakIdx }));
  // finer-bar settlement: an H1 bar holds both the stop and the target; the minutes show the target first
  const hb = (o: number, h: number, l: number, c: number, i: number) => ({ t: i * H, o, h, l, c, v: 1 });
  const hc = [hb(100, 100.5, 99.8, 100.2, 0), hb(100.2, 102.5, 98.5, 99, 1)];
  const sub60 = (i: number, path: [number, number][]) => path.map(([h, l], k) => ({ t: i * H + k * 60_000, o: (h + l) / 2, h, l, c: (h + l) / 2, v: 1 }));
  const minutes = [...sub60(0, [[100.4, 100.1], [100.3, 99.9], ...Array.from({ length: 58 }, () => [100.3, 100.1] as [number, number])]), ...sub60(1, [[102.5, 100.5], ...Array.from({ length: 59 }, () => [100, 98.5] as [number, number])])];
  const lx = ltfIndex([{ tf: 60_000, c: minutes }]);
  const cons = settleOrder(hc, 0, true, 100, 99, { '2': 102 }, 5, 0), fine = settleOrder(hc, 0, true, 100, 99, { '2': 102 }, 5, 0, { ltf: lx });
  ok('settlement: with only H1 a bar holding stop and target is a loss; minute bars that show the target first make it a 2R win', cons.r['2'] === -1 && fine.r['2'] === 2 && fine.resolvedBy === 'ltf', JSON.stringify({ cons: cons.r, fine: fine.r }));
  const same = settleOrder(hc, 0, true, 100, 99, { '2': 102 }, 5, 0, { ltf: ltfIndex([{ tf: 60_000, c: [] }]) });
  ok('settlement: when no finer bars cover the hour, the result is exactly the conservative one', same.r['2'] === cons.r['2'] && same.resolvedBy === 'h1');
}
// ---- interpreter and regime: only closed bars, salience in range ----
{
  const T = sol[46000]!.t + 20 * 60_000, cut = sol.slice(0, 46001), tampered = sol.map((b, i) => (i >= 46000 ? { ...b, h: b.h * 1.3, l: b.l * 0.7, c: b.c * 1.2 } : b));
  const a = interpret(makeFeed(cut), T, { instrument: SOL, price: sol[46000]!.o }), z = interpret(makeFeed(tampered), T, { instrument: SOL, price: sol[46000]!.o });
  ok('interpreter: the reading at a moment is identical when every bar from the decision bar on is rewritten (no lookahead)', JSON.stringify(a.layers) === JSON.stringify(z.layers) && JSON.stringify(a.salience) === JSON.stringify(z.salience), a.headline);
  ok('interpreter: five layers, salience in [0, 1] for every node, regime present', a.layers.length === 5 && Object.values(a.salience).every((v) => v >= 0 && v <= 1) && a.keys.trend !== 'n/a', JSON.stringify(a.keys));
}
// ---- atlas: one identity per level, hindsight only from bars after first sight ----
{
  const keys = { trend: 'up', vol: 'low', bias: 'long', nLevels: 1, go: false, spaceOk: null };
  const st = (t: number, p: number) => ({ t, r: { levels: [{ p, d: 1 as const, n: 2, tier: 'needs-one-confluence', bsb: 3, ok: true }], atrD1: 10, keys } });
  const ids = mergeLevels([st(0, 100), st(H, 101), st(2 * H, 102.4), st(3 * H, 104)]);
  ok('atlas: the same support seen at 100, 101 and 102.4 in successive scans is one level; 104 (over 0.25 ATR away) is a new one', ids.length === 2 && ids[0]!.seen === 3 && ids[1]!.price === 104, JSON.stringify(ids.map((x) => [x.price, x.seen])));
  // support at 100, zone +/-1, ATR 10: bar 1 sits 3 clear (away), bar 2 dips into the zone (test 1), bar 3 rallies 6 clear (held), bar 4 closes below 98 (dies)
  const bars: Candle[] = [[100, 101, 99, 100], [104, 106, 103.6, 105], [103, 103.5, 100.5, 102], [102, 107.2, 101.5, 107], [104, 104.2, 97, 97.5]].map(([o, h, l, c2], i) => ({ t: i * H, o: o!, h: h!, l: l!, c: c2!, v: 1 }));
  const L: LevelId = { id: 0, dir: 1, price: 100, first: 0, last: 0, seen: 1, firstTouches: 1, firstTier: 'x', firstBsb: 0, firstTrend: 'up', firstVol: 'low', firstBias: 'long', maxTouches: 1, flipOf: null, tests: 0, held: 0, diedAt: null, firstTestAfterBars: null, zone: 1 };
  fate(bars, 0, 4, L);
  ok('atlas: a level is tested, holds, then dies on a close through it, counting only bars after first sight', L.tests === 1 && L.held === 1 && L.diedAt === 4 * H && L.firstTestAfterBars === 2, JSON.stringify(L));
}
void lastClosed;
console.log(`\n${passes} passed, ${fails} failed`);
process.exit(fails ? 1 : 0);
