"""Evaluate K1/K2 exactly as registered in ledger.py (wave 3). Prints pre-2026 only unless every Kev pass condition holds."""
import json
import os
import sys

import numpy as np
import pandas as pd

sys.path.insert(0, os.path.dirname(__file__))
from sweep_lib import COST, COST_FRAC, boot_ci

D = pd.read_pickle(os.path.join(os.path.dirname(__file__), "kev_sweep_scored.pkl"))


def rank_corr(a, b):
    return float(pd.Series(a).rank().corr(pd.Series(b).rank()))


def gate(x):
    """Causal: compare each event's P(reversal) to the median of the previous 100 scored events of the same instrument."""
    med = x.p.shift(1).rolling(100, min_periods=30).median()
    return x.assign(med=med).dropna(subset=["med"])


rows, pooled_k1 = [], []
for name, x in D.items():
    x = x.sort_index(); pre = gate(x[:"2025"])
    k1 = pre[pre.p > pre.med].R
    # K2 follows the break: approx R of the opposite trade = -(fade R) - 2*cost_in_R; cost_in_R not stored per event, so recompute:
    k2 = -pre[pre.p <= pre.med].R
    rows.append(dict(instr=name, n=len(pre), spearman=round(rank_corr(pre.p, pre.R), 3), S1_R=round(pre.R.mean(), 3),
                     K1_n=len(k1), K1_R=round(k1.mean(), 3), K2_n=len(pre) - len(k1), K2_R_approx=round(k2.mean(), 3),
                     p_min=round(x.p.min(), 3), p_med=round(x.p.median(), 3), p_max=round(x.p.max(), 3)))
    pooled_k1.append(k1)
T = pd.DataFrame(rows).set_index("instr")
print(T.to_string())
pk = pd.concat(pooled_k1).sort_index(); lo, hi = boot_ci(pk)
print(f"pooled K1: n={len(pk)} mean R={pk.mean():+.3f} block-bootstrap 95% CI [{lo:+.3f}, {hi:+.3f}]")
c1 = (T.spearman > 0).all(); c2 = (T.K1_R > T.S1_R).all(); c3 = lo > 0
print(f"Kev pass conditions: spearman>0 on all 4 = {c1}; K1 beats S1 on all 4 = {c2}; pooled CI lower > 0 = {c3}")
print("KEV PASSES -> score 2026 with: python3 kev_sweep.py --unseal" if (c1 and c2 and c3) else "KEV FAILS the pre-registered bar; 2026 stays sealed.")
print("note: K2_R_approx is the sign-flipped fade R; it ignores that the follow trade pays its own cost and has a different stop, so it is optimistic.")
