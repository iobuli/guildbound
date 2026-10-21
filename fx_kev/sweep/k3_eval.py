"""K3 confirmatory test, exactly as registered in ledger.py wave 5. Uses only the fresh sample (kev_sweep_fresh.pkl)."""
import os
import sys

import numpy as np
import pandas as pd

sys.path.insert(0, os.path.dirname(__file__))
F = pd.read_pickle(os.path.join(os.path.dirname(__file__), "kev_sweep_fresh.pkl"))


def rho(a, b):
    return float(pd.Series(a).rank().corr(pd.Series(b).rank()))


per, P, Rr, grp = {}, [], [], []
for n, x in F.items():
    per[n] = rho(x.p.values, x.R.values)
    # rank within instrument so pooling does not mix instrument-level differences
    P += list(pd.Series(x.p.values).rank(pct=True)); Rr += list(pd.Series(x.R.values).rank(pct=True)); grp += [n] * len(x)
pooled = rho(P, Rr)
rng = np.random.default_rng(0); P_, R_, g_ = np.array(P), np.array(Rr), np.array(grp); null = []
for _ in range(5000):
    Rp = R_.copy()
    for n in F:                       # permute within instrument
        m = g_ == n; Rp[m] = rng.permutation(Rp[m])
    null.append(rho(P_, Rp))
p_one = float(np.mean(np.array(null) <= pooled))
print("fresh-sample Spearman(P(reversal), fade R):", {k: round(v, 3) for k, v in per.items()})
print(f"pooled (within-instrument ranks) = {pooled:+.3f}, one-sided permutation p = {p_one:.4f}, n = {len(P)}")
c1 = sum(v < 0 for v in per.values()) >= 3; c2 = pooled < 0 and p_one < 0.05
print(f"K3 conditions: negative on >=3 of 4 = {c1}; pooled negative with p<0.05 = {c2}")
print("K3 CONFIRMED -> next: evaluate follow-the-break trade with real costs, then unseal 2026" if (c1 and c2) else "K3 NOT CONFIRMED: the negative correlation was noise. Kev has no usable signal on sweeps.")
