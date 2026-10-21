"""Nucleus trace: per minute of the hour, how much the nucleus balances say about where the hour (and the next) closes (kev-brain/experiments/2026-10-21-nucleus.md).

Descriptive only, nothing tuned. Outcome z = ln(close / price at mh) / (sigma * sqrt(minutes left)), sigma = std of 1m log returns over the
prior 1440 minutes. Per mh: Spearman rho, AUC for z > 0, and a permutation band (95th pct |rho| over 100 hour shuffles). States: mean z
with day-cluster bootstrap and BH q. Writes out/lab/trace.json.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parent))
import data  # noqa: E402
import nucleus as N  # noqa: E402
import patterns  # noqa: E402

FEATS = ["bal", "slope_bal"] + [f"{k}.bal" for k in N.TFS]
MH = np.arange(59)
STATE_MH = (0, 15, 30, 45, 55)
SPLIT = 1_640_995_200_000  # 2022-01-01 UTC
END = 1_672_531_200_000  # 2023-01-01 UTC
PERMS = 100


def r4(x) -> list:
    return [None if not np.isfinite(v) else round(float(v), 4) for v in np.atleast_1d(x)]


def hours(nu: dict) -> dict:
    """Complete hours as [hour, mh] matrices of row indices, plus outcomes."""
    t = nu["t"]
    H = t // 3_600_000
    uh, start, cnt = np.unique(H, return_index=True, return_counts=True)
    ok = cnt == 60
    uh, start = uh[ok], start[ok]
    I = start[:, None] + np.arange(60)  # rows of every minute of each complete hour
    lc = np.log(nu["c"].astype(float))
    r = np.diff(lc, prepend=np.nan)
    sig = pd.Series(r).rolling(1440).std().to_numpy()  # data-derived scale, known at the minute's close
    close = lc[I[:, 59]]
    nxt = np.searchsorted(uh, uh + 1)
    has = (nxt < uh.size) & (uh[np.clip(nxt, 0, uh.size - 1)] == uh + 1)
    nclose = np.where(has, close[np.clip(nxt, 0, uh.size - 1)], np.nan)
    rows = I[:, :59]
    s = sig[rows]
    good = nu["valid"][rows] & (s > 0)
    z = np.where(good, (close[:, None] - lc[rows]) / (s * np.sqrt(59 - MH)), np.nan)
    zn = np.where(good, (nclose[:, None] - lc[rows]) / (s * np.sqrt(119 - MH)), np.nan)
    return {"hour": uh * 3_600_000, "rows": rows, "z": z, "zn": zn}


def curves(F: dict, z: np.ndarray, U: np.ndarray) -> dict:
    """rho, band, auc per feature per mh. F: feature -> [hour, mh]; z: [hour, mh]. U: one random matrix -> shared hour shuffles."""
    out = {f: {"rho": [], "band": [], "auc": []} for f in F}
    for mh in MH:
        ok = np.isfinite(z[:, mh]) & np.all([np.isfinite(F[f][:, mh]) for f in F], 0)
        n = int(ok.sum())
        if n < 50:
            for f in F:
                for k in out[f]:
                    out[f][k].append(np.nan)
            continue
        rz = pd.Series(z[ok, mh]).rank().to_numpy()
        rz = (rz - rz.mean()) / rz.std()
        RF = np.stack([pd.Series(F[f][ok, mh]).rank().to_numpy() for f in F], 1)
        RF = (RF - RF.mean(0)) / np.where(RF.std(0) > 0, RF.std(0), 1)
        rho = rz @ RF / n
        perm = np.argsort(U[:, :n], 1)  # same shuffles for every mh and feature
        band = np.quantile(np.abs(rz[perm] @ RF / n), 0.95, 0)
        pos = z[ok, mh] > 0
        npos, nneg = pos.sum(), (~pos).sum()
        RR = np.stack([pd.Series(F[f][ok, mh]).rank().to_numpy() for f in F], 1)
        auc = (RR[pos].sum(0) - npos * (npos + 1) / 2) / (npos * nneg)
        for i, f in enumerate(F):
            out[f]["rho"].append(rho[i])
            out[f]["band"].append(band[i])
            out[f]["auc"].append(auc[i])
    return {f: {k: r4(v) for k, v in d.items()} for f, d in out.items()}


def states(nu: dict, hz: dict, sel: np.ndarray, period: str) -> list[dict]:
    rows = []
    for mh in STATE_MH:
        i = hz["rows"][sel, mh]
        z = hz["z"][sel, mh]
        day = nu["t"][i] // 86_400_000
        o, cl = nu["1h.order"][i], nu["1h.cloud"][i]
        B = np.stack([nu[f"{k}.bal"][i] for k in N.TFS], 1)
        al = np.where((B > 0).all(1), "all up", np.where((B < 0).all(1), "all down", "mixed"))
        masks = {f"1h {N.ORDERS[a]} {['below', 'in', 'above'][b + 1]} cloud": (o == a) & (cl == b) for a in range(6) for b in (1, 0, -1)}
        masks |= {f"6 TF bal {a}": al == a for a in ("all up", "all down", "mixed")}
        for name, mk in masks.items():
            mk = mk & np.isfinite(z)
            if mk.sum() < 30:
                continue
            m, lo, hi, p = patterns.boot(z[mk], day[mk])
            rows.append({"period": period, "mh": mh, "state": name, "n": int(mk.sum()), "mean": m, "lo": lo, "hi": hi, "p": p})
    for r, q in zip(rows, patterns.bh([r.pop("p") for r in rows])):
        r["q"] = q
        for k in ("mean", "lo", "hi", "q"):
            r[k] = r4(r[k])[0]
    return rows


def verdict(cur: dict, nh: dict) -> str:
    def above(c, f):  # minutes where |rho| > band in both periods with the same sign
        a, b = c["discovery"][f], c["check"][f]
        return [mh for mh in MH if None not in (a["rho"][mh], b["rho"][mh], a["band"][mh], b["band"][mh])
                and abs(a["rho"][mh]) > a["band"][mh] and abs(b["rho"][mh]) > b["band"][mh] and np.sign(a["rho"][mh]) == np.sign(b["rho"][mh])]
    out = []
    for label, c in (("Rest of the hour", cur), ("Next hour", nh)):
        hit = {f: above(c, f) for f in FEATS}
        hit = {f: v for f, v in hit.items() if v}
        if not hit:
            out.append(f"{label}: no feature rises above its permutation band in both periods.")
            continue
        parts = []
        for f, v in hit.items():
            rd, rc = c["discovery"][f]["rho"], c["check"][f]["rho"]
            k = max(v, key=lambda mh: abs(rc[mh]))
            parts.append(f"{f} at {len(v)}/59 minutes (e.g. mh {k}: rho {rd[k]:+.3f} / {rc[k]:+.3f})")
        out.append(f"{label}: above band in both periods with the same sign: " + "; ".join(parts) + ".")
    out.append("The 1m lines carry bid-ask bounce (price at the last tick vs the minute's range), so a negative 1m rho is expected and not tradable. "
               "Descriptive only: |rho| of a few hundredths is a weak edge before costs.")
    return " ".join(out)


def main() -> dict:
    m = N.load_m1()
    nu = N.nucleus(m)
    del m
    hz = hours(nu)
    F = {f: nu[f][hz["rows"]].astype(float) for f in FEATS}
    first = int(nu["t"][np.argmax(nu["valid"])])
    per = {"discovery": (hz["hour"] >= first) & (hz["hour"] < SPLIT), "check": (hz["hour"] >= SPLIT) & (hz["hour"] < END)}
    U = np.random.default_rng(7).random((PERMS, hz["hour"].size))
    res = {"symbol": "SOLUSDT", "periods": {}, "n_hours": {}, "minutes": MH.tolist(), "features": FEATS, "curves": {}, "next_hour": {}, "states": []}
    for p, sel in per.items():
        sel = sel & np.isfinite(hz["z"]).any(1)
        res["periods"][p] = [int(hz["hour"][sel].min()), int(hz["hour"][sel].max() + 3_599_999)]
        res["n_hours"][p] = int(sel.sum())
        Fp = {f: v[sel] for f, v in F.items()}
        res["curves"][p] = curves(Fp, hz["z"][sel], U)
        res["next_hour"][p] = curves(Fp, hz["zn"][sel], U)
        res["states"] += states(nu, hz, sel, p)
    res["verdict"] = verdict(res["curves"], res["next_hour"])
    out = data.ROOT / "out" / "lab"
    out.mkdir(parents=True, exist_ok=True)
    (out / "trace.json").write_text(json.dumps(res))
    return res


if __name__ == "__main__":
    import time
    t0 = time.time()
    r = main()
    print(f"{time.time() - t0:.0f}s", r["n_hours"])
    print(r["verdict"])
