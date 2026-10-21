"""N2: cross-metal SMT divergence (frozen rule in ledger.py wave 6)."""
import json, os, sys
import numpy as np, pandas as pd
sys.path.insert(0, os.path.dirname(__file__))
from sweep_lib import load, levels, atr, outcome, boot_ci
from ledger import METHODS
from run_ledger import thin

def legs():
    g, s = load("XAUUSD_H1", True), load("XAGUSD_H1", True)
    idx = g.index.intersection(s.index); g, s = g.loc[idx], s.loc[idx]
    out = {}
    for a_name, a, b in (("XAUUSD_H1", g, s), ("XAGUSD_H1", s, g)):
        La, Lb = levels(a, a_name), levels(b, "XAGUSD_H1" if a_name == "XAUUSD_H1" else "XAUUSD_H1")
        long_ = (a.low < La.PDL) & (a.close > La.PDL) & (b.low > Lb.PDL)
        short = (a.high > La.PDH) & (a.close < La.PDH) & (b.high < Lb.PDH)
        A = atr(a).shift(1); rows = []
        for m, sd in ((long_, 1), (short, -1)):
            for t in a.index[m.fillna(False).values]:
                i = a.index.get_loc(t); rows.append((t, i, sd, a.low.iloc[i] if sd == 1 else a.high.iloc[i], A.iloc[i]))
        E = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t").sort_index().dropna()
        E = E[~E.index.duplicated(keep=False)]
        R = outcome(a, E, a_name, H=24); R = R.loc[thin(E.i, 24)].dropna(); out[a_name] = R
    return out

out = legs(); n_reg = len(METHODS) + 4   # + K1,K2,K3,N2
res = {"id": "N2"}; ok = True; pooled = []
for k, R in out.items():
    pre = R[:"2025"]; pooled.append(pre)
    e1, e2 = pre["2018":"2021"].mean(), pre["2022":"2025"].mean(); lo, hi = boot_ci(pre)
    res[k] = dict(n=len(pre), R=round(float(pre.mean()), 4), ci=[round(float(lo), 3), round(float(hi), 3)], eras=[round(float(e1), 3), round(float(e2), 3)])
    ok &= e1 > 0 and e2 > 0
P = pd.concat(pooled); lo, hi = boot_ci(P.sort_index())
x = P.values; rng = np.random.default_rng(1); p = float(np.mean([rng.choice(x, len(x)).mean() <= 0 for _ in range(2000)]))
passed = bool(ok and lo > 0 and p < 0.05 / n_reg)
res.update(pooled_R=round(float(P.mean()), 4), pooled_ci=[round(float(lo), 3), round(float(hi), 3)], p=p, PASS_pre2026=passed)
if passed:
    res["held_2026"] = {k: round(float(R["2026":].mean()), 4) for k, R in out.items()}
else:
    res["held_2026"] = "SEALED"
print(json.dumps(res, indent=1))
open(os.path.join(os.path.dirname(__file__), "results_v2.jsonl"), "a").write(json.dumps(res) + "\n")
