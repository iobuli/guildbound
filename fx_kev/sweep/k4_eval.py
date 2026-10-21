"""K4 evaluation, exactly as registered in ledger.py wave 11. Requires the fine-tuned checkpoint served on port 8009:
  uv run --extra serve python -m kev.serve --run /home/user/fx_kev/sweep/k4_run --port 8009 --device cpu
Scores the 2021-2025 evaluation sample (k4_eval_events.pkl), then applies the fixed pass rule. Also scores the same events with
the ORIGINAL kev-0.8b (port 8008, cache kev_sweep_cache style) only if --baseline is given, for a before/after comparison."""
import json
import os
import sys

import numpy as np
import pandas as pd

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from kev_sweep import Q  # noqa: E402
from sweep_lib import boot_ci  # noqa: E402

PORT = int(os.environ.get("K4_PORT", "8009"))
CACHE = os.path.join(HERE, f"k4_cache_{PORT}.json")


def score(ev):
    from typesafe_sdk import Choice, TypeSafeClient
    cl = TypeSafeClient(api_key="local", base_url=f"http://127.0.0.1:{PORT}", model="kev-latest")
    cache = json.load(open(CACHE)) if os.path.exists(CACHE) else {}
    out = {}
    for name, x in ev.items():
        ps = []
        for k, (t, e) in enumerate(x.iterrows()):
            key = f"{name}|{t}"
            if key not in cache:
                cache[key] = cl.system_one(state=e.state, questions={"q": Choice(**Q)}).choices["q"].probabilities["reversal"]
            ps.append(cache[key])
            if k % 50 == 0: json.dump(cache, open(CACHE, "w")); print(name, k, len(x), flush=True)
        json.dump(cache, open(CACHE, "w")); out[name] = x.assign(p=ps)
    return out


def judge(D):
    rows, pooled = [], []
    for name, x in D.items():
        x = x.sort_index(); med = x.p.shift(1).rolling(100, min_periods=30).median(); g = x.assign(med=med).dropna(subset=["med"])
        k = g[g.p > g.med].R
        rows.append(dict(instr=name, n=len(x), spearman=round(float(x.p.rank().corr(x.R.rank())), 3), ungated_R=round(float(g.R.mean()), 3),
                         gated_n=len(k), gated_R=round(float(k.mean()), 3), p_min=round(float(x.p.min()), 3), p_max=round(float(x.p.max()), 3)))
        pooled.append(k)
    T = pd.DataFrame(rows).set_index("instr"); pk = pd.concat(pooled).sort_index(); lo, hi = boot_ci(pk)
    c1, c2, c3 = bool((T.spearman > 0).all()), bool((T.gated_R > T.ungated_R).all()), bool(lo > 0)
    print(T.to_string()); print(f"pooled gated n={len(pk)} mean R={pk.mean():+.3f} CI [{lo:+.3f}, {hi:+.3f}]")
    print(f"K4 conditions: spearman>0 all 4 = {c1}; gated beats ungated all 4 = {c2}; pooled CI lower > 0 = {c3} -> PASS = {c1 and c2 and c3}")
    return dict(table=T.reset_index().to_dict("records"), pooled=[float(pk.mean()), float(lo), float(hi)], PASS=c1 and c2 and c3)


if __name__ == "__main__":
    ev = pd.read_pickle(os.path.join(HERE, "k4_eval_events.pkl"))
    res = judge(score(ev)); res["port"] = PORT
    with open(os.path.join(HERE, "k4_result.jsonl"), "a") as f: f.write(json.dumps(res) + "\n")
