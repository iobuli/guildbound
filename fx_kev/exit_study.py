"""Does the trade anatomy (losers fail fast, winners run ~37h) give an exploitable exit rule? Causal rules only:
each rule at bar k uses only bars <= k (high/low of bar k are checked before the close-based rules at k). Costs included.
"""
import numpy as np
import pandas as pd

from fx_hier import FEE, TF, U
from ichi_opt import load

HOLD, STOP = 96, 4.0     # allow up to 96h so trailing rules have room; baseline row uses 48h to match earlier work


def sim(o, hi, lo, cl, j, side, a, rule):
    """Return net R for one trade. rule = dict(hold, stop, early=(bars, min_R), trail=(act_R, dist_ATR), be=(act_R))."""
    entry = o[j]; stop = entry - side * rule.get("stop", STOP) * a; hold = rule.get("hold", 48)
    peak = 0.0; be_on = False
    for k in range(hold):
        i = j + k
        fav_hi = side * ((hi[i] if side == 1 else lo[i]) - entry) / a          # best excursion this bar (ATR)
        adv_lo = side * ((lo[i] if side == 1 else hi[i]) - entry) / a          # worst excursion this bar
        s_R = side * (stop - entry) / a
        if adv_lo <= s_R:                                                       # stop first (conservative)
            return s_R, k
        peak = max(peak, fav_hi)
        c = side * (cl[i] - entry) / a
        if "trail" in rule and peak >= rule["trail"][0]:
            stop_R = peak - rule["trail"][1]
            stop = entry + side * max(side * (stop - entry), stop_R * a) if False else entry + side * max(side * (stop - entry) / a, stop_R) * a
        if "be" in rule and peak >= rule["be"] and not be_on:
            stop = entry + side * max(side * (stop - entry) / a, 0.0) * a; be_on = True
        if "early" in rule and k + 1 == rule["early"][0] and c < rule["early"][1]:
            return c, k
    return side * (cl[j + hold - 1] - entry) / a, hold - 1


def main():
    X = pd.read_pickle("/home/user/fx_kev/pattern_X.pkl")
    h = load(U + TF["H1"][0]); idx = h.index
    o, hi, lo, cl = (h[c].values for c in ("open", "high", "low", "close"))
    from fx_hier import atr, feature_frame
    f = feature_frame(h, TF["H1"][1])
    rules = {
        "baseline 48h, 4ATR stop": dict(hold=48),
        "hold 96h": dict(hold=96),
        "early exit 6h if R<0": dict(hold=48, early=(6, 0.0)),
        "early exit 6h if R<-1": dict(hold=48, early=(6, -1.0)),
        "early exit 12h if R<0": dict(hold=48, early=(12, 0.0)),
        "early exit 12h if R<+0.5": dict(hold=48, early=(12, 0.5)),
        "tight stop 2ATR": dict(hold=48, stop=2.0),
        "stop 3ATR": dict(hold=48, stop=3.0),
        "breakeven after +2ATR": dict(hold=48, be=2.0),
        "trail 3ATR after +3ATR": dict(hold=96, trail=(3.0, 3.0)),
        "trail 2ATR after +4ATR": dict(hold=96, trail=(4.0, 2.0)),
        "trail 4ATR after +2ATR": dict(hold=96, trail=(2.0, 4.0)),
        "early6h<0 + trail 3/3 (96h)": dict(hold=96, early=(6, 0.0), trail=(3.0, 3.0)),
    }
    res = {name: [] for name in rules}
    meta = []
    for t, r in X.iterrows():
        j = idx.searchsorted(t, side="left")
        if j + 100 >= len(idx): continue
        a = f.atr.loc[t]; cost_R = 2 * FEE * o[j] / a
        for name, rule in rules.items():
            R, _ = sim(o, hi, lo, cl, j, int(r.side), a, rule)
            res[name].append(R - cost_R)
        meta.append((t, int(r.side), r.split))
    M = pd.DataFrame(res, index=[m[0] for m in meta]); M["side"] = [m[1] for m in meta]; M["split"] = [m[2] for m in meta]
    for label, sub in (("ALL TRADES (long+short)", M), ("LONG ONLY", M[M.side == 1])):
        print(f"\n===== {label}: net mean R per trade (higher is better) | win% | Sharpe-like (mean/std*sqrt(n)) =====")
        rows = []
        for name in rules:
            r = {"rule": name}
            for s in ("train", "val", "test"):
                x = sub[sub.split == s][name]
                r[s] = round(x.mean(), 3); r[s + "_sh"] = round(x.mean() / x.std() * np.sqrt(len(x)), 2)
            rows.append(r)
        print(pd.DataFrame(rows).set_index("rule").to_string())
    M.to_pickle("/home/user/fx_kev/exit_M.pkl")


if __name__ == "__main__":
    main()
