"""K4: build Kev fine-tuning records (events before 2021) and the 2021-2025 evaluation sample. See ledger.py wave 11."""
import json, os, sys
import numpy as np, pandas as pd
sys.path.insert(0, os.path.dirname(__file__))
from sweep_lib import events, load, outcome
from run_ledger import thin, INSTR
from kev_sweep import narrative, Q

OUT = os.path.dirname(os.path.abspath(__file__))
train, ev = [], {}
for name in INSTR:
    df = load(name, include_holdout=False)              # never touches 2026
    E = events(df, name); R = outcome(df, E, name, H=24)
    E = E.loc[thin(E.i, 24)]; E["R"] = R.reindex(E.index); E = E.dropna(subset=["R"])
    tr = E[:"2020"]
    for t, e in tr.iterrows():
        train.append({"state": narrative(df, e, name), "questions": {"q": {"type": "choice", "instructions": Q["instructions"],
                      "criteria": Q["criteria"], "label": "reversal" if e.R > 0 else "continuation"}}})
    dv = E["2021":"2025"]; pick = dv.iloc[np.unique(np.linspace(0, len(dv) - 1, min(300, len(dv))).astype(int))]
    ev[name] = pick.assign(state=[narrative(df, e, name) for _, e in pick.iterrows()])
    print(name, "train", len(tr), "reversal share", round((tr.R > 0).mean(), 3), "| eval 2021-25", len(pick))
rng = np.random.default_rng(0); rng.shuffle(train)
with open(os.path.join(OUT, "k4_train.jsonl"), "w") as f:
    for r in train: f.write(json.dumps(r) + "\n")
pd.to_pickle(ev, os.path.join(OUT, "k4_eval_events.pkl"))
print("train records:", len(train))
