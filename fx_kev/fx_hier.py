"""Hierarchical multi-timeframe Kev strategy on USDJPY (D1 -> H1 -> M15), built so no decision can see the future.

Rules that enforce this:
  * Every bar gets avail = open_time + duration (the moment it is fully closed). A decision at time t only sees bars with avail <= t.
  * Fixed context: D1 CTX_D bars, H1 CTX_H bars, M15 CTX_M bars of closed history are required for every decision; the common
    start is the first t at which ALL timeframes have that much, so every method is scored on the identical decision set.
  * Entry fills at the open of the first M15 bar at or after t (never at a price known before t).
  * Chronological train/val/test; trades that would cross a split boundary are dropped (embargo). Test is read once.
  * `leak_test` corrupts every bar after t and checks the state texts are unchanged.
Levels: D1 = regime, H1 = setup (Tenkan/Kijun cross candidates), M15 = entry timing. Kev answers one Noul per level.
"""
import argparse
import json
import os

import numpy as np
import pandas as pd

from ichi_opt import ichimoku, load

U = "/root/.claude/uploads/e6c1b55c-a96f-536d-adc8-7a7224cce5ae/"
TF = {"D1": ("07a122c8-USDJPY1440.csv", pd.Timedelta("1D")), "H1": ("bf881312-USDJPY60_1.csv", pd.Timedelta("1h")),
      "M15": ("9dd65c88-USDJPY15.csv", pd.Timedelta("15min"))}
CTX = {"D1": 120, "H1": 240, "M15": 200}   # constant minimum context (closed bars) for every decision
WARM = 52 + 26                              # Ichimoku warm-up
FEE, STOP_ATR, MAX_HOLD_H = 0.00004, 4.0, 48


def atr(df, n=14):
    tr = pd.concat([df.high - df.low, (df.high - df.close.shift()).abs(), (df.low - df.close.shift()).abs()], axis=1).max(axis=1)
    return tr.rolling(n).mean()


def rsi(c, n=14):
    d = c.diff()
    up, dn = d.clip(lower=0).rolling(n).mean(), (-d.clip(upper=0)).rolling(n).mean()
    return 100 - 100 / (1 + up / dn.replace(0, np.nan))


def feature_frame(raw, dur):
    f = ichimoku(raw, 9, 26, 52)
    f["close"], f["atr"], f["rsi"] = raw.close, atr(raw), rsi(raw.close)
    f["ret24"], f["ret20"] = raw.close.pct_change(24) * 100, raw.close.pct_change(20) * 100
    f["kslope"] = (f.kijun / f.kijun.shift(5) - 1) * 100
    f["rng16"] = (raw.close - raw.low.rolling(16).min()) / (raw.high.rolling(16).max() - raw.low.rolling(16).min())
    f["avail"] = raw.index + dur
    f["n"] = np.arange(1, len(raw) + 1)
    return f.set_index("avail")


def last_closed(f, t):
    i = f.index.searchsorted(t, side="right") - 1
    return None if i < 0 else f.iloc[i]


def cloud(r):
    return "above" if r.close > r.top else "below" if r.close < r.bot else "inside"


def states(feats, t, side):
    d, h, m = (last_closed(feats[k], t) for k in ("D1", "H1", "M15"))
    s = "LONG" if side == 1 else "SHORT"
    return {
        "D1": {"context": f"USD/JPY DAILY chart. Proposed position: {s}.",
               "cloud": f"price is {cloud(d)} the daily cloud; Tenkan is {'above' if d.tenkan > d.kijun else 'below'} Kijun; lagging span is {'above' if d.chikou_ok_up else 'below'} price 26 days ago",
               "future_cloud": "bullish" if d.fa > d.fb else "bearish",
               "trend": f"Kijun slope {d.kslope:+.2f}% over 5 days; 20-day return {d.ret20:+.1f}%; RSI {d.rsi:.0f}",
               "volatility": f"daily ATR {d.atr / d.close * 100:.2f}% of price"},
        "H1": {"context": f"USD/JPY 1-HOUR chart. A Tenkan/Kijun cross just signalled a {s} entry.",
               "cloud": f"price is {cloud(h)} the cloud; lagging span is {'above' if h.chikou_ok_up else 'below'} price 26 bars ago",
               "future_cloud": "bullish" if h.fa > h.fb else "bearish",
               "trend": f"Kijun slope {h.kslope:+.3f}% over 5 bars; 24-bar return {h.ret24:+.2f}%; RSI {h.rsi:.0f}",
               "volatility": f"hourly ATR {h.atr / h.close * 100:.3f}% of price", "session": f"{t.day_name()} {t.hour:02d}:00"},
        "M15": {"context": f"USD/JPY 15-MINUTE chart, deciding whether to enter a {s} now.",
                "range": f"price sits at {m.rng16 * 100:.0f}% of the last 4-hour range",
                "trend": f"15m Tenkan is {'above' if m.tenkan > m.kijun else 'below'} Kijun; price is {cloud(m)} the 15m cloud; RSI {m.rsi:.0f}",
                "volatility": f"15m ATR {m.atr / m.close * 100:.3f}% of price"},
    }


Q = {"D1": "Does the daily picture support holding this position over the next two days?",
     "H1": "Is this a valid, high-quality trend entry on the hourly chart (not choppy consolidation)?",
     "M15": "Is the 15-minute price action a good moment to enter in this direction right now?"}


def build():
    raw = {k: load(U + v[0]) for k, v in TF.items()}
    feats = {k: feature_frame(raw[k], TF[k][1]) for k in TF}
    start = max(feats[k].index[CTX[k] + WARM] for k in TF)   # first closed-bar time at which every TF has full context
    return raw, feats, start


def candidates(raw, feats, start):
    """H1 TK crosses (both directions) at H1 bar close. Entry at first M15 open >= t."""
    h = feats["H1"]
    cu = (h.tenkan > h.kijun) & (h.tenkan.shift() <= h.kijun.shift())
    cd = (h.tenkan < h.kijun) & (h.tenkan.shift() >= h.kijun.shift())
    m = raw["M15"]
    lo, hi, op = m.low.values, m.high.values, m.open.values
    rows = []
    for t in h.index[(cu | cd) & (h.index >= start)]:
        side = 1 if cu.loc[t] else -1
        j = m.index.searchsorted(t, side="left")
        end = j + MAX_HOLD_H * 4
        if end >= len(m) or (m.index[j] - t) > pd.Timedelta("1h"):
            continue
        entry, a = op[j], h.atr.loc[t]
        stop = entry - side * STOP_ATR * a
        exit_p = m.close.iloc[end]
        hit = np.nonzero((lo[j:end + 1] <= stop) if side == 1 else (hi[j:end + 1] >= stop))[0]
        if len(hit):
            exit_p = stop
        rows.append((t, side, side * (exit_p / entry - 1) - 2 * FEE, m.index[end]))
    return pd.DataFrame(rows, columns=["t", "side", "ret", "t_exit"]).set_index("t")


def leak_test(raw, cands, feats, n=12, seed=0):
    """Corrupt every bar that closes after t; the three state texts must not change."""
    rng = np.random.default_rng(seed)
    bad = 0
    for t in cands.index[rng.choice(len(cands), n, replace=False)]:
        side = int(cands.side.loc[t])
        base = states(feats, t, side)
        f2 = {}
        for k, (_, dur) in TF.items():
            r = raw[k].copy()
            mask = (r.index + dur) > t
            r.loc[mask, ["open", "high", "low", "close"]] = rng.uniform(50, 250, (mask.sum(), 4))
            f2[k] = feature_frame(r, dur)
        if states(f2, t, side) != base:
            bad += 1
            print("LEAK at", t)
    print(f"leak test: {n - bad}/{n} decision times unchanged after corrupting all future bars")
    return bad == 0


def auc(y, s):
    y, s = np.asarray(y), np.asarray(s)
    p, q = s[y == 1], s[y == 0]
    return float("nan") if len(p) == 0 or len(q) == 0 else float((p[:, None] > q[None, :]).mean() + .5 * (p[:, None] == q[None, :]).mean())


def st(r):
    if len(r) < 15:
        return dict(n=len(r), total=np.nan, avg=np.nan, win=np.nan, sharpe=-9.0)
    return dict(n=len(r), total=(1 + r).prod() - 1, avg=r.mean() * 100, win=(r > 0).mean(), sharpe=r.mean() / r.std() * np.sqrt(len(r)))


def fm(s):
    return f"n={s['n']:4d} total={s['total']:+7.1%} avg={s['avg']:+.3f}% win={s['win']:.1%} sharpe={s['sharpe']:+.2f}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", default="http://127.0.0.1:8008"); ap.add_argument("--model", default="kev-latest")
    ap.add_argument("--cache", default="hier_cache.json"); ap.add_argument("--max-per-split", type=int, default=0)
    ap.add_argument("--no-kev", action="store_true")
    a = ap.parse_args()
    raw, feats, start = build()
    print("common start (all timeframes have full context):", start, "| data end:", raw["M15"].index[-1])
    c = candidates(raw, feats, start)
    lo, hi = c.index[0], c.index[-1]
    b1, b2 = lo + (hi - lo) * .5, lo + (hi - lo) * .75
    c["split"] = np.where(c.t_exit < b1, "train", np.where((c.index >= b1) & (c.t_exit < b2), "val", np.where(c.index >= b2, "test", "drop")))
    c = c[c.split != "drop"]
    print(c.groupby("split").ret.agg(n="count", avg="mean", win=lambda x: (x > 0).mean()))
    if not leak_test(raw, c, feats):
        raise SystemExit("lookahead detected, stopping")
    if a.max_per_split:
        c = pd.concat([g.iloc[np.linspace(0, len(g) - 1, min(len(g), a.max_per_split)).astype(int)] for _, g in c.groupby("split")])
    if a.no_kev:
        return
    from typesafe_sdk import Noul, TypeSafeClient
    cl = TypeSafeClient(api_key="local", base_url=a.base_url, model=a.model)
    cache = json.load(open(a.cache)) if os.path.exists(a.cache) else {}
    scores = {k: [] for k in TF}
    for i, (t, r) in enumerate(c.iterrows()):
        sts = states(feats, t, int(r.side))
        for k in TF:
            key = f"{a.model}|{k}|{t}|{int(r.side)}"
            if key not in cache:
                cache[key] = cl.system_one(state=sts[k], questions={"q": Noul(instructions=Q[k])}).nouls["q"].noul
            scores[k].append(cache[key])
        if i % 50 == 0:
            json.dump(cache, open(a.cache, "w")); print(f"  {i}/{len(c)}", flush=True)
    json.dump(cache, open(a.cache, "w"))
    for k in TF:
        c["p_" + k] = scores[k]
    print("\nAUC by level (0.5 = no skill)")
    for s in ("train", "val", "test"):
        d = c[c.split == s]
        print(f"  {s:5s} n={len(d):4d} " + " ".join(f"{k}={auc((d.ret > 0).astype(int), d['p_' + k]):.3f}" for k in TF))
    tv, te = c[c.split != "test"], c[c.split == "test"]
    best = None
    for qd in (0, .3, .5):
        for qh in (0, .3, .5):
            for qm in (0, .3, .5):
                th = (tv.p_D1.quantile(qd), tv.p_H1.quantile(qh), tv.p_M15.quantile(qm))
                f = lambda d: d[(d.p_D1 >= th[0]) & (d.p_H1 >= th[1]) & (d.p_M15 >= th[2])]
                s_tr, s_va = st(f(c[c.split == "train"]).ret), st(f(c[c.split == "val"]).ret)
                print(f"  gate q=({qd},{qh},{qm}) train {fm(s_tr)} | val {fm(s_va)}")
                if s_tr["n"] >= 15 and s_va["n"] >= 15 and (best is None or min(s_tr["sharpe"], s_va["sharpe"]) > best[0]):
                    best = (min(s_tr["sharpe"], s_va["sharpe"]), (qd, qh, qm), th)
    _, qs, th = best
    sel = te[(te.p_D1 >= th[0]) & (te.p_H1 >= th[1]) & (te.p_M15 >= th[2])]
    print(f"\nCHOSEN on train+val (worst-of-two Sharpe): quantile gates D1/H1/M15 = {qs}")
    print("  TEST unfiltered       :", fm(st(te.ret)))
    print("  TEST hierarchical Kev :", fm(st(sel.ret)))
    if len(sel) >= 15:
        rs = [st(te.sample(len(sel), random_state=i).ret)["sharpe"] for i in range(300)]
        print(f"  TEST random subsets of same size: mean Sharpe {np.nanmean(rs):+.2f}; Kev beats {np.mean([st(sel.ret)['sharpe'] > x for x in rs]):.0%} of draws")
    for k in TF:
        d = te[te["p_" + k] >= tv["p_" + k].quantile(.5)]
        print(f"  TEST single level {k} (top half):", fm(st(d.ret)))


if __name__ == "__main__":
    main()
