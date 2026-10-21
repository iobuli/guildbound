"""Use Kev as a trade filter on Ichimoku TK-cross entries (SOLUSDT 1h).

Every TK cross (both directions) is a candidate trade. Kev reads a text snapshot (Ichimoku state, trend, momentum, volatility)
and answers `p(win)`. Prompt variants and the probability threshold are chosen on train+val; the test slice is read once.

  python kev_filter.py --base-url http://127.0.0.1:8008 --model kev-latest
"""
import argparse
import json
import os

import numpy as np
import pandas as pd

from ichi_opt import fmt, ichimoku, load, run, stats

CFG = dict(t=9, k=26, b=52, use_cloud=False, use_chikou=False, use_future=False, long_only=False, atr_stop=4, max_hold=48)


def snapshot(df, ich, atr, ts, side):
    i = df.index.get_loc(ts)
    c = df.close.iloc[i]
    cloud = "above" if c > ich.top.iloc[i] else "below" if c < ich.bot.iloc[i] else "inside"
    fut = "bullish (span A over B)" if ich.fa.iloc[i] > ich.fb.iloc[i] else "bearish (span A under B)"
    kslope = (ich.kijun.iloc[i] / ich.kijun.iloc[i - 5] - 1) * 100
    r24, r120 = (c / df.close.iloc[i - 24] - 1) * 100, (c / df.close.iloc[i - 120] - 1) * 100
    chik = "above" if ich.chikou_ok_up.iloc[i] else "below"
    return {
        "setup": f"Ichimoku Tenkan/Kijun cross signalling a {'LONG' if side == 1 else 'SHORT'} entry on SOL/USDT 1-hour chart",
        "price_vs_cloud": f"{cloud} the cloud; future cloud is {fut}",
        "chikou_span": f"lagging span is {chik} the price of 26 bars ago",
        "kijun_slope": f"Kijun changed {kslope:+.2f}% over the last 5 bars",
        "momentum": f"24h return {r24:+.1f}%, 5-day return {r120:+.1f}%",
        "volatility": f"ATR is {atr.iloc[i] / c * 100:.2f}% of price",
        "session": f"{ts.day_name()} {ts.hour:02d}:00 UTC",
    }


QUESTIONS = {
    "profit": "Will this trade be profitable after fees if held for up to 48 hours?",
    "trend": "Is the market in a clean trend (rather than choppy consolidation) that agrees with the signal direction?",
}


def ask(client, Noul, state):
    r = client.system_one(state=state, questions={k: Noul(instructions=v) for k, v in QUESTIONS.items()})
    return {k: r.nouls[k].noul for k in QUESTIONS}


def auc(y, s):
    y, s = np.asarray(y), np.asarray(s)
    pos, neg = s[y == 1], s[y == 0]
    if len(pos) == 0 or len(neg) == 0: return float("nan")
    return float((pos[:, None] > neg[None, :]).mean() + 0.5 * (pos[:, None] == neg[None, :]).mean())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", default="http://127.0.0.1:8008"); ap.add_argument("--model", default="kev-latest")
    ap.add_argument("--api-key", default="local"); ap.add_argument("--cache", default="kev_cache.json")
    ap.add_argument("--max-trades", type=int, default=0, help="cap candidates per split (0 = all)")
    a = ap.parse_args()
    from typesafe_sdk import Noul, TypeSafeClient
    client = TypeSafeClient(api_key=a.api_key, base_url=a.base_url, model=a.model)

    df = load()
    ich = ichimoku(df, CFG["t"], CFG["k"], CFG["b"])
    tr_ = pd.concat([df.high - df.low, (df.high - df.close.shift()).abs(), (df.low - df.close.shift()).abs()], axis=1).max(axis=1)
    atr = tr_.rolling(14).mean()
    trades = run(df, CFG["t"], CFG["k"], CFG["b"], CFG["use_cloud"], CFG["use_chikou"], CFG["use_future"], CFG["long_only"], CFG["atr_stop"], CFG["max_hold"])
    trades = trades[trades.index >= df.index[130]]
    n = len(df); a1, b1 = df.index[int(n * 0.5)], df.index[int(n * 0.75)]
    trades["split"] = np.where(trades.index < a1, "train", np.where(trades.index < b1, "val", "test"))
    if a.max_trades:
        trades = pd.concat([g.iloc[np.linspace(0, len(g) - 1, min(len(g), a.max_trades)).astype(int)] for _, g in trades.groupby("split")])
    print(trades.groupby("split").ret.agg(["count", "mean", lambda x: (x > 0).mean()]).rename(columns={"<lambda_0>": "win"}))

    cache = json.load(open(a.cache)) if os.path.exists(a.cache) else {}
    ps = []
    for k, (ts, row) in enumerate(trades.iterrows()):
        key = f"{a.model}|{ts}|{row.side}"
        if key not in cache:
            cache[key] = ask(client, Noul, snapshot(df, ich, atr, ts, int(row.side)))
        ps.append(cache[key])
        if k % 50 == 0: json.dump(cache, open(a.cache, "w")); print(f"  {k}/{len(trades)}", flush=True)
    json.dump(cache, open(a.cache, "w"))
    trades["p_profit"] = [p["profit"] for p in ps]; trades["p_trend"] = [p["trend"] for p in ps]
    trades["p_both"] = np.sqrt(trades.p_profit * trades.p_trend)

    print("\nAUC (does Kev's score rank winners above losers?)  0.5 = no skill")
    for s in ("train", "val", "test"):
        d = trades[trades.split == s]
        print(f"  {s:5s} n={len(d):4d}  " + "  ".join(f"{c}={auc((d.ret > 0).astype(int), d[c]):.3f}" for c in ("p_profit", "p_trend", "p_both")))

    print("\nThreshold sweep on train+val (keep trades with score >= quantile)")
    tv = trades[trades.split != "test"]
    best = None
    for col in ("p_profit", "p_trend", "p_both"):
        for q in (0.0, 0.3, 0.5, 0.7):
            thr = tv[col].quantile(q)
            s = stats(tv[tv[col] >= thr])
            print(f"  {col:8s} q={q:.1f} thr={thr:.3f}  {fmt(s)}")
            if q > 0 and (best is None or s["sharpe"] > best[0]): best = (s["sharpe"], col, thr)
    _, col, thr = best
    te = trades[trades.split == "test"]
    print(f"\nCHOSEN on train+val: {col} >= {thr:.3f}")
    print("  TEST unfiltered:", fmt(stats(te)))
    print("  TEST Kev-filtered:", fmt(stats(te[te[col] >= thr])))
    rnd = np.mean([stats(te.sample(frac=(te[col] >= thr).mean(), random_state=s))["avg"] for s in range(200)])
    print(f"  TEST random subset of same size, mean avg/trade over 200 draws: {rnd:+.3f}%")


if __name__ == "__main__":
    main()
