"""Ichimoku grid search on USDJPY files (train 50% / val 25% / test 25%). Cost: ~1 pip round trip."""
import itertools, sys
import pandas as pd
from ichi_opt import load, run, stats, fmt

U = "/root/.claude/uploads/e6c1b55c-a96f-536d-adc8-7a7224cce5ae/"
FILES = {"D1": ("07a122c8-USDJPY1440.csv", [10, 40]), "H1": ("bf881312-USDJPY60_1.csv", [48, 200])}
FEE = 0.00004  # per side as fraction of price (~0.6 pip at 150)


def main(tf):
    fn, holds = FILES[tf]
    df = load(U + fn); n = len(df); a, b = int(n * .5), int(n * .75)
    tr, va, te = df.iloc[:a], df.iloc[a:b], df.iloc[b:]
    bh = lambda d: d.close.iloc[-1] / d.close.iloc[0] - 1
    print(f"\n##### {tf}: bars={n} train ..{tr.index[-1].date()} val ..{va.index[-1].date()} test ..{te.index[-1].date()}  buy&hold {bh(tr):+.0%}/{bh(va):+.0%}/{bh(te):+.0%}")
    grid = [g for g in itertools.product([5, 9, 12, 20], [20, 26, 40, 60], [52, 100], [False, True], [False, True], [False, True], [False, True], [2, 4], holds) if g[0] < g[1]]
    res = [(g, stats(run(tr, *g, fee=FEE)), stats(run(va, *g, fee=FEE))) for g in grid]
    res.sort(key=lambda r: r[1]["sharpe"], reverse=True)
    top = [r for r in res[:40] if r[1]["total"] > 0]; top.sort(key=lambda r: r[2]["sharpe"], reverse=True)
    for g, s1, s2 in top[:3]: print(g, "\n   train", fmt(s1), "\n   val  ", fmt(s2))
    best = top[0][0] if top else res[0][0]
    print("CHOSEN", best, "\n   TEST", fmt(stats(run(te, *best, fee=FEE))))
    print("   default 9/26/52 unfiltered L+S  TEST", fmt(stats(run(te, 9, 26, 52, False, False, False, False, 4, holds[1], fee=FEE))))
    print("   default 9/26/52 all filters L+S TEST", fmt(stats(run(te, 9, 26, 52, True, True, True, False, 4, holds[1], fee=FEE))))
    pos = sum(1 for g, s1, s2 in res if s1["total"] > 0 and s2["total"] > 0 and stats(run(te, *g, fee=FEE))["total"] > 0) if False else None


main(sys.argv[1])
