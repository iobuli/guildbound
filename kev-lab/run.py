"""python3 kev-lab/run.py: score the four candidates on 2020-2022, freeze the best, run the $100k book on 2023+, write out/lab/results.json."""
import json, sys, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import data, strategy, quant, iterate
import numpy as np

def clean(x):
    """JSON has no NaN/Infinity: write null so the page can parse it."""
    if isinstance(x, dict): return {k: clean(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)): return [clean(v) for v in x]
    if isinstance(x, (float, np.floating)): return float(x) if np.isfinite(x) else None
    return x

def main():
    t0 = time.time()
    sig = {m: data.signals(m) for m in data.markets() if (data.SIGNALS / f"{m}.json").exists()}
    res = strategy.run_all(sig)
    # regime statistics per market and year (Hurst and variance ratio on hourly log returns)
    reg = {}
    for m in sig:
        c = data.h1(m); lr = np.diff(np.log(c["c"])); yrs = (c["t"][1:] / 1000 / 86400 / 365.25 + 1970).astype(int)
        reg[m] = {int(y): {"hurst": quant.hurst(lr[yrs == y]), "vr24": quant.variance_ratio(lr[yrs == y], 24)[0], "z": quant.variance_ratio(lr[yrs == y], 24)[1]} for y in sorted(set(yrs)) if (yrs == y).sum() > 500}
    res["regime"] = reg
    # iteration 1: walk-forward self-selection (primary DSR >= 0.90, sensitivity 0.50) and the frozen live choice
    res["walk"] = {str(th): iterate.walk_forward(sig, th) for th in (0.9, 0.5)}
    res["live_choice"] = iterate.live_choice(sig, 0.9)
    res["generated"] = int(time.time() * 1000)
    out = data.ROOT / "out" / "lab"; out.mkdir(parents=True, exist_ok=True)
    (out / "results.json").write_text(json.dumps(clean(res), default=float))
    print(f"done in {time.time()-t0:.0f}s")
    return res

if __name__ == "__main__":
    r = main()
    f = lambda x, d=2: f"{x:.{d}f}"
    print("\nDesign 2020-2022 (0.5% risk, $100k):")
    for k, m in r["candidates"].items():
        print(f"  {k:28s} trades {m['trades']:4d} meanR {f(m['meanR'])} win {f(100*m['win'],0)}% final ${m['final']:,.0f} sharpe {f(m['sharpe'])} maxDD {f(100*m['max_dd'],1)}% DSR {f(m['dsr'])} PSR {f(m['psr'])}")
    print(f"\nChosen: {r['chosen']}")
    fw, pl = r["forward"], r["placebo"]
    print(f"Forward 2023+: trades {fw['trades']} meanR {f(fw['meanR'])} win {f(100*fw['win'],0)}% final ${fw['final']:,.0f} CAGR {f(100*fw['cagr'],1)}% sharpe {f(fw['sharpe'])} maxDD {f(100*fw['max_dd'],1)}% PSR {f(fw['psr'])} skipped {fw['skipped']}")
    print(f"Placebo book: trades {pl['trades']} meanR {f(pl['meanR'])} final ${pl['final']:,.0f} sharpe {f(pl['sharpe'])}")
    mc = r["montecarlo"]; print(f"Monte Carlo: P(loss) {f(100*mc['p_loss'],0)}%, final 5/50/95% {[round(x) for x in mc['final_q']]}, maxDD 5/50/95% {[round(100*x,1) for x in mc['dd_q']]}")
    print("Per market:", {k: (v['trades'], round(v['pnl']), round(v['meanR'], 3)) for k, v in r["per_market"].items()})
