"""Evaluate each registered method once; append to results.jsonl. Also includes the causality corruption test."""
import json
import os
import sys

import numpy as np
import pandas as pd

sys.path.insert(0, os.path.dirname(__file__))
from ledger import METHODS
from sweep_lib import ERAS, METAL_ERAS, boot_ci, events, load, outcome
from methods2 import KINDS

WAVE1_REVEALED = {"S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8"}
OUT = os.path.join(os.path.dirname(__file__), "results_v2.jsonl")
DISC = ["USDJPY_H1", "SOLUSDT_H1"]
METALS = ["XAUUSD_H1", "XAGUSD_H1"]   # never seen before; frozen methods replicate here
INSTR = DISC + METALS


def corruption_test(name, n=10, seed=0):
    df = load(name, include_holdout=False); E = events(df, name)
    rng = np.random.default_rng(seed); bad = 0
    for t in E.index[rng.choice(len(E), n, replace=False)]:
        d2 = df.copy(); fut = d2.index > t
        d2.loc[fut, ["open", "high", "low", "close"]] = rng.uniform(1, 500, (fut.sum(), 4))
        E2 = events(d2, name)
        cols = ["side", "level", "atr", "ext", "close_loc", "trend24", "comp"]
        if t not in E2.index or not E.loc[t, cols].equals(E2.loc[t, cols]): bad += 1
    return bad


def select(E, m):
    x = E
    if m["levels"] != "ALL":
        allowed = set(m["levels"].split(","))
        x = x[x["level"].apply(lambda s: bool(set(s.split(",")) & allowed))]
    x = x[x.n_levels >= m["min_levels"]]
    if m["filt"] == "comp": x = x[x.comp == True]
    elif m["filt"]: x = x.query(m["filt"])
    return x


def thin(R, gap):
    """Keep non-overlapping events only (>= gap bars apart)."""
    keep, last = [], -10**9
    for t, i in R.items():
        if i - last >= gap: keep.append(t); last = i
    return keep


def evaluate(m, cache):
    """P1-P3 on the two discovery instruments (USDJPY, SOL), P5 replication on the never-seen metals, P4 held-out 2026 last."""
    res = {"id": m["id"], "desc": m["desc"]}
    n_reg = len(METHODS)
    for name in INSTR:
        df, E = cache[name]
        if m.get("kind"):
            S = KINDS[m["kind"]](df, name); R = outcome(df, S, name, H=m["H"])
        else:
            S = select(E, m); R = outcome(df, S, name, H=m["H"], confirm=m["confirm"], stop_pad=m.get("pad", 0.25))
        R = R.loc[thin(S.i, m["H"])].dropna()
        pre, post = R[:"2025"], R["2026":]
        lo, hi = boot_ci(pre)
        x = pre.values; rng = np.random.default_rng(1)
        bs = [rng.choice(x, len(x)).mean() for _ in range(2000)] if len(x) > 20 else [0]
        eras = ERAS[:3] if name in DISC else METAL_ERAS
        res[name] = dict(n_pre=len(pre), mean_pre=round(float(pre.mean()), 4), ci_pre=[round(float(lo), 4), round(float(hi), 4)],
                         p_pre=round(float(np.mean(np.array(bs) <= 0)), 4),
                         eras={nm: round(float(R[a_:b_].mean()), 4) if len(R[a_:b_]) else None for nm, a_, b_ in eras},
                         n_held=len(post), mean_held=round(float(post.mean()), 4) if len(post) else None)
    u, s = res["USDJPY_H1"], res["SOLUSDT_H1"]
    P1 = all(r["ci_pre"][0] > 0 for r in (u, s))
    P2 = all((u["eras"][e] or -1) > 0 for e in ("2011-2015", "2016-2020", "2021-2025")) and (s["eras"]["2021-2025"] or -1) > 0
    P3 = max(u["p_pre"], s["p_pre"]) < 0.05 / n_reg
    P5 = all((res[n]["eras"][e] or -1) > 0 for n in METALS for e in ("2018-2021", "2022-2025"))
    gate = P1 and P2 and P3 and P5
    if not gate:
        for n in INSTR: res[n]["mean_held"] = "SEALED"; res[n]["n_held"] = "SEALED"
    P4 = gate and all(isinstance(res[n]["mean_held"], float) and res[n]["mean_held"] > 0 for n in INSTR)
    res.update(P1=P1, P2=P2, P3=P3, P5=P5, P4=P4, CLAIMED=bool(gate and P4), n_registered_at_eval=n_reg)
    return res


def main():
    done = set()
    if os.path.exists(OUT):
        done = {json.loads(l)["id"] for l in open(OUT)}
    todo = [m for m in METHODS if m["id"] not in done]
    if not todo: print("nothing new to evaluate"); return
    for name in INSTR:
        b = corruption_test(name); print(f"corruption test {name}: {b} failures / 10"); assert b == 0, "lookahead!"
    cache = {}
    for name in INSTR:
        df = load(name, include_holdout=True); cache[name] = (df, events(df, name))
    with open(OUT, "a") as f:
        for m in todo:
            r = evaluate(m, cache); f.write(json.dumps(r) + "\n"); f.flush()
            line = f"{m['id']:4s}"
            for n in INSTR:
                q = r[n]; line += f" | {n[:6]} n={q['n_pre']:5d} R={q['mean_pre']:+.3f} [{q['ci_pre'][0]:+.3f},{q['ci_pre'][1]:+.3f}]"
            print(line + f" | P1={r['P1']:d} P2={r['P2']:d} P3={r['P3']:d} P5={r['P5']:d} P4={r['P4']:d} CLAIMED={r['CLAIMED']}")


if __name__ == "__main__":
    main()
