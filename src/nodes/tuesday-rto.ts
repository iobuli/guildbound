import { defineNode } from '../core/node.js';
import { TuesdayPort } from '../core/ports.js';
import { rule, src } from '../core/rule.js';
import { P } from '../core/params.js';
import { bodyOf, rng } from '../core/ta.js';

export default defineNode({
  id: 'tuesday-rto',
  title: 'Tuesday weekly-open retest',
  stage: 'E',
  summary: 'A self-contained setup: on a Tuesday, if the weekly open has not been touched, trade an H1 pinbar toward it with the pin as the stop.',
  decideAt: 'prior_bars',
  step: { n: 0.3, label: 'Tuesday weekly-open setup' },
  inputs: {},
  outputs: { tuesday: TuesdayPort },
  rules: [
    rule('tuesday-rto.rules', 'Tuesday only. The weekly open must not have been touched since 22:00 on Monday (and not so far on Tuesday). Buy or sell with the weekly open as the target.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-tuesday-rto.md', '2025-01-10')],
    }),
    rule('tuesday-rto.getting-in', 'Enter on a multiple-tap level or an H1 pinbar. Weekly open above price: buy a bullish pin. Weekly open below: sell a bearish pin.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-tuesday-rto.md', '2025-01-10')], note: 'Only the pinbar route is coded; the multiple-tap-level route needs a level and is left to the level node.',
    }),
    rule('tuesday-rto.orders', 'Buy the top of the pin or sell its low. The stop goes on the other side of the pinbar. Only take trades that are at least 1R.', {
      codable: true, sources: [src('lesson', 'curated/taking-a-trade-tuesday-rto.md', '2025-01-10')],
    }),
    rule('tuesday-rto.holds-past-tuesday', 'It does not matter if the trade runs into Wednesday; only the entry day counts.', {
      sources: [src('lesson', 'curated/taking-a-trade-tuesday-rto.md', '2025-01-10')],
    }),
    rule('tuesday-rto.weekly-open-definition', 'The weekly open is the first bar of the trading week: Sunday 17:00 New York for FX and metals (the 22:00 UK open the sources refer to), Monday 00:00 UTC for crypto. \"Since 22:00 on Monday\" is read on the London clock.', {
      status: 'inferred', codable: true, sources: [src('lesson', 'curated/taking-a-trade-tuesday-rto.md', '2025-01-10')],
    }),
  ],
  evaluate: (_i, v) => {
    const reasons: string[] = [], c = v.h1;
    const out = (valid: boolean, f: Partial<{ dir: 'long' | 'short'; entry: number; stop: number; target: number; rr: number }> = {}) =>
      ({ out: { tuesday: { valid, dir: f.dir ?? null, entry: f.entry ?? null, stop: f.stop ?? null, target: f.target ?? null, rr: f.rr ?? null, reasons } } });
    if (c.length < 30) { reasons.push('not enough bars'); return out(false); }
    if (v.session.weekday(v.asOf) !== 2) { reasons.push('not a Tuesday (trading day)'); return out(false); }
    const ws = v.session.weekStart(v.asOf), wi = c.findIndex((b) => b.t >= ws), since = v.session.wallClock(v.session.label(v.asOf) - 86_400_000, 22);
    if (wi < 0) { reasons.push('no bars for this week'); return out(false); }
    const wo = c[wi]!.o;
    if (c.some((b) => b.t >= since && b.l <= wo && wo <= b.h)) { reasons.push('weekly open already touched since Monday 22:00 London'); return out(false); }
    const pin = c[c.length - 1]!, r = rng(pin);
    const lower = r > 0 ? (Math.min(pin.o, pin.c) - pin.l) / r : 0, upper = r > 0 ? (pin.h - Math.max(pin.o, pin.c)) / r : 0;
    const isPin = r > 0 && bodyOf(pin) <= P.pinBodyMax * r;
    if (wo > pin.c) {
      if (!(isPin && lower >= P.pinWickMin)) { reasons.push('weekly open is above price but the last H1 is not a bullish pin'); return out(false); }
      const rr = (wo - pin.h) / (pin.h - pin.l);
      if (rr < 1) { reasons.push(`only ${rr.toFixed(2)}R to the weekly open`); return out(false, { dir: 'long', entry: pin.h, stop: pin.l, target: wo, rr }); }
      return out(true, { dir: 'long', entry: pin.h, stop: pin.l, target: wo, rr });
    }
    if (!(isPin && upper >= P.pinWickMin)) { reasons.push('weekly open is below price but the last H1 is not a bearish pin'); return out(false); }
    const rr = (pin.l - wo) / (pin.h - pin.l);
    if (rr < 1) { reasons.push(`only ${rr.toFixed(2)}R to the weekly open`); return out(false, { dir: 'short', entry: pin.l, stop: pin.h, target: wo, rr }); }
    return out(true, { dir: 'short', entry: pin.l, stop: pin.h, target: wo, rr });
  },
});
