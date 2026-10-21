"""Harness for the multi-agent proposal batch. See proposals/README.md. Usage: eval_proposal.py {check|register|evaluate|unseal} ID"""
import fcntl
import hashlib
import importlib.util
import json
import os
import sys
import time

import numpy as np
import pandas as pd

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from sweep_lib import COST, COST_FRAC, ERAS, METAL_ERAS, boot_ci, load  # noqa: E402

PDIR = os.path.join(HERE, "proposals")
REG, RES = os.path.join(PDIR, "registry.jsonl"), os.path.join(PDIR, "results.jsonl")
DISC, METALS = ["USDJPY_H1", "SOLUSDT_H1"], ["XAUUSD_H1", "XAGUSD_H1"]
INSTR = DISC + METALS
PRIOR_METHODS = 25          # S1-S13, K1-K3, N1-N8, K4


def path(pid): return os.path.join(PDIR, f"P_{pid}.py")


def sha(pid): return hashlib.sha256(open(path(pid), "rb").read()).hexdigest()


def module(pid):
    spec = importlib.util.spec_from_file_location(f"P_{pid}", path(pid)); m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
    assert m.PREREG["id"] == pid, "PREREG id must match file name"
    for k in ("family", "desc", "rationale", "novelty", "data_informed", "H", "pad"): assert k in m.PREREG, f"PREREG missing {k}"
    assert 1 <= int(m.PREREG["H"]) <= 240
    return m


def jl(p):
    return [json.loads(l) for l in open(p)] if os.path.exists(p) else []


def append(p, obj):
    with open(p, "a") as f:
        fcntl.flock(f, fcntl.LOCK_EX); f.write(json.dumps(obj, default=float) + "\n"); fcntl.flock(f, fcntl.LOCK_UN)


def gen(m, df, name):
    E = m.generate(df, name)
    if E is None or len(E) == 0: return pd.DataFrame(columns=["i", "side", "ext", "atr"])
    E = E.copy(); E = E[~E.index.duplicated(keep="first")].sort_index()
    for c in ("i", "side", "ext", "atr"): assert c in E.columns, f"missing column {c}"
    E["i"] = E["i"].astype(int); E["side"] = E["side"].astype(int)
    assert set(E.side.unique()) <= {1, -1}
    assert (df.index[E.i.values] == E.index).all(), "row index must equal df.index[i]"
    return E.dropna(subset=["atr", "ext"])


def simulate(df, E, name, H, pad):
    o, h, l, c = df.open.values, df.high.values, df.low.values, df.close.values; n = len(o); out = []
    for t, e in E.iterrows():
        i, s = int(e.i), int(e.side); j = i + 1
        if j + H >= n: out.append(np.nan); continue
        entry = o[j]; stop = e.ext - s * pad * e.atr; risk = s * (entry - stop)
        if not np.isfinite(risk) or risk <= 0.05 * e.atr: out.append(np.nan); continue
        tp = e.tp if "tp" in E.columns else np.nan; R = None
        for k in range(j, j + H):
            if (s == 1 and l[k] <= stop) or (s == -1 and h[k] >= stop): R = s * (min(stop, o[k]) - entry) / risk if s == 1 else s * (max(stop, o[k]) - entry) / risk; break
            if tp == tp and ((s == 1 and h[k] >= tp) or (s == -1 and l[k] <= tp)): R = s * (tp - entry) / risk; break
        if R is None: R = s * (c[j + H - 1] - entry) / risk
        cost = COST[name] if COST[name] is not None else COST_FRAC[name] * entry
        out.append(R - cost / risk)
    return pd.Series(out, index=E.index, dtype=float)


def thin(E, gap):
    keep, last = [], -10**9
    for t, i in E.i.items():
        if i - last >= gap: keep.append(t); last = i
    return E.loc[keep]


def check(pid):
    m = module(pid); rep = {"id": pid, "sha256": sha(pid)}; rng = np.random.default_rng(0); bad = 0
    for name in INSTR:
        df = load(name, include_holdout=False); E = gen(m, df, name); Et = thin(E, int(m.PREREG["H"]))
        rep[name] = dict(signals=len(E), non_overlapping=len(Et), long_share=round(float((E.side == 1).mean()), 3) if len(E) else None,
                         by_year={str(k): int(v) for k, v in Et.groupby(Et.index.year).size().items()})
        if len(E) == 0: continue
        for t in E.index[rng.choice(len(E), min(10, len(E)), replace=False)]:
            d2 = df.copy(); fut = d2.index > t
            d2.loc[fut, ["open", "high", "low", "close"]] = rng.uniform(0.5, 2.0, (fut.sum(), 4)) * d2.close.iloc[0]
            d2.loc[fut, "volume"] = rng.integers(1, 10**6, fut.sum())
            E2 = gen(m, d2, name); cols = [c for c in ("i", "side", "ext", "atr", "tp", "w") if c in E.columns]
            if t not in E2.index or not np.allclose(E.loc[t, cols].astype(float).values, E2.loc[t, cols].astype(float).values, equal_nan=True): bad += 1
        # also: no signal may appear at a time later than the last bar it could see (sanity)
    rep["corruption_failures"] = bad
    print(json.dumps(rep, indent=1)); return rep


def register(pid):
    rep = check(pid)
    if rep["corruption_failures"]: sys.exit(f"REFUSED: {rep['corruption_failures']} lookahead failures; fix generate() first")
    if any(r["id"] == pid for r in jl(REG)): sys.exit("REFUSED: already registered")
    append(REG, {"id": pid, "sha256": sha(pid), "registered_at": time.strftime("%Y-%m-%dT%H:%M:%S"), "prereg": module(pid).PREREG})
    print(f"REGISTERED {pid}")


def stats(R, w, eras):
    pre = R[:"2025"].dropna(); wp = w.reindex(pre.index)
    if len(pre) < 20: return dict(n=len(pre), mean=None, ci=[None, None], p=1.0, eras={nm: None for nm, _, _ in eras})
    x = pre * wp / wp.mean(); lo, hi = boot_ci(x)
    rng = np.random.default_rng(1); bs = np.array([rng.choice(x.values, len(x)).mean() for _ in range(2000)])
    er = {}
    for nm, a, b in eras:
        s = R[a:b].dropna(); ws = w.reindex(s.index)
        er[nm] = round(float((s * ws).sum() / ws.sum()), 4) if len(s) >= 5 else None
    return dict(n=len(pre), mean=round(float((pre * wp).sum() / wp.sum()), 4), ci=[round(float(lo), 4), round(float(hi), 4)],
                p=round(float((bs <= 0).mean()), 4), eras=er, win=round(float((pre > 0).mean()), 3))


def evaluate(pid, unseal=False):
    reg = [r for r in jl(REG) if r["id"] == pid]
    if not reg: sys.exit("REFUSED: not registered")
    if reg[0]["sha256"] != sha(pid): sys.exit("REFUSED: file changed after registration")
    done = [r for r in jl(RES) if r["id"] == pid]
    if done and not unseal: print(json.dumps(done[-1], indent=1)); return done[-1]
    m = module(pid); H, pad = int(m.PREREG["H"]), float(m.PREREG["pad"])
    n_reg = max(40, PRIOR_METHODS + len(jl(REG)))
    res = {"id": pid, "family": m.PREREG["family"], "data_informed": m.PREREG["data_informed"], "n_registered": n_reg}
    held = {}
    for name in INSTR:
        df = load(name, include_holdout=True); E = thin(gen(m, df, name), H)
        R = simulate(df, E, name, H, pad); w = E["w"].astype(float) if "w" in E.columns else pd.Series(1.0, index=E.index)
        res[name] = stats(R, w, ERAS[:3] if name in DISC else METAL_ERAS)
        post = R["2026":].dropna(); held[name] = (round(float((post * w.reindex(post.index)).sum() / w.reindex(post.index).sum()), 4) if len(post) else None, len(post))
    u, s = res["USDJPY_H1"], res["SOLUSDT_H1"]
    ok = lambda v: v is not None and v > 0
    res["P1"] = all(ok(r["ci"][0]) for r in (u, s))
    res["P2"] = all(ok(u["eras"][e]) for e in ("2011-2015", "2016-2020", "2021-2025")) and ok(s["eras"]["2021-2025"])
    res["P3"] = max(u["p"], s["p"]) < 0.05 / n_reg
    res["P5"] = all(ok(res[n]["eras"][e]) for n in METALS for e in ("2018-2021", "2022-2025"))
    res["GATE"] = bool(res["P1"] and res["P2"] and res["P3"] and res["P5"])
    if unseal:
        if not done or not done[-1]["GATE"]: sys.exit("REFUSED: unseal only for a method that passed the gate")
        res = dict(done[-1]); res["heldout_2026"] = {k: {"mean": v[0], "n": v[1]} for k, v in held.items()}
        res["P4"] = all(v[0] is not None and v[0] > 0 for v in held.values()); res["CLAIMED"] = res["GATE"] and res["P4"]
        append(RES, res)
    else:
        res["heldout_2026"] = "SEALED"; append(RES, res)
    print(json.dumps(res, indent=1)); return res


if __name__ == "__main__":
    cmd, pid = sys.argv[1], sys.argv[2]
    {"check": check, "register": register, "evaluate": evaluate, "unseal": lambda p: evaluate(p, unseal=True)}[cmd](pid)
