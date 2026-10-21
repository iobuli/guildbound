"""Pattern lab (kev-brain/experiments/2026-10-21-pattern-lab.md): what price does after 3-touch level fills, sweeps and structure breaks.

Every event is entered at its bar's close and measured in prior-day ATR, signed in the trade's direction, then compared with all bars of
the same month and direction (lift). 2020-2022 is studied; the refinement rule picks at most 5 patterns to check once on 2023 onward.
"""
from __future__ import annotations

import datetime as dt
import json
import math
import sys
from pathlib import Path

import numpy as np
import pandas as pd
from numpy.lib.stride_tricks import sliding_window_view as win

sys.path.insert(0, str(Path(__file__).resolve().parent))
import data  # noqa: E402

HORIZONS = [1, 4, 12, 24, 72]
PAIRS = [(0.5, 0.5), (0.5, 1.0), (1.0, 1.0), (1.0, 2.0)]
TIMEOUT = 72
DESIGN = (dt.datetime(2020, 1, 1, tzinfo=dt.timezone.utc), dt.datetime(2023, 1, 1, tzinfo=dt.timezone.utc))
EV_DIR = data.ROOT / "out" / "events"


def ms(d: dt.datetime) -> int:
    return int(d.timestamp() * 1000)


# ------------------------------------------------------------------ per-bar measures
def bar_measures(b: dict, atr: np.ndarray, d: int) -> dict[str, np.ndarray]:
    """Every measure for an entry at each bar's close in direction d (+1 long, -1 short), in prior-day ATR."""
    c, h, l = b["c"], b["h"], b["l"]
    n = c.size
    out: dict[str, np.ndarray] = {}
    for hz in HORIZONS:
        f = np.full(n, np.nan)
        f[: n - hz] = d * (c[hz:] - c[: n - hz]) / atr[: n - hz]
        out[f"r{hz}"] = f
    # future highs/lows over the next TIMEOUT bars, aligned so row k holds bars k+1..k+TIMEOUT
    m = n - TIMEOUT
    H = win(h[1:], TIMEOUT)[:m]
    L = win(l[1:], TIMEOUT)[:m]
    fav = (np.maximum.accumulate(H, axis=1) - c[:m, None]) if d == 1 else (c[:m, None] - np.minimum.accumulate(L, axis=1))
    adv = (c[:m, None] - np.minimum.accumulate(L, axis=1)) if d == 1 else (np.maximum.accumulate(H, axis=1) - c[:m, None])
    a = atr[:m, None]
    pad = lambda x: np.concatenate([x, np.full(n - m, np.nan)])  # noqa: E731
    out["mfe24"] = pad(fav[:, 23] / atr[:m])
    out["mae24"] = pad(adv[:, 23] / atr[:m])
    end = d * (c[TIMEOUT:] - c[:m])
    for s, t in PAIRS:
        hit_t, hit_s = fav >= t * a, adv >= s * a
        it = np.where(hit_t.any(1), hit_t.argmax(1), TIMEOUT)
        is_ = np.where(hit_s.any(1), hit_s.argmax(1), TIMEOUT)
        r = np.where(is_ <= it, -1.0, t / s)  # a stop on the same bar as the target counts first
        r = np.where((is_ == TIMEOUT) & (it == TIMEOUT), end / (s * atr[:m]), r)
        out[f"S{s}_T{t}"] = pad(r)
        out[f"hit S{s}_T{t}"] = pad(np.where((it < is_), 1.0, 0.0))
    return out


MEASURES = [f"r{h}" for h in HORIZONS] + ["mfe24", "mae24"] + [f"S{s}_T{t}" for s, t in PAIRS]


# ------------------------------------------------------------------ events per market
def market_events(sym: str) -> tuple[pd.DataFrame, dict]:
    b = data.h1(sym)
    e = json.loads((EV_DIR / f"{sym}.json").read_text())
    atr = np.array([np.nan if x is None else x for x in e["atr"]], float)
    assert atr.size == b["t"].size, f"{sym}: events were built on a different H1 file; rerun `kev events`"
    meas = {d: bar_measures(b, atr, d) for d in (1, -1)}
    month = (b["t"] // 86_400_000).astype("datetime64[D]").astype("datetime64[M]").astype(np.int64)
    # baseline: mean of every measure over all bars of the same month and direction
    base = {}
    for d in (1, -1):
        df = pd.DataFrame({k: meas[d][k] for k in MEASURES})
        df["month"] = month
        base[d] = df.groupby("month")[MEASURES].mean()
    ev = pd.DataFrame(e["ev"])
    ev["n"] = 0
    # kev's level fills (one-hour-spec) as events at their fill bar
    sig = data.signals(sym)["trades"]
    k_of = {int(t): i for i, t in enumerate(b["t"])}
    lv = pd.DataFrame([{"k": k_of[r["t"]], "t": r["t"], "kind": "level", "dir": r["dir"], "n": r["n"], "sp": r["sp"], "go": r["go"]} for r in sig if r["t"] in k_of])
    ev = pd.concat([ev, lv], ignore_index=True)
    ev = ev[ev["k"] < atr.size].copy()
    ev["k"] = ev["k"].astype(int)
    # volatility monitor: percentile of the prior-day ATR within the trailing year (one sample per day), same rule as `kev events`
    def vol_at(k: int) -> float:
        w = atr[np.arange(k - 24, max(-1, k - 24 * 365 - 1), -24)] if k >= 24 else np.array([])
        w = w[np.isfinite(w)]
        return float((w <= atr[k]).mean()) if w.size > 30 and np.isfinite(atr[k]) else np.nan
    ev["vol"] = [vol_at(int(k)) for k in ev["k"]]
    # combos
    sw = ev[ev["kind"] == "sweep"]
    sweep_k = {d: np.sort(sw[sw["dir"] == d]["k"].to_numpy()) for d in (1, -1)}

    def swept_within(k: int, d: int, lo: int) -> bool:
        a = sweep_k[d]
        i = np.searchsorted(a, k - lo)
        return i < a.size and a[i] <= k

    ev["after_sweep24"] = [bool(r.kind == "break" and r.msb == 1 and swept_within(r.k, r.dir, 24)) for r in ev.itertuples()]
    ev["sweep6"] = [bool(r.kind == "level" and swept_within(r.k, r.dir, 6)) for r in ev.itertuples()]
    # context known at the bar's close
    c = b["c"]
    k = ev["k"].to_numpy()
    back = np.maximum(k - 50 * 24, 0)
    ev["with50"] = np.sign(c[k] - c[back]) * ev["dir"].to_numpy() > 0
    hour = (ev["t"].to_numpy() // 3_600_000) % 24
    ev["session"] = np.select([hour < 7, hour < 13, hour < 20], ["Asia", "London", "NY"], "late")
    ev["volT"] = pd.cut(ev["vol"], [-0.01, 1 / 3, 2 / 3, 1.01], labels=["low vol", "mid vol", "high vol"]).astype(str)
    ev["day"] = ev["t"] // 86_400_000
    ev["mkt"] = sym
    for m_ in MEASURES:
        raw = np.array([meas[d][m_][kk] for kk, d in zip(k, ev["dir"])])
        bl = np.array([base[d][m_].get(mm, np.nan) for mm, d in zip(month[k], ev["dir"])])
        ev[m_] = raw
        ev["lift " + m_] = raw - bl
    for s_, t_ in PAIRS:
        key = f"S{s_}_T{t_}"
        ev["fade " + key] = np.array([meas[-d][key][kk] for kk, d in zip(k, ev["dir"])])
    ev["hit"] = np.array([meas[d]["hit S1.0_T2.0"][kk] for kk, d in zip(k, ev["dir"])])
    prof = json.loads((data.MARKETS / sym / "profile.json").read_text())
    ev["costR_s1"] = prof["costs"]["slipR"] + prof["costs"]["spreadBps"] * 1e-4 * c[k] / atr[k]  # per trade, for a 1-ATR stop
    ev = ev.dropna(subset=["r24"])
    return ev, {"bars": int(b["t"].size)}


# ------------------------------------------------------------------ statistics
def boot(v: np.ndarray, day: np.ndarray, reps: int = 600, seed: int = 11) -> tuple[float, float, float, float]:
    """Mean, day-cluster bootstrap 95% CI and two-sided p (share of resampled means on the other side of 0)."""
    ok = np.isfinite(v)
    v, day = v[ok], day[ok]
    if v.size < 10:
        return float("nan"), float("nan"), float("nan"), float("nan")
    _, inv = np.unique(day, return_inverse=True)
    s, n = np.bincount(inv, v), np.bincount(inv)
    rng = np.random.default_rng(seed)
    idx = rng.integers(0, s.size, (reps, s.size))
    bs = s[idx].sum(1) / n[idx].sum(1)
    m = float(v.mean())
    p = 2 * min((bs <= 0).mean(), (bs >= 0).mean())
    return m, float(np.quantile(bs, 0.025)), float(np.quantile(bs, 0.975)), float(min(1.0, max(p, 1 / reps)))


def bh(p: list[float]) -> list[float]:
    p = np.array([1.0 if not np.isfinite(x) else x for x in p])
    o = np.argsort(p)
    q = p[o] * p.size / np.arange(1, p.size + 1)
    q = np.minimum.accumulate(q[::-1])[::-1]
    out = np.empty_like(q)
    out[o] = np.minimum(q, 1)
    return list(out)


def groups(ev: pd.DataFrame) -> dict[str, pd.Series]:
    """Relaxed patterns first, then refinements. Each is a boolean mask over events."""
    k, lvl, sw, br = ev["kind"], ev["kind"] == "level", ev["kind"] == "sweep", ev["kind"] == "break"
    g = {
        "level fill, any touches": lvl, "level fill, 1 touch": lvl & (ev["n"] == 1), "level fill, 2 touches": lvl & (ev["n"] == 2),
        "T3: level fill, 3+ touches": lvl & (ev["n"] >= 3), "T3 + space": lvl & (ev["n"] >= 3) & (ev["sp"] == 1),
        "T3 + sweep in last 6 bars": lvl & (ev["n"] >= 3) & ev["sweep6"],
        "sweep (SFP), any": sw, "sweep of a deep swing (1+ ATR)": sw & (ev["deep"] >= 1), "sweep of a shallow swing": sw & (ev["deep"] < 1),
        "sweep, swing < 24 bars old": sw & (ev["age"] < 24), "sweep, swing 24-120 bars old": sw & (ev["age"] >= 24),
        "sweep, strong rejection (close in far third)": sw & (ev["closeLoc"] >= 2 / 3), "sweep, big bar (0.5+ ATR)": sw & (ev["barAtr"] >= 0.5),
        "break of structure, any": br, "MSB (break against prior structure)": br & (ev["msb"] == 1), "BOS continuation (not MSB)": br & (ev["msb"] == 0),
        "sweep then MSB within 24 bars": ev["after_sweep24"],
    }
    core = ["T3: level fill, 3+ touches", "sweep (SFP), any", "sweep of a deep swing (1+ ATR)", "MSB (break against prior structure)", "sweep then MSB within 24 bars", "BOS continuation (not MSB)"]
    for name in core:
        for col, vals in (("volT", ["low vol", "mid vol", "high vol"]), ("session", ["Asia", "London", "NY", "late"])):
            for v in vals:
                g[f"{name} | {v}"] = g[name] & (ev[col] == v)
        g[f"{name} | with 50-day move"] = g[name] & ev["with50"]
        g[f"{name} | against 50-day move"] = g[name] & ~ev["with50"]
    return g


def table(ev: pd.DataFrame, mkts: list[str]) -> list[dict]:
    rows = []
    for name, mask in groups(ev).items():
        x = ev[mask]
        if len(x) < 30:
            continue
        m, lo, hi, p = boot(x["lift r24"].to_numpy(), (x["day"].astype(str) + x["mkt"]).to_numpy())
        pairs = {f"S{s}_T{t}": float(np.nanmean(x[f"S{s}_T{t}"])) for s, t in PAIRS}
        best = max(pairs, key=pairs.get)
        rows.append({
            "group": name, "n": int(len(x)), "lift24": m, "lo": lo, "hi": hi, "p": p,
            **{f"r{h}": float(np.nanmean(x[f"r{h}"])) for h in HORIZONS}, **{f"lift r{h}": float(np.nanmean(x[f"lift r{h}"])) for h in HORIZONS},
            "mfe24": float(np.nanmedian(x["mfe24"])), "mae24": float(np.nanmedian(x["mae24"])), "pairs": pairs, "best": best,
            "best lift": float(np.nanmean(x["lift " + best])), "hit S1_T2": float(np.nanmean(x["hit"])), "costR_s1": float(np.nanmean(x["costR_s1"])),
            "per_market": {mk: (float(np.nanmean(x[x["mkt"] == mk]["lift r24"])) if (x["mkt"] == mk).sum() >= 10 else None) for mk in mkts},
            "n_market": {mk: int((x["mkt"] == mk).sum()) for mk in mkts},
        })
    for r, q in zip(rows, bh([r["p"] for r in rows])):
        r["q"] = q
    return rows


def quarters(ev: pd.DataFrame, names: list[str]) -> dict:
    """Granular view: lift of the 24-bar return per quarter, per pattern (pooled markets)."""
    g = groups(ev)
    qs = pd.to_datetime(ev["t"], unit="ms").dt.to_period("Q").astype(str)
    out = {}
    for name in names:
        x = ev[g[name]]
        out[name] = {q: {"n": int(len(y)), "lift24": float(y["lift r24"].mean()), "best": float(y["lift S1.0_T2.0"].mean())} for q, y in x.groupby(qs[g[name]]) if len(y) >= 5}
    return out


def select(rows: list[dict], mkts: list[str]) -> list[dict]:
    """The pre-registered refinement rule."""
    ok = []
    for r in rows:
        signs = [np.sign(v) for v in r["per_market"].values() if v is not None]
        same = sum(s == np.sign(r["lift24"]) for s in signs)
        if r["q"] < 0.05 and (r["lo"] > 0 or r["hi"] < 0) and same >= 2 and r["n"] >= 100:
            ok.append({**r, "t": abs(r["lift24"]) / max(1e-9, (r["hi"] - r["lo"]) / 3.92)})
    return sorted(ok, key=lambda r: -r["t"])[:5]


def confirm(ev: pd.DataFrame, chosen: list[dict]) -> list[dict]:
    """Once, on 2023 onward: the chosen patterns with their 2020-2022 best stop/target pair, net of costs."""
    g = groups(ev)
    out = []
    for r in chosen:
        x = ev[g[r["group"]]]
        sgn = 1 if r["lift24"] > 0 else -1  # a negative lift is traded the other way: the pattern fails
        key = r["best"] if sgn > 0 else min(r["pairs"], key=r["pairs"].get)
        s = float(key.split("_")[0][1:])
        R = x[key].to_numpy() if sgn > 0 else _flip(x, key)
        cost = x["costR_s1"].to_numpy() / s
        m, lo, hi, p = boot(R - cost, (x["day"].astype(str) + x["mkt"]).to_numpy())
        l24, l_lo, l_hi, l_p = boot(sgn * x["lift r24"].to_numpy(), (x["day"].astype(str) + x["mkt"]).to_numpy())
        out.append({"group": r["group"], "direction": "as signalled" if sgn > 0 else "faded", "pair": key, "n": int(len(x)),
                    "netR": m, "lo": lo, "hi": hi, "p": p, "lift24": l24, "lift_lo": l_lo, "lift_hi": l_hi, "design_lift24": r["lift24"]})
    return out


def _flip(x: pd.DataFrame, key: str) -> np.ndarray:
    """R of the opposite trade is not the negative of R (stops and targets differ); use the measure for the other direction."""
    return x["fade " + key].to_numpy()


def main() -> dict:
    mkts = [m for m in data.markets() if (EV_DIR / f"{m}.json").exists()]
    ev = pd.concat([market_events(m)[0] for m in mkts], ignore_index=True)
    t0, t1 = ms(DESIGN[0]), ms(DESIGN[1])
    des, fwd = ev[(ev["t"] >= t0) & (ev["t"] < t1)], ev[ev["t"] >= t1]
    rows = table(des, mkts)
    chosen = select(rows, mkts)
    res = {"design": [str(DESIGN[0].date()), str(DESIGN[1].date())], "markets": mkts, "n_events": int(len(des)), "table": rows,
           "quarters": quarters(des, ["T3: level fill, 3+ touches", "sweep (SFP), any", "sweep of a deep swing (1+ ATR)", "MSB (break against prior structure)", "sweep then MSB within 24 bars", "BOS continuation (not MSB)"]),
           "chosen": [r["group"] for r in chosen]}
    res["confirm"] = confirm(fwd, chosen) if chosen else []
    out = data.ROOT / "out" / "lab"
    out.mkdir(parents=True, exist_ok=True)
    (out / "patterns.json").write_text(json.dumps(res, default=lambda o: None if isinstance(o, float) and not math.isfinite(o) else float(o)))
    return res


if __name__ == "__main__":
    r = main()
    print(f"{r['n_events']} events in {r['design']}")
    for x in r["table"][:17]:
        print(f"{x['group'][:44]:44s} n {x['n']:5d}  lift24 {x['lift24']:+.3f} [{x['lo']:+.3f},{x['hi']:+.3f}] q {x['q']:.3f}  r1/4/24/72 {x['r1']:+.2f} {x['r4']:+.2f} {x['r24']:+.2f} {x['r72']:+.2f}  best {x['best']} {x['pairs'][x['best']]:+.2f}R  mkts {x['per_market']}")
    print("chosen:", r["chosen"])
    for c in r["confirm"]:
        print(c)
