"""C1 crypto continuation replication, exactly as registered in sweep/ledger.py wave 12. Pre-2026 data only."""
import glob
import json
import os
import sys
import zipfile

import numpy as np
import pandas as pd

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "sweep"))
import eval_proposal as ep  # noqa: E402
import sweep_lib  # noqa: E402

COINS = ["BTCUSDT", "ETHUSDT", "BNBUSDT", "XRPUSDT", "ADAUSDT", "LTCUSDT", "LINKUSDT", "DOGEUSDT"]
METHODS = ["FVG1", "EXH1", "TSMOM1", "R2C"]          # FVG1 primary
CRYPTO_CAL = {"TSMOM1", "R2C"}


def load_coin(sym):
    parts = []
    for z in sorted(glob.glob(os.path.join(HERE, "raw", f"{sym}-1h-*.zip"))):
        with zipfile.ZipFile(z) as f:
            d = pd.read_csv(f.open(f.namelist()[0]), header=None, usecols=[0, 1, 2, 3, 4, 5])
        parts.append(d)
    d = pd.concat(parts); d.columns = ["t", "open", "high", "low", "close", "volume"]
    d["t"] = pd.to_datetime(np.where(d.t > 1e14, d.t // 1000, d.t), unit="ms")      # 2025 files use microseconds
    d = d.drop_duplicates("t").set_index("t").sort_index()
    return d[d.index < "2026-01-01"].astype(float)


def week_cluster_ci(R, n=2000, seed=0):
    wk = R.index.to_period("W"); g = pd.DataFrame({"R": R.values, "wk": wk}).groupby("wk").R
    s, c = g.sum().values, g.count().values; rng = np.random.default_rng(seed); means = []
    for _ in range(n):
        k = rng.integers(0, len(s), len(s)); means.append(s[k].sum() / c[k].sum())
    means = np.array(means)
    return float(np.percentile(means, 2.5)), float(np.percentile(means, 97.5)), float((means <= 0).mean())


def main():
    reg = {r["id"]: r["sha256"] for r in ep.jl(ep.REG)}
    for m in METHODS: assert reg[m] == ep.sha(m), f"{m} changed since registration"
    data = {}
    for sym in COINS:
        df = load_coin(sym); name = f"{sym}_H1"; data[sym] = df
        sweep_lib.COST[name] = None; sweep_lib.COST_FRAC[name] = 0.001
        med = df.close[: df.index[0] + pd.Timedelta(days=365)].median()
        sweep_lib.RN_STEP[name] = 10 ** (round(np.log10(med)) - 1)
        print(f"{sym}: {len(df)} bars {df.index[0].date()}..{df.index[-1].date()} RN step {sweep_lib.RN_STEP[name]}")
    out = {}
    for mid in METHODS:
        m = ep.module(mid); H, pad = int(m.PREREG["H"]), float(m.PREREG["pad"]); per, allR, allD = {}, [], []
        for sym, df in data.items():
            name = f"{sym}_H1"; gname = "SOLUSDT_H1" if mid in CRYPTO_CAL else name
            E = ep.thin(ep.gen(m, df, gname), H); R = ep.simulate(df, E, name, H, pad)
            E = E.assign(R=R).dropna(subset=["R"])
            lr = np.log(df.close); muH = (lr.shift(-H) - lr).mean()
            entry = df.open.values[E.i.values + 1]; risk = E.side.values * (entry - (E.ext.values - E.side.values * pad * E.atr.values))
            drift = E.side * muH * entry / risk
            per[sym] = dict(n=len(E), mean=round(float(E.R.mean()), 4), long_share=round(float((E.side == 1).mean()), 3),
                            drift_adj=round(float((E.R - drift).mean()), 4))
            allR.append(E.R); allD.append(E.R - drift)
        P, D = pd.concat(allR).sort_index(), pd.concat(allD)
        lo, hi, p = week_cluster_ci(P)
        pos = sum(v["mean"] > 0 for v in per.values())
        alpha = 0.05 if mid == "FVG1" else 0.05 / 4
        passed = bool(P.mean() > 0 and lo > 0 and pos >= 6 and D.mean() > 0 and (mid == "FVG1" or p < alpha))
        out[mid] = dict(per_coin=per, pooled_n=len(P), pooled_mean=round(float(P.mean()), 4), week_cluster_ci=[round(lo, 4), round(hi, 4)],
                        p_one_sided=round(p, 4), coins_positive=pos, pooled_drift_adj=round(float(D.mean()), 4), PASS=passed)
        print(f"\n{mid}: pooled n={len(P)} mean R={P.mean():+.4f} week-cluster CI [{lo:+.4f}, {hi:+.4f}] p={p:.4f} coins>0 {pos}/8 drift-adj {D.mean():+.4f} -> PASS={passed}")
        for sym, v in per.items(): print(f"   {sym:9s} n={v['n']:5d} R={v['mean']:+.4f} drift-adj={v['drift_adj']:+.4f} long={v['long_share']}")
    with open(os.path.join(HERE, "c1_result.json"), "w") as f: json.dump(out, f, indent=1)


if __name__ == "__main__":
    main()
