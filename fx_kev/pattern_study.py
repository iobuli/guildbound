"""Pattern study: what conditions precede profitable Ichimoku TK-cross entries on USDJPY H1 (2010-2026)?
Features use only bars closed at decision time t (D1 + H1). Outcomes: entry at next H1 open, 4xATR stop, 48h max hold.
Patterns are discovered on TRAIN, must hold on VAL, and are confirmed once on TEST.
"""
import numpy as np
import pandas as pd

from fx_hier import FEE, U, TF, atr, feature_frame, last_closed
from ichi_opt import load

pd.set_option("display.width", 200, "display.max_columns", 30)
HOLD, STOP = 48, 4.0


def build():
    h_raw, d_raw = load(U + TF["H1"][0]), load(U + TF["D1"][0])
    fh, fd = feature_frame(h_raw, TF["H1"][1]), feature_frame(d_raw, TF["D1"][1])
    fh["atr_pct"] = fh.atr.rolling(500).rank(pct=True)
    cu = (fh.tenkan > fh.kijun) & (fh.tenkan.shift() <= fh.kijun.shift())
    cd = (fh.tenkan < fh.kijun) & (fh.tenkan.shift() >= fh.kijun.shift())
    o, hi, lo, cl = h_raw.open.values, h_raw.high.values, h_raw.low.values, h_raw.close.values
    idx = h_raw.index
    ts = fh.index[(cu | cd) & (fh.n > 300)]
    # D1 lookup in one vectorised pass (last closed daily bar at each t)
    di = fd.index.searchsorted(ts, side="right") - 1
    D = fd.iloc[di].reset_index(drop=True)
    rows = []
    for k, t in enumerate(ts):
        if di[k] < 130: continue
        side = 1 if cu.loc[t] else -1
        j = idx.searchsorted(t, side="left")
        if j + HOLD + 1 >= len(idx) or (idx[j] - t) > pd.Timedelta("2h"): continue
        h = fh.loc[t]; a = h.atr; d = D.iloc[k]; c0 = o[j]
        stop = c0 - side * STOP * a
        path_lo, path_hi, path_cl = lo[j:j + HOLD], hi[j:j + HOLD], cl[j:j + HOLD]
        hit = np.nonzero(path_lo <= stop if side == 1 else path_hi >= stop)[0]
        end = hit[0] if len(hit) else HOLD - 1
        exit_p = stop if len(hit) else path_cl[-1]
        fav = (path_hi[:end + 1].max() - c0) if side == 1 else (c0 - path_lo[:end + 1].min())
        adv = (c0 - path_lo[:end + 1].min()) if side == 1 else (path_hi[:end + 1].max() - c0)
        tpk = int(np.argmax((path_hi[:end + 1] - c0) if side == 1 else (c0 - path_lo[:end + 1])))
        cl_top, cl_bot = h.top, h.bot
        rows.append(dict(
            t=t, side=side, ret=side * (exit_p / c0 - 1) - 2 * FEE, R=(side * (exit_p - c0)) / a, mfe=fav / a, mae=adv / a, t_peak=tpk, stopped=len(hit) > 0,
            hour=t.hour, dow=t.dayofweek,
            # H1 context, signed so +ve = in the trade's favour
            h_cloud=side * np.sign(h.close - (cl_top if side == 1 else cl_bot)) if not np.isnan(cl_top) else np.nan,
            h_dist_cloud=side * (h.close - (cl_top + cl_bot) / 2) / a, h_thick=(cl_top - cl_bot) / a,
            h_kslope=side * h.kslope / (a / h.close * 100), h_ret24=side * h.ret24 / (a / h.close * 100),
            h_rsi=h.rsi if side == 1 else 100 - h.rsi, h_atrpct=h.atr_pct, h_chikou=int(h.chikou_ok_up if side == 1 else h.chikou_ok_dn),
            h_future=int((h.fa > h.fb) if side == 1 else (h.fa < h.fb)), h_tk_dist=side * (h.tenkan - h.kijun) / a,
            # D1 context
            d_cloud=side * np.sign(d.close - (d.top if side == 1 else d.bot)), d_kslope=side * d.kslope / (d.atr / d.close * 100),
            d_ret20=side * d.ret20 / (d.atr / d.close * 100), d_rsi=d.rsi if side == 1 else 100 - d.rsi,
            d_tk=int((d.tenkan > d.kijun) if side == 1 else (d.tenkan < d.kijun)), d_future=int((d.fa > d.fb) if side == 1 else (d.fa < d.fb)),
            d_chikou=int(d.chikou_ok_up if side == 1 else d.chikou_ok_dn)))
    X = pd.DataFrame(rows).set_index("t").dropna()
    n = len(X); X["split"] = np.where(np.arange(n) < n * .5, "train", np.where(np.arange(n) < n * .75, "val", "test"))
    return X


def table(X, col, bins=None, q=4):
    g = pd.qcut(X[col], q, duplicates="drop") if bins is None else X[col]
    out = X.groupby([g, "split"], observed=True).R.agg(["mean", "count"]).unstack("split")
    return out["mean"][["train", "val", "test"]].round(3).join(out["count"].sum(axis=1).rename("n"))


def main():
    X = build()
    print(f"{len(X)} crosses  {X.index[0].date()}..{X.index[-1].date()}  splits:", X.split.value_counts().to_dict())
    print("\n=== BASELINE (mean R per trade, R = profit in units of H1 ATR; cost included in ret not R) ===")
    print(X.groupby("split").agg(n=("R", "size"), meanR=("R", "mean"), win=("ret", lambda x: (x > 0).mean()), ret_pct=("ret", lambda x: x.mean() * 100), stopped=("stopped", "mean"), mfe=("mfe", "mean"), mae=("mae", "mean"), t_peak=("t_peak", "median")).round(3))
    print("\n=== TRADE ANATOMY: winners vs losers (all splits) ===")
    print(X.groupby(X.ret > 0)[["mfe", "mae", "t_peak"]].agg(["mean", "median"]).round(2))
    print("\n=== SIDE ===");  print(X.groupby(["side", "split"]).R.mean().unstack().round(3))
    print("\n=== SESSION (UTC hour block) ===")
    X["sess"] = pd.cut(X.hour, [-1, 6, 12, 16, 20, 23], labels=["Asia 0-6", "London 7-12", "NY-open 13-16", "NY 17-20", "late 21-23"])
    print(X.groupby(["sess", "split"], observed=True).R.mean().unstack().round(3).join(X.groupby("sess", observed=True).size().rename("n")))
    print("\n=== WEEKDAY ==="); print(X.groupby(["dow", "split"]).R.mean().unstack().round(3).join(X.groupby("dow").size().rename("n")))
    print("\n=== CONTEXT FEATURES: mean R by quartile, per split (a real pattern has the same shape in all three) ===")
    for col in ["h_dist_cloud", "h_thick", "h_kslope", "h_ret24", "h_rsi", "h_atrpct", "h_tk_dist", "d_kslope", "d_ret20", "d_rsi"]:
        print(f"\n-- {col}"); print(table(X, col))
    print("\n=== BINARY CONTEXT FLAGS ===")
    for col in ["h_cloud", "h_chikou", "h_future", "d_cloud", "d_tk", "d_future", "d_chikou"]:
        print(f"\n-- {col}"); print(table(X, col, bins=False))
    # Combination: daily regime aligned AND hourly structure aligned
    X["aligned"] = ((X.d_cloud == 1) & (X.h_cloud == 1)).astype(int)
    X["against"] = ((X.d_cloud == -1)).astype(int)
    print("\n-- D1 and H1 both clearly on the trade's side of the cloud"); print(table(X, "aligned", bins=False))
    print("\n-- D1 cloud AGAINST the trade"); print(table(X, "against", bins=False))
    X.to_pickle("/home/user/fx_kev/pattern_X.pkl")


if __name__ == "__main__":
    main()
