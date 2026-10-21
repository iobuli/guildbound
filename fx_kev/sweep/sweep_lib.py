"""Levels + liquidity-sweep event engine. Causal by construction: every level at bar i uses only bars closed before bar i opens.

Levels (all known before bar i):
  PDH/PDL  previous completed day's high/low
  PWH/PWL  previous completed week's high/low
  ASH/ASL  today's Asian range (hours 0-6) once those bars have closed (only for bars at hour >= 7)
  SWH/SWL  most recent 5-bar fractal swing high/low, confirmed 2 bars after the pivot (so known at pivot+3)
  RN       round number (USDJPY: whole yen; SOL: multiples of 5)
Sweep of a level by bar i:
  buy-side sweep  (bearish): high_i > level >= max(open_i, close_i)? no: high_i > level and close_i < level  -> fade = SHORT
  sell-side sweep (bullish): low_i  < level and close_i > level                                        -> fade = LONG
Trade: enter at open of bar i+1. Stop = sweep extreme -/+ 0.25*ATR. 1R = |entry - stop|. Exit at close of bar i+H or at stop
(stop checked first on each bar, conservative). Net R includes costs.
"""
import numpy as np
import pandas as pd

DATA = "/home/user/fx_kev/blind/data/"
HOLD = "/tmp/claude-0/-home-user/e6c1b55c-a96f-536d-adc8-7a7224cce5ae/scratchpad/holdout/"
COST = {"USDJPY_H1": 0.010, "SOLUSDT_H1": None, "XAUUSD_H1": None, "XAGUSD_H1": None}   # USDJPY: 1 pip in price; others: fraction of price below
COST_FRAC = {"SOLUSDT_H1": 0.001, "XAUUSD_H1": 0.00025, "XAGUSD_H1": 0.0005}   # round trip; fixed before metals were evaluated
RN_STEP = {"USDJPY_H1": 1.0, "SOLUSDT_H1": 5.0, "XAUUSD_H1": 25.0, "XAGUSD_H1": 0.5}
LEVELS = ["PDH", "PDL", "PWH", "PWL", "ASH", "ASL", "SWH", "SWL", "RNU", "RND"]
UPPER = {"PDH", "PWH", "ASH", "SWH", "RNU"}


def load(name, include_holdout):
    a = pd.read_csv(DATA + name + ".csv", parse_dates=["time"])
    if include_holdout:
        a = pd.concat([a, pd.read_csv(HOLD + name + ".csv", parse_dates=["time"])])
    return a.drop_duplicates("time").set_index("time").sort_index()


def levels(df, name):
    o, h, l, c = df.open, df.high, df.low, df.close
    day = df.index.normalize()
    d = df.groupby(day).agg(high=("high", "max"), low=("low", "min"))
    L = pd.DataFrame(index=df.index)
    L["PDH"] = d.high.shift(1).reindex(day).values
    L["PDL"] = d.low.shift(1).reindex(day).values
    wk = df.index.to_period("W").start_time
    w = df.groupby(wk).agg(high=("high", "max"), low=("low", "min"))
    L["PWH"] = w.high.shift(1).reindex(wk).values
    L["PWL"] = w.low.shift(1).reindex(wk).values
    asia = df.index.hour < 7
    ah = h.where(asia).groupby(day).transform("max"); al = l.where(asia).groupby(day).transform("min")
    L["ASH"] = ah.where(df.index.hour >= 7); L["ASL"] = al.where(df.index.hour >= 7)
    piv_h = (h.shift(2) > h.shift(1)) & (h.shift(2) > h.shift(3)) & (h.shift(2) > h) & (h.shift(2) > h.shift(4))
    piv_l = (l.shift(2) < l.shift(1)) & (l.shift(2) < l.shift(3)) & (l.shift(2) < l) & (l.shift(2) < l.shift(4))
    # pivot at j is confirmed after bar j+2 closes -> usable from bar j+3: shift once more
    L["SWH"] = h.shift(2).where(piv_h).ffill().shift(1)
    L["SWL"] = l.shift(2).where(piv_l).ffill().shift(1)
    step = RN_STEP[name]
    L["RNU"] = np.ceil(o / step) * step      # nearest round number above this bar's open (open is known at bar start)
    L["RND"] = np.floor(o / step) * step
    return L


def atr(df, n=14):
    tr = pd.concat([df.high - df.low, (df.high - df.close.shift()).abs(), (df.low - df.close.shift()).abs()], axis=1).max(axis=1)
    return tr.rolling(n).mean()


def events(df, name):
    L = levels(df, name); a = atr(df).shift(1)          # ATR known before the bar
    h, l, c, o = df.high, df.low, df.close, df.open
    r = np.log(c).diff()
    comp = (r.rolling(12).std() / r.rolling(120).std()).shift(1) < 0.4   # compression known before bar
    rows = []
    for lv in LEVELS:
        x = L[lv]
        if lv in UPPER:
            m = (h > x) & (c < x) & (o < x)                               # wicked above, closed back below
            side = -1
        else:
            m = (l < x) & (c > x) & (o > x)
            side = 1
        for t in df.index[m.fillna(False).values]:
            rows.append((t, lv, side))
    E = pd.DataFrame(rows, columns=["t", "level", "side"])
    if E.empty: return E
    # merge multiple levels swept by the same bar in the same direction
    E = E.groupby(["t", "side"])["level"].apply(lambda s: ",".join(sorted(s))).reset_index()
    E["n_levels"] = E["level"].str.count(",") + 1
    E = E.set_index("t")
    E = E[~E.index.duplicated(keep=False)]                               # drop bars that swept both sides (ambiguous)
    E["atr"] = a.reindex(E.index); E["comp"] = comp.reindex(E.index)
    i = df.index.get_indexer(E.index)
    E["ext"] = np.where(E.side == 1, l.values[i], h.values[i])          # sweep extreme
    E["body_frac"] = (np.abs(c.values[i] - o.values[i]) / (h.values[i] - l.values[i]).clip(1e-12))
    E["close_loc"] = np.where(E.side == 1, (c.values[i] - l.values[i]), (h.values[i] - c.values[i])) / (h.values[i] - l.values[i]).clip(1e-12)
    E["range_atr"] = (h.values[i] - l.values[i]) / E.atr
    E["trend24"] = (c.values[i] / c.shift(24).values[i] - 1) / (E.atr / c.values[i]) * E.side   # +ve = sweep fades WITH 24h trend
    E["i"] = i
    return E.dropna(subset=["atr"])


def outcome(df, E, name, H=24, stop_pad=0.25, confirm=False):
    """Net R per event. confirm=True: only enter if bar i+1 closes in the fade direction beyond the sweep bar close (enter at i+2 open)."""
    o, h, l, c = df.open.values, df.high.values, df.low.values, df.close.values
    out = []
    for t, e in E.iterrows():
        i = int(e.i); s = int(e.side)
        if confirm:
            if i + 2 >= len(o) or not (s * (c[i + 1] - c[i]) > 0): out.append(np.nan); continue
            j = i + 2
        else:
            j = i + 1
        if j + H >= len(o): out.append(np.nan); continue
        entry = o[j]; stop = e.ext - s * stop_pad * e.atr; risk = s * (entry - stop)
        if risk <= 0.05 * e.atr: out.append(np.nan); continue          # entry already beyond stop: skip
        R = None; tp = e.get("tp", np.nan) if hasattr(e, "get") else np.nan
        for k in range(j, j + H):
            if (s == 1 and l[k] <= stop) or (s == -1 and h[k] >= stop): R = -1.0; break
            if tp == tp and ((s == 1 and h[k] >= tp) or (s == -1 and l[k] <= tp)): R = s * (tp - entry) / risk; break
        if R is None: R = s * (c[j + H - 1] - entry) / risk
        cost = COST[name] if COST[name] is not None else COST_FRAC[name] * entry
        out.append(R - cost / risk)
    return pd.Series(out, index=E.index)


ERAS = [("2011-2015", "2011", "2015"), ("2016-2020", "2016", "2020"), ("2021-2025", "2021", "2025"), ("HELDOUT-2026", "2026", "2026")]
METAL_ERAS = [("2018-2021", "2018", "2021"), ("2022-2025", "2022", "2025")]


def era_table(R):
    rows = {}
    for nm, a, b in ERAS:
        x = R[a:b].dropna()
        rows[nm] = (len(x), x.mean() if len(x) else np.nan)
    return rows


def boot_ci(x, n=2000, seed=0, block=20):
    """Block bootstrap of the mean (blocks of consecutive events) to respect clustering."""
    x = np.asarray(x.dropna()); m = len(x)
    if m < 20: return (np.nan, np.nan)
    rng = np.random.default_rng(seed); nb = int(np.ceil(m / block)); res = []
    for _ in range(n):
        st = rng.integers(0, m - block + 1, nb); s = np.concatenate([x[k:k + block] for k in st])[:m]; res.append(s.mean())
    return tuple(np.percentile(res, [2.5, 97.5]))
