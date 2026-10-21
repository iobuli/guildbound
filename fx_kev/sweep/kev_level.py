"""N7: Kev-0.8B judges the swept LEVEL only (rule frozen in ledger.py wave 9). Pre-2026 events only; 2026 never touched."""
import json, os, sys
import numpy as np, pandas as pd
sys.path.insert(0, os.path.dirname(__file__))
from sweep_lib import events, load, outcome, levels, UPPER
from run_ledger import thin, INSTR
from kev_sweep import NAMES, PRETTY

CACHE = os.path.join(os.path.dirname(__file__), "kev_level_cache.json")
QTXT = "Is this a significant liquidity level where many stop orders are likely resting?"


def age(df, L, i, lv):
    t = df.index[i]
    if lv in ("PDH", "PDL"): return t.hour                                   # bars since previous day ended
    if lv in ("PWH", "PWL"): return int((t - t.to_period("W").start_time) / pd.Timedelta("1h"))
    if lv in ("ASH", "ASL"): return t.hour - 6
    if lv in ("SWH", "SWL"):
        col = df.high.values if lv == "SWH" else df.low.values; v = L[lv].values[i]
        for j in range(i - 3, max(i - 2000, 0), -1):
            if col[j] == v: return i - j
    return None                                                               # round numbers: not computable


def describe(df, L, e, name):
    i = int(e.i); a = e.atr; parts = []
    for lv in e.level.split(","):
        v = L[lv].values[i]; col = df.high.values if lv in UPPER else df.low.values
        touches = int(np.sum(np.abs(col[max(i - 48, 0):i] - v) <= 0.1 * a))
        ag = age(df, L, i, lv)
        s = f"{NAMES[lv]}"
        if ag is not None: s += f", which formed {ag} hours ago"
        s += f"; in the previous 48 hours price came within a tenth of an average hourly range of it {touches} time{'s' if touches != 1 else ''}"
        parts.append(s)
    side = "resistance above price" if int(e.side) == -1 else "support below price"
    return f"{PRETTY[name]} 1-hour chart. A price level acting as {side}: " + ". Also at the same price: ".join(parts) + "."


def main(sample=100):
    from typesafe_sdk import Noul, TypeSafeClient
    cl = TypeSafeClient(api_key="local", base_url="http://127.0.0.1:8008", model="kev-latest")
    cache = json.load(open(CACHE)) if os.path.exists(CACHE) else {}
    res = {}
    for name in INSTR:
        df = load(name, include_holdout=False); E = events(df, name); L = levels(df, name)
        R = outcome(df, E, name, H=24); E = E.loc[thin(E.i, 24)]; E["R"] = R.reindex(E.index); E = E.dropna(subset=["R"])
        pre = E[:"2025"]; pick = pre.iloc[np.unique(np.linspace(0, len(pre) - 1, min(sample, len(pre))).astype(int))]
        ps = []
        for k, (t, e) in enumerate(pick.iterrows()):
            key = f"{name}|{t}"
            if key not in cache:
                r = cl.system_one(state=describe(df, L, e, name), questions={"q": Noul(instructions=QTXT)})
                cache[key] = r.answers["q"].noul
            ps.append(cache[key])
            if k % 25 == 0: json.dump(cache, open(CACHE, "w")); print(name, k, flush=True)
        json.dump(cache, open(CACHE, "w"))
        p = pd.Series(ps, index=pick.index); x = pick.R
        rho = float(p.rank().corr(x.rank())); gated = float(x[p > p.median()].mean())
        res[name] = (len(x), rho, gated, float(x.mean()), float(p.min()), float(p.max()))
    line = "N7  "
    for n, (m, rho, g, u, lo, hi) in res.items():
        line += f" | {n[:6]} n={m} rho={rho:+.3f} gated={g:+.3f} ungated={u:+.3f} P[{lo:.2f},{hi:.2f}]"
    ok = all(v[1] > 0 for v in res.values()) and all(v[2] > v[3] for v in res.values())
    print(line + f" | PASS={ok}")


if __name__ == "__main__":
    main()
