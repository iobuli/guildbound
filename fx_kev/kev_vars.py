"""Treat Kev's percentages as continuous variables. Uses the cached Kev answers (hier_cache.json) joined to realized R outcomes (exit_M.pkl).
Fit on TRAIN, tune on VAL, report TEST once. Variables: logit(p_D1), logit(p_H1), logit(p_M15), and their mean.
"""
import json

import numpy as np
import pandas as pd

BASE, TRAIL = "baseline 48h, 4ATR stop", "trail 4ATR after +2ATR"


def logit(p): p = np.clip(p, 1e-3, 1 - 1e-3); return np.log(p / (1 - p))


def load():
    cache = json.load(open("hier_cache.json"))
    rows = {}
    for k, v in cache.items():
        _, lvl, t, side = k.split("|")
        rows.setdefault((pd.Timestamp(t), int(side)), {})["p_" + lvl] = v
    P = pd.DataFrame.from_dict(rows, orient="index"); P.index = pd.MultiIndex.from_tuples(P.index, names=["t", "side"])
    P = P.dropna().reset_index().set_index("t")
    M = pd.read_pickle("exit_M.pkl")
    D = P.join(M[[BASE, TRAIL, "split"]], how="inner")
    D = D[D.side == M.loc[D.index, "side"]].sort_index()
    n = len(D)   # the Kev run sampled 200 per chronological split; re-split by time (the 16-year labels call all of 2022+ "test")
    D["split"] = np.where(np.arange(n) < n / 3, "train", np.where(np.arange(n) < 2 * n / 3, "val", "test"))
    for c in ("p_D1", "p_H1", "p_M15"): D["z_" + c[2:]] = logit(D[c])
    D["z_mean"] = D[["z_D1", "z_H1", "z_M15"]].mean(axis=1)
    return D


def sh(x): x = np.asarray(x); return x.mean() / x.std() * np.sqrt(len(x)) if len(x) > 5 and x.std() > 0 else np.nan


def boot_diff(r, w, n=2000, seed=0):
    """Bootstrap CI of (Sharpe of sized returns - Sharpe of flat returns), resampling trades."""
    rng = np.random.default_rng(seed); r, w = np.asarray(r), np.asarray(w); d = []
    for _ in range(n):
        i = rng.integers(0, len(r), len(r)); d.append(sh(r[i] * w[i]) - sh(r[i]))
    return np.nanpercentile(d, [2.5, 97.5])


def main():
    D = load()
    print(f"{len(D)} scored trades:", D.split.value_counts().to_dict(), "\n")
    print("Distribution of Kev percentages (all splits):"); print(D[["p_D1", "p_H1", "p_M15"]].describe().loc[["mean", "std", "min", "25%", "50%", "75%", "max"]].round(3))
    for exit_name in (BASE, TRAIL):
        for book, sub in (("long+short", D), ("long only", D[D.side == 1])):
            print(f"\n################ exit = {exit_name} | {book} ################")
            R = sub[exit_name]
            print("Spearman corr(variable, realized R)  [|rho| > ~0.1 needed for n~200 to be distinguishable from 0]")
            out = {s: {v: sub[sub.split == s][v].rank().corr(sub[sub.split == s][exit_name].rank()) for v in ("z_D1", "z_H1", "z_M15", "z_mean")} for s in ("train", "val", "test")}
            print(pd.DataFrame(out).round(3))
            print("Calibration: realized mean R and win% by Kev score tercile (terciles cut on TRAIN)")
            cut = sub[sub.split == "train"].z_mean.quantile([1 / 3, 2 / 3]).values
            g = pd.cut(sub.z_mean, [-np.inf, *cut, np.inf], labels=["low", "mid", "high"])
            print(sub.assign(tercile=g, win=(R > 0)).groupby(["tercile", "split"], observed=True).agg(R=(exit_name, "mean"), win=("win", "mean"), n=(exit_name, "size")).unstack("split").round(3).to_string())
            # fit R ~ a + b*z on train, size by predicted R
            tr = sub[sub.split == "train"]
            X = np.c_[np.ones(len(tr)), tr[["z_D1", "z_H1", "z_M15"]].values]
            beta = np.linalg.lstsq(X, tr[exit_name].values, rcond=None)[0]
            pred = lambda d: np.c_[np.ones(len(d)), d[["z_D1", "z_H1", "z_M15"]].values] @ beta
            print("fitted coefficients on train [const, D1, H1, M15]:", beta.round(3))
            ref = np.mean(np.clip(pred(tr), 0, None)) or 1.0
            schemes = {"flat size=1": lambda d: np.ones(len(d)),
                       "size = p_mean/avg (raw percent as size)": lambda d: d[["p_D1", "p_H1", "p_M15"]].mean(axis=1).values / tr[["p_D1", "p_H1", "p_M15"]].mean(axis=1).mean(),
                       "size = max(fitted R,0)/avg": lambda d: np.clip(pred(d), 0, None) / ref}
            rows = []
            for s in ("train", "val", "test"):
                d = sub[sub.split == s]
                for name, f in schemes.items():
                    w = f(d); r = d[exit_name].values
                    rows.append(dict(split=s, scheme=name, R_per_unit_risk=(w * r).sum() / w.sum(), sharpe=sh(r * w), avg_size=w.mean()))
            tab = pd.DataFrame(rows).pivot(index="scheme", columns="split", values=["R_per_unit_risk", "sharpe"]).round(3)
            print(tab.to_string())
            te = sub[sub.split == "test"]
            lo, hi = boot_diff(te[exit_name].values, schemes["size = max(fitted R,0)/avg"](te))
            print(f"TEST Sharpe(sized by fitted R) - Sharpe(flat): 95% bootstrap CI [{lo:+.2f}, {hi:+.2f}]  (0 inside => no proven benefit)")


if __name__ == "__main__":
    main()
