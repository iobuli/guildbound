"""Wave 3: Kev scores S1 sweep events from a plain-English narrative. See ledger.py for the frozen rule."""
import json, os, sys
import numpy as np, pandas as pd
sys.path.insert(0, os.path.dirname(__file__))
from sweep_lib import events, load, outcome, boot_ci, levels
from run_ledger import thin, INSTR

CACHE = os.path.join(os.path.dirname(__file__), "kev_sweep_cache.json")
NAMES = {"PDH": "yesterday's high", "PDL": "yesterday's low", "PWH": "last week's high", "PWL": "last week's low", "ASH": "today's Asian-session high",
         "ASL": "today's Asian-session low", "SWH": "the most recent swing high", "SWL": "the most recent swing low", "RNU": "a round number", "RND": "a round number"}
PRETTY = {"USDJPY_H1": "USD/JPY", "SOLUSDT_H1": "SOL/USDT", "XAUUSD_H1": "gold (XAU/USD)", "XAGUSD_H1": "silver (XAG/USD)"}
Q = dict(instructions="This describes a liquidity sweep on a 1-hour chart. Over the next 24 hours, what is more likely?",
         criteria={"reversal": "It was a stop-hunt: price moves away from the swept level in the opposite direction of the wick",
                   "continuation": "The level gives way: price returns through it and continues in the direction of the wick"})


def narrative(df, e, name):
    i = int(e.i); t = df.index[i]; c = df.close.values; side = int(e.side)
    lv = " and ".join(sorted({NAMES[x] for x in e.level.split(",")}))
    wick_dir = "above" if side == -1 else "below"
    r24 = (c[i] / c[i - 24] - 1) / (e.atr / c[i]); r120 = (c[i] / c[i - 120] - 1) / (e.atr / c[i])
    move = lambda r: f"{'rose' if r > 0 else 'fell'} {abs(r):.1f} times its hourly ATR"
    return (f"{PRETTY[name]}, {t.day_name()} {t.hour:02d}:00. In this hour price pushed {wick_dir} {lv}, then closed back on the other side of it. "
            f"The bar's range was {e.range_atr:.1f} times the average hourly range and it closed {e.close_loc * 100:.0f}% of the way back from the wick extreme. "
            f"Over the previous 24 hours price {move(r24)}; over the previous 5 days it {move(r120)}. "
            f"Volatility before the sweep was {'unusually compressed' if e.comp else 'normal'}.")


def main(sample=250, include_2026=False, fresh=False):
    from typesafe_sdk import Choice, TypeSafeClient
    cl = TypeSafeClient(api_key="local", base_url="http://127.0.0.1:8008", model="kev-latest")
    cache = json.load(open(CACHE)) if os.path.exists(CACHE) else {}
    out = {}
    for name in INSTR:
        df = load(name, include_holdout=True); E = events(df, name)
        R = outcome(df, E, name, H=24); E = E.loc[thin(E.i, 24)]; E["R"] = R.reindex(E.index); E = E.dropna(subset=["R"])
        pre = E[:"2025"]; grid = np.linspace(0, len(pre) - 1, min(sample, len(pre)))
        if fresh: grid = (grid[:-1] + grid[1:]) / 2 + 0.5          # midpoints between K1 picks: disjoint fresh sample
        pick = pre.iloc[np.unique(grid.astype(int))]
        if include_2026: pick = pd.concat([pick, E["2026":]])
        ps = []
        for k, (t, e) in enumerate(pick.iterrows()):
            key = f"{name}|{t}"
            if key not in cache:
                r = cl.system_one(state=narrative(df, e, name), questions={"q": Choice(**Q)})
                cache[key] = r.choices["q"].probabilities["reversal"]
            ps.append(cache[key])
            if k % 25 == 0: json.dump(cache, open(CACHE, "w")); print(name, k, len(pick), flush=True)
        json.dump(cache, open(CACHE, "w"))
        pick = pick.assign(p=ps); out[name] = pick
    pd.to_pickle(out, os.path.join(os.path.dirname(__file__), "kev_sweep_fresh.pkl" if fresh else "kev_sweep_scored.pkl"))


if __name__ == "__main__":
    main(include_2026="--unseal" in sys.argv, fresh="--fresh" in sys.argv)
