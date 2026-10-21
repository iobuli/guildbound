import { defineNode } from '../core/node.js';
import { RiskPort, StopPort } from '../core/ports.js';
import { rule, src } from '../core/rule.js';

export default defineNode({
  id: 'risk',
  title: 'Risk and sizing',
  stage: 'R',
  summary: 'Turns the stop distance into a position size for a fixed percentage risk. The rules about adding and not cutting size live here as rules.',
  decideAt: 'fill_time',
  step: { n: 8, label: 'Risk: size from the stop distance' },
  inputs: { stop: StopPort },
  outputs: { risk: RiskPort },
  rules: [
    rule('risk.always-one-r', '"Always 1R. I never cut size." Cutting size right after a loss makes no sense. 1R can shrink as a share of the account in a drawdown because risk is a fixed percentage.', {
      sources: [src('tweet', 'curated tweets', '2026-09-29')],
    }),
    rule('risk.size-from-stop', 'Position size = account risk / stop distance, with risk a fixed percentage of the account.', {
      codable: true, status: 'inferred', sources: [src('tweet', 'curated tweets', '2026-09-29'), src('backtest', 'user-supplied twelve-bar page')], note: 'The backtest used 0.5% risk per trade; the percentage in live use is not stated.',
    }),
    rule('risk.adding-keeps-risk-controlled', 'Add to a trade that is confirming your thesis if the addition fits the plan, the upside is still asymmetric and total risk stays controlled. Adding on H1 price action to a blind D1 bid leaves risk at 1R overall because the first stop is near breakeven.', {
      sources: [src('tweet', 'curated tweets', '2026-06-03'), src('tweet', 'curated tweets', '2026-05-01')],
    }),
    rule('risk.review-in-drawdown', 'Reviewing strategies in a -10R or -20R drawdown.', {
      kind: 'heuristic', status: 'observed', sources: [src('webinar', 'curated/notes-images.md', '2024-09-30')],
    }),
    rule('risk.correlated-setups', 'Rule 4, "know when to break the rules": he took a second highly correlated setup (breaking his usual rule) only because he had seen exactly one loss under that setup.', {
      kind: 'heuristic', sources: [src('webinar', 'user-pasted notes', '2024-09-24')],
    }),
    rule('risk.slippage-unmeasured', 'The backtest charged a flat 0.10R of slippage. That figure is unmeasured, especially for gold in thin hours and a crypto venue with funding and depth limits. "Nothing here should be sized until it is."', {
      status: 'backtest-reported', sources: [src('backtest', 'user-supplied forensic page', '2026-09-13')],
    }),
    rule('risk.long-losing-runs', 'A 29% win rate means long stretches of being wrong; the worst losing run is the price of the far target.', {
      status: 'backtest-reported', sources: [src('backtest', 'user-supplied forensic page', '2026-09-13')],
    }),
  ],
  evaluate: (inp, _v, setup) => {
    const amount = (setup.equity * setup.riskPct) / 100, d = inp.stop.riskDist;
    return { out: { risk: { riskAmount: amount, riskDistance: d, size: d > 0 ? amount / d : 0, ok: d > 0 } }, checks: [{ rule: 'risk.size-from-stop', pass: d > 0, detail: d > 0 ? `risking ${amount.toFixed(2)} over a ${d.toFixed(4)} stop = ${(amount / d).toFixed(4)} units` : 'no valid stop distance' }] };
  },
});
