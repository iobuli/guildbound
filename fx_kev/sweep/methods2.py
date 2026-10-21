"""Event generators for wave-2 methods. Same output contract as sweep_lib.events: index t (signal bar), i (bar index; entry at i+1 open),
side, ext (stop anchor; stop = ext - side*0.25*ATR), atr. Causal: every input is known at the close of bar i."""
import numpy as np
import pandas as pd

from sweep_lib import atr, events


def pivots(df):
    h, l = df.high.values, df.low.values; n = len(h)
    ph = np.zeros(n, bool); pl = np.zeros(n, bool)
    for j in range(2, n - 2):
        ph[j] = h[j] > max(h[j - 2], h[j - 1], h[j + 1], h[j + 2]); pl[j] = l[j] < min(l[j - 2], l[j - 1], l[j + 1], l[j + 2])
    return ph, pl          # pivot at j is known only after bar j+2 closes


def eqhl(df, name):
    a = atr(df).shift(1).values; h, l, c, o = (df[k].values for k in ("high", "low", "close", "open"))
    ph, pl = pivots(df); rows = []
    hist_h, hist_l = [], []          # (pivot index, value) confirmed so far
    for i in range(5, len(df)):
        j = i - 3                    # pivot j confirmed at close of j+2 = i-1, usable at bar i
        if ph[j]: hist_h.append((j, h[j]))
        if pl[j]: hist_l.append((j, l[j]))
        if np.isnan(a[i]): continue
        if len(hist_h) >= 2:
            (j1, v1), (j2, v2) = hist_h[-2], hist_h[-1]
            if i - j1 <= 48 and abs(v1 - v2) < 0.15 * a[i]:
                lv = max(v1, v2)
                if h[i] > lv and c[i] < lv and o[i] < lv: rows.append((df.index[i], i, -1, h[i], a[i]))
        if len(hist_l) >= 2:
            (j1, v1), (j2, v2) = hist_l[-2], hist_l[-1]
            if i - j1 <= 48 and abs(v1 - v2) < 0.15 * a[i]:
                lv = min(v1, v2)
                if l[i] < lv and c[i] > lv and o[i] > lv: rows.append((df.index[i], i, 1, l[i], a[i]))
    E = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")
    return E[~E.index.duplicated(keep=False)]


def mss(df, name):
    E = events(df, name); h, l, c = df.high.values, df.low.values, df.close.values
    ph, pl = pivots(df); n = len(df)
    last_ph = np.full(n, np.nan); last_pl = np.full(n, np.nan); vh = vl = np.nan
    for i in range(n):
        j = i - 3
        if j >= 0 and ph[j]: vh = h[j]
        if j >= 0 and pl[j]: vl = l[j]
        last_ph[i], last_pl[i] = vh, vl
    rows = []
    for t, e in E.iterrows():
        i, s = int(e.i), int(e.side); ref = last_ph[i] if s == 1 else last_pl[i]
        if np.isnan(ref): continue
        for k in range(i + 1, min(i + 7, n)):
            if (s == 1 and c[k] > ref) or (s == -1 and c[k] < ref):
                rows.append((df.index[k], k, s, e.ext, e.atr)); break
    out = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")
    return out[~out.index.duplicated(keep="first")]


def failed(df, name):
    E = events(df, name); h, l, c = df.high.values, df.low.values, df.close.values; n = len(df); rows = []
    for t, e in E.iterrows():
        i, s = int(e.i), int(e.side)
        for k in range(i + 1, min(i + 4, n)):
            if (s == 1 and c[k] < e.ext) or (s == -1 and c[k] > e.ext):     # closed beyond the sweep wick: sweep failed
                opp = h[i] if s == 1 else l[i]                               # follow direction = -s; stop beyond the sweep bar's other end
                rows.append((df.index[k], k, -s, opp, e.atr)); break
    out = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")
    return out[~out.index.duplicated(keep="first")]


KINDS = {"eqhl": eqhl, "mss": mss, "failed": failed}


def target(df, name):
    """N1: events carry a take-profit at the nearest opposite liquidity level (>= 2 ATR away). Stored in column 'tp'."""
    from sweep_lib import levels
    E = events(df, name); L = levels(df, name); c = df.close.values; rows = []
    up_cols, dn_cols = ["PDH", "PWH", "ASH", "SWH"], ["PDL", "PWL", "ASL", "SWL"]
    for t, e in E.iterrows():
        i, s = int(e.i), int(e.side); px = c[i]
        cand = L.iloc[i][up_cols if s == 1 else dn_cols].dropna().values
        cand = cand[cand > px] if s == 1 else cand[cand < px]
        if not len(cand): continue
        tp = cand.min() if s == 1 else cand.max()
        if abs(tp - px) < 2 * e.atr: continue
        rows.append((t, i, s, e.ext, e.atr, tp))
    return pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr", "tp"]).set_index("t")


KINDS["target"] = target


def londonopen(df, name):
    E = events(df, name)
    E = E[E["level"].str.contains("AS") & np.isin(E.index.hour, [7, 8])]
    return E[["i", "side", "ext", "atr"]]


KINDS["londonopen"] = londonopen


def resweep(df, name):
    """N4: second sweep beyond an earlier S1 sweep's wick within 24 bars, closing back inside; fade. Causal: uses S1 events at bar i < k
    (each known at close of i) and bar k's own OHLC."""
    E = events(df, name); h, l, c = df.high.values, df.low.values, df.close.values; a = atr(df).shift(1).values; n = len(df); rows = []
    for t, e in E.iterrows():
        i, s, x = int(e.i), int(e.side), float(e.ext)
        for k in range(i + 1, min(i + 25, n)):
            if s == -1 and h[k] > x:
                if c[k] < x and not np.isnan(a[k]): rows.append((df.index[k], k, -1, h[k], a[k]))
                break
            if s == 1 and l[k] < x:
                if c[k] > x and not np.isnan(a[k]): rows.append((df.index[k], k, 1, l[k], a[k]))
                break
    out = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")
    return out[~out.index.duplicated(keep=False)].sort_index()


KINDS["resweep"] = resweep


def dcluster(df, name):
    """N5: multi-day daily-high/low cluster sweep. Daily highs/lows of the previous 10 COMPLETED days only; ATR known before bar."""
    a = atr(df).shift(1).values; h, l, c, o = (df[k].values for k in ("high", "low", "close", "open"))
    day = df.index.normalize(); d = df.groupby(day).agg(high=("high", "max"), low=("low", "min"))
    dpos = d.index.get_indexer(day); DH, DL = d.high.values, d.low.values; rows = []
    cache = {}
    for i in range(len(df)):
        p = dpos[i]
        if p < 10 or np.isnan(a[i]): continue
        tol = 0.5 * a[i] * np.sqrt(24)
        hs = np.sort(DH[p - 10:p]); ls = np.sort(DL[p - 10:p])
        zh = zl = np.nan
        for k in range(len(hs) - 3, -1, -1):          # highest run of 3 consecutive sorted highs within tol
            if hs[k + 2] - hs[k] <= tol:
                m = k + 2
                while m + 1 < len(hs) and hs[m + 1] - hs[k] <= tol: m += 1
                zh = hs[m]; break
        for k in range(0, len(ls) - 2):               # lowest run of 3 lows within tol
            if ls[k + 2] - ls[k] <= tol:
                m = k
                while m - 1 >= 0 and ls[k + 2] - ls[m - 1] <= tol: m -= 1
                zl = ls[m]; break
        up = zh == zh and h[i] > zh and c[i] < zh and o[i] < zh
        dn = zl == zl and l[i] < zl and c[i] > zl and o[i] > zl
        if up and dn: continue
        if up: rows.append((df.index[i], i, -1, h[i], a[i]))
        if dn: rows.append((df.index[i], i, 1, l[i], a[i]))
    return pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")


KINDS["dcluster"] = dcluster


def volrelease(df, name):
    """N6: follow an S1 sweep occurring 1-6 bars after a compression release. Ratio at bar j uses closes through j (known at j's close; i > j)."""
    E = events(df, name); h, l = df.high.values, df.low.values
    r = np.log(df.close).diff(); ratio = (r.rolling(12).std() / r.rolling(120).std()).values
    prevmin = pd.Series(ratio).shift(1).rolling(24).min().values
    rel = np.zeros(len(df), bool)
    rel[1:] = (ratio[1:] > 0.6) & (ratio[:-1] <= 0.6) & (prevmin[1:] < 0.4)
    relidx = np.flatnonzero(rel); rows = []
    for t, e in E.iterrows():
        i, s = int(e.i), int(e.side)
        k = np.searchsorted(relidx, i) - 1           # last release strictly before i
        if k < 0 or not (1 <= i - relidx[k] <= 6): continue
        opp = h[i] if s == 1 else l[i]
        rows.append((t, i, -s, opp, e.atr))
    return pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")


KINDS["volrelease"] = volrelease


def weekopen(df, name):
    E = events(df, name)
    E = E[E["level"].str.contains("PW") & (E.index.dayofweek == 0) & (E.index.hour < 6)]
    return E[["i", "side", "ext", "atr"]]


KINDS["weekopen"] = weekopen
