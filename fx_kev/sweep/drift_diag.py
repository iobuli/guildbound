"""Diagnostic only (pre-2026, no result is changed): split near-miss trades by side and subtract each instrument's own drift.
drift_R = side * (mean H-bar log return of the instrument, pre-2026) * entry / risk. A timing edge should survive the subtraction."""
import sys, numpy as np, pandas as pd
sys.path.insert(0, ".")
import eval_proposal as ep
from sweep_lib import load, COST, COST_FRAC

ids = sys.argv[1:]
rows = []
for pid in ids:
    m = ep.module(pid); H, pad = int(m.PREREG["H"]), float(m.PREREG["pad"])
    for name in ep.INSTR:
        df = load(name, include_holdout=False); E = ep.thin(ep.gen(m, df, name), H)
        R = ep.simulate(df, E, name, H, pad); E = E.assign(R=R).dropna(subset=["R"])
        lr = np.log(df.close); muH = (lr.shift(-H) - lr).mean()          # full-sample drift: diagnostic benchmark only
        o = df.open.values; entry = o[E.i.values + 1]; risk = E.side.values * (entry - (E.ext.values - E.side.values * pad * E.atr.values))
        E["drift"] = E.side * muH * entry / risk
        for s, g in E.groupby("side"):
            rows.append(dict(id=pid, instr=name[:6], side="long" if s == 1 else "short", n=len(g), R=g.R.mean(), drift=g.drift.mean(), R_minus_drift=(g.R - g.drift).mean()))
        rows.append(dict(id=pid, instr=name[:6], side="ALL", n=len(E), R=E.R.mean(), drift=E.drift.mean(), R_minus_drift=(E.R - E.drift).mean()))
T = pd.DataFrame(rows)
pd.set_option("display.width", 200)
print(T.round(3).to_string(index=False))
