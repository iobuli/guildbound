"""Ichimoku rule strategy on SOLUSDT 1h: grid search with a train / validation / test split.
Cost is charged per side as a fraction of price (default 0.05% = 0.1% round trip).
"""
import itertools
import sys

import numpy as np
import pandas as pd

CSV = "/root/.claude/uploads/e6c1b55c-a96f-536d-adc8-7a7224cce5ae/60a79ead-SOLUSDT60.csv"
FEE = 0.0005


def load(path=CSV):
    df = pd.read_csv(path, sep="\t", header=None, names=["time", "open", "high", "low", "close", "volume"])
    df["time"] = pd.to_datetime(df["time"])
    return df.set_index("time").sort_index()


def mid(h, l, n):
    return (h.rolling(n).max() + l.rolling(n).min()) / 2


def ichimoku(df, t=9, k=26, b=52):
    h, l, c = df.high, df.low, df.close
    tenkan, kijun = mid(h, l, t), mid(h, l, k)
    span_a = ((tenkan + kijun) / 2).shift(k)           # cloud as plotted at time now (computed k bars ago)
    span_b = mid(h, l, b).shift(k)
    return pd.DataFrame({"tenkan": tenkan, "kijun": kijun, "top": np.maximum(span_a, span_b),
                         "bot": np.minimum(span_a, span_b), "chikou_ok_up": c > c.shift(k), "chikou_ok_dn": c < c.shift(k),
                         "fa": (tenkan + kijun) / 2, "fb": mid(h, l, b)})  # unshifted = "future" cloud


def signals(df, ich, use_cloud, use_chikou, use_future):
    c = df.close
    cross_up = (ich.tenkan > ich.kijun) & (ich.tenkan.shift() <= ich.kijun.shift())
    cross_dn = (ich.tenkan < ich.kijun) & (ich.tenkan.shift() >= ich.kijun.shift())
    long_ok = pd.Series(True, index=df.index)
    short_ok = long_ok.copy()
    if use_cloud:
        long_ok &= c > ich.top; short_ok &= c < ich.bot
    if use_chikou:
        long_ok &= ich.chikou_ok_up; short_ok &= ich.chikou_ok_dn
    if use_future:
        long_ok &= ich.fa > ich.fb; short_ok &= ich.fa < ich.fb
    return cross_up & long_ok, cross_dn & short_ok


def run(df, t, k, b, use_cloud, use_chikou, use_future, long_only, atr_stop, max_hold, fee=FEE):
    ich = ichimoku(df, t, k, b)
    le, se = signals(df, ich, use_cloud, use_chikou, use_future)
    if long_only: se = se & False
    c, h, l = df.close.values, df.high.values, df.low.values
    tr = pd.concat([df.high - df.low, (df.high - df.close.shift()).abs(), (df.low - df.close.shift()).abs()], axis=1).max(axis=1)
    atr = tr.rolling(14).mean().values
    ent_l, ent_s = le.values, se.values
    rets, times, sides = [], [], []
    i, n = 0, len(df)
    while i < n - 1:
        side = 1 if ent_l[i] else -1 if ent_s[i] else 0
        if not side or np.isnan(atr[i]):
            i += 1; continue
        entry = c[i]; stop = entry - side * atr_stop * atr[i]; j = i + 1; exit_p = c[min(i + max_hold, n - 1)]
        end = min(i + max_hold, n - 1)
        while j <= end:
            if (side == 1 and l[j] <= stop) or (side == -1 and h[j] >= stop):
                exit_p = stop; break
            opp = (ent_s[j] if side == 1 else ent_l[j])
            if opp:
                exit_p = c[j]; break
            j += 1
        else:
            j = end
        rets.append(side * (exit_p / entry - 1) - 2 * fee); times.append(df.index[i]); sides.append(side)
        i = j + 1
    return pd.DataFrame({"ret": rets, "side": sides}, index=pd.DatetimeIndex(times))


def stats(t):
    if len(t) < 10: return dict(n=len(t), total=-9, avg=-9, win=0, sharpe=-9, dd=0)
    eq = (1 + t.ret).cumprod()
    return dict(n=len(t), total=eq.iloc[-1] - 1, avg=t.ret.mean() * 100, win=(t.ret > 0).mean(),
                sharpe=t.ret.mean() / t.ret.std() * np.sqrt(len(t)), dd=(1 - eq / eq.cummax()).max())


def fmt(s): return f"n={s['n']:4d} total={s['total']:+8.1%} avg/trade={s['avg']:+.3f}% win={s['win']:.1%} sharpe={s['sharpe']:+.2f} maxDD={s['dd']:.1%}"


def main():
    df = load()
    n = len(df); a, b = int(n * 0.5), int(n * 0.75)
    tr, va, te = df.iloc[:a], df.iloc[a:b], df.iloc[b:]
    print(f"bars={n}  train {tr.index[0].date()}..{tr.index[-1].date()}  val ..{va.index[-1].date()}  test ..{te.index[-1].date()}")
    bh = lambda d: d.close.iloc[-1] / d.close.iloc[0] - 1
    print(f"buy&hold: train {bh(tr):+.0%} val {bh(va):+.0%} test {bh(te):+.0%}")
    grid = list(itertools.product([5, 9, 12, 20], [20, 26, 40, 60], [52, 100], [False, True], [False, True], [False, True], [False, True], [2, 4], [48, 200]))
    res = []
    for g in grid:
        t, k, bb, cl, ch, fu, lo, st, mh = g
        if t >= k: continue
        res.append((g, stats(run(tr, *g)), stats(run(va, *g))))
    # rank by train sharpe, require positive on val too -> choose by val sharpe among top-train
    res.sort(key=lambda r: r[1]["sharpe"], reverse=True)
    top = [r for r in res[:40] if r[1]["total"] > 0]
    top.sort(key=lambda r: r[2]["sharpe"], reverse=True)
    print("\nconfig = (tenkan,kijun,spanB,cloud,chikou,future,long_only,atr_stop,max_hold)")
    for g, s1, s2 in top[:6]:
        print(g, "\n   train", fmt(s1), "\n   val  ", fmt(s2))
    best = top[0][0] if top else res[0][0]
    print("\nCHOSEN", best)
    print("   TEST", fmt(stats(run(te, *best))))
    print("   (default 9/26/52, no filters, long+short) TEST", fmt(stats(run(te, 9, 26, 52, False, False, False, False, 4, 200))))
    print("   (default 9/26/52, all filters)            TEST", fmt(stats(run(te, 9, 26, 52, True, True, True, False, 4, 200))))


if __name__ == "__main__":
    main()
