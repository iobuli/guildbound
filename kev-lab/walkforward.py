"""2024-only nucleus logistic, frozen, then scored on 2025 and 2026 Jan-Sep.

The rules are kev-brain/experiments/2026-10-22-nucleus-walkforward.md.
Selection never receives an hour outside 2024.
"""
from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parent))
import nucleus as N  # noqa: E402
import patterns  # noqa: E402
import trace as T  # noqa: E402

MH_GRID = (0, 15, 30, 45)
LAM_GRID = (100.0, 10.0, 1.0, 0.1)
PERMS = 100
PERM_SEED = 7
ATR_N = 20
STOP_ATR = 0.5
SLIP_R = 0.1
SPREAD_BPS = 5.0
NEWTON_STEPS = 30


def ms(y: int, m: int, d: int) -> int:
    return int(datetime(y, m, d, tzinfo=timezone.utc).timestamp() * 1000)


TRAIN_START = ms(2024, 1, 1)
Y2025 = ms(2025, 1, 1)
Y2026 = ms(2026, 1, 1)
TEST_END = ms(2026, 10, 1)
QUARTERS = [ms(2024, 1, 1), ms(2024, 4, 1), ms(2024, 7, 1), ms(2024, 10, 1), ms(2025, 1, 1)]


def feature_names() -> list[str]:
    tfs = list(N.TFS)
    names = ["bal", "slope_bal"]
    names += [f"{tf}.bal" for tf in tfs]
    names += [f"{tf}.slope_bal" for tf in tfs]
    names += [f"{tf}.{n}.pos" for tf in tfs for n in N.NS]
    names += [f"{tf}.cloud" for tf in tfs]
    return names


def expanding_folds(hour_ms: np.ndarray) -> list[tuple[np.ndarray, np.ndarray]]:
    """Three expanding quarters. Rejects any hour outside 2024."""
    if hour_ms.size and np.any((hour_ms < QUARTERS[0]) | (hour_ms >= QUARTERS[-1])):
        raise ValueError("folds are 2024 only")
    folds = []
    for k in range(1, 4):
        train = hour_ms < QUARTERS[k]
        val = (hour_ms >= QUARTERS[k]) & (hour_ms < QUARTERS[k + 1])
        folds.append((train, val))
    return folds


def zstats(X: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    mu = X.mean(axis=0)
    sd = X.std(axis=0)
    sd = np.where(sd > 0, sd, 1.0)
    return mu, sd


def apply_z(X: np.ndarray, mu: np.ndarray, sd: np.ndarray) -> np.ndarray:
    return (X - mu) / sd


def _sigmoid(z: np.ndarray) -> np.ndarray:
    z = np.clip(z, -30, 30)
    return 1.0 / (1.0 + np.exp(-z))


def fit_logistic(X: np.ndarray, y01: np.ndarray, lam: float) -> np.ndarray:
    """Newton on mean log loss plus (lam/2)||w||^2. Column 0 of the returned weight is the intercept."""
    n, p = X.shape
    X1 = np.column_stack([np.ones(n), X])
    w = np.zeros(p + 1)
    y = y01.astype(float)
    for _ in range(NEWTON_STEPS):
        ph = _sigmoid(X1 @ w)
        grad = X1.T @ (ph - y) / n
        grad[1:] += lam * w[1:]
        var = np.clip(ph * (1.0 - ph), 1e-6, None)
        H = (X1 * var[:, None]).T @ X1 / n
        H[1:, 1:] += np.eye(p) * lam
        step = np.linalg.solve(H, grad)
        w -= step
        if np.linalg.norm(grad) < 1e-8:
            break
    return w


def linear_score(X: np.ndarray, w: np.ndarray, mu: np.ndarray, sd: np.ndarray) -> np.ndarray:
    Z = apply_z(X, mu, sd)
    return w[0] + Z @ w[1:]


def auc_score(score: np.ndarray, y01: np.ndarray) -> float:
    y = y01.astype(bool)
    npos, nneg = int(y.sum()), int((~y).sum())
    if npos == 0 or nneg == 0:
        return 0.5
    ranks = pd.Series(score).rank().to_numpy()
    return float((ranks[y].sum() - npos * (npos + 1) / 2) / (npos * nneg))


def _unit_rank(a: np.ndarray) -> np.ndarray | None:
    r = pd.Series(a).rank().to_numpy()
    sd = float(r.std())
    if not np.isfinite(sd) or sd == 0:
        return None
    return (r - r.mean()) / sd


def spearman(score: np.ndarray, z: np.ndarray) -> float:
    rs, rz = _unit_rank(score), _unit_rank(z)
    if rs is None or rz is None:
        return 0.0
    return float(rs @ rz / rs.size)


def rho_band(score: np.ndarray, z: np.ndarray) -> tuple[float, float]:
    rs, rz = _unit_rank(score), _unit_rank(z)
    if rs is None or rz is None:
        return 0.0, 1.0
    rho = float(rs @ rz / rs.size)
    n = rs.size
    perm = np.argsort(np.random.default_rng(PERM_SEED).random((PERMS, n)), axis=1)
    band = float(np.quantile(np.abs(rz[perm] @ rs / n), 0.95))
    return rho, band


def better(auc: float, lam: float, mh: int, best: dict | None) -> bool:
    if best is None:
        return True
    if auc > best["cv_auc"] + 1e-12:
        return True
    if abs(auc - best["cv_auc"]) <= 1e-12 and (lam > best["lam"] or (lam == best["lam"] and mh < best["mh"])):
        return True
    return False


def select(X_by_mh: dict[int, np.ndarray], y_by_mh: dict[int, np.ndarray], hour_ms: np.ndarray) -> tuple[dict, list[dict]]:
    """Pick minute and lambda on 2024 folds. hour_ms must lie entirely inside 2024."""
    folds = expanding_folds(hour_ms)
    best: dict | None = None
    table = []
    for mh in MH_GRID:
        X, y = X_by_mh[mh], y_by_mh[mh]
        for lam in LAM_GRID:
            aucs = []
            for tr, va in folds:
                if int(tr.sum()) < 200 or int(va.sum()) < 200:
                    continue
                ytr, yva = y[tr], y[va]
                if int((ytr == 1).sum()) < 50 or int((ytr == 0).sum()) < 50:
                    continue
                if int((yva == 1).sum()) < 50 or int((yva == 0).sum()) < 50:
                    continue
                mu, sd = zstats(X[tr])
                w = fit_logistic(apply_z(X[tr], mu, sd), ytr, lam)
                score = linear_score(X[va], w, mu, sd)
                aucs.append(auc_score(score, yva))
            row = {"mh": mh, "lam": lam, "folds": len(aucs), "cv_auc": None if len(aucs) < 2 else float(np.mean(aucs))}
            table.append(row)
            if row["cv_auc"] is not None and better(row["cv_auc"], lam, mh, best):
                best = row
    if best is None:
        raise RuntimeError("no 2024 cell had two usable folds")
    return best, table


def refit(X: np.ndarray, y: np.ndarray, lam: float) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    mu, sd = zstats(X)
    w = fit_logistic(apply_z(X, mu, sd), y, lam)
    return w, mu, sd


def slip_cost_bps(atr_price: np.ndarray, entry: np.ndarray) -> np.ndarray:
    """0.10R of a 0.5 daily-ATR stop, in bps of the decision price."""
    return STOP_ATR * SLIP_R * atr_price / entry * 1e4


def trade_metrics(score: np.ndarray, z: np.ndarray, ret: np.ndarray, day: np.ndarray, cost: np.ndarray) -> dict:
    pos = np.sign(score)
    ok = np.isfinite(score) & np.isfinite(z) & np.isfinite(ret) & np.isfinite(cost) & (pos != 0) & (z != 0)
    score, z, ret, day, cost, pos = score[ok], z[ok], ret[ok], day[ok], cost[ok], pos[ok]
    gross = pos * ret
    net = gross - cost
    net5 = gross - SPREAD_BPS
    rho, band = rho_band(score, z)
    g_m, g_lo, g_hi, _ = patterns.boot(gross, day)
    n_m, n_lo, n_hi, _ = patterns.boot(net, day)
    return {
        "n": int(ok.sum()),
        "auc": round(auc_score(score, (z > 0).astype(float)), 4),
        "hit": round(float((gross > 0).mean()), 4),
        "base_up": round(float((ret > 0).mean()), 4),
        "rho": round(rho, 4),
        "band": round(band, 4),
        "clears_band": bool(abs(rho) > band),
        "gross_bps": round(float(gross.mean()), 2),
        "gross_lo": round(g_lo, 2),
        "gross_hi": round(g_hi, 2),
        "net_slip_bps": round(float(net.mean()), 2),
        "net_slip_lo": round(n_lo, 2),
        "net_slip_hi": round(n_hi, 2),
        "net_5_bps": round(float(net5.mean()), 2),
        "mean_cost_bps": round(float(cost.mean()), 2),
        "boot_mean_gross": round(g_m, 2),
        "boot_mean_net": round(n_m, 2),
    }


def _wilder(tr: np.ndarray, n: int) -> np.ndarray:
    out = np.full(tr.size, np.nan)
    if tr.size < n:
        return out
    a = float(tr[:n].mean())
    out[n - 1] = a
    for i in range(n, tr.size):
        a = (a * (n - 1) + tr[i]) / n
        out[i] = a
    return out


def previous_day_atr(m: dict) -> dict[int, float]:
    """Wilder-20 ATR in price units, keyed by the day that may use it (the day after the ATR's last bar)."""
    day = m["t"] // 86_400_000
    ud, start, cnt = np.unique(day, return_index=True, return_counts=True)
    hh = np.maximum.reduceat(m["h"].astype(float), start)
    ll = np.minimum.reduceat(m["l"].astype(float), start)
    cc = m["c"][start + cnt - 1].astype(float)
    prev_c = np.concatenate([[cc[0]], cc[:-1]])
    tr = np.maximum(hh - ll, np.maximum(np.abs(hh - prev_c), np.abs(ll - prev_c)))
    atr = _wilder(tr, ATR_N)
    return {int(ud[i]): float(atr[i - 1]) for i in range(1, ud.size) if np.isfinite(atr[i - 1])}


def features_at(nu: dict, idx: np.ndarray) -> np.ndarray:
    tfs = list(N.TFS)
    cols = [nu["bal"][idx].astype(float), nu["slope_bal"][idx].astype(float)]
    cols += [nu[f"{tf}.bal"][idx].astype(float) for tf in tfs]
    for tf in tfs:
        sl = np.mean([nu[f"{tf}.{n}.slope"][idx].astype(float) for n in N.NS], axis=0)
        cols.append(sl)
    cols += [nu[f"{tf}.{n}.pos"][idx].astype(float) for tf in tfs for n in N.NS]
    cols += [nu[f"{tf}.cloud"][idx].astype(float) for tf in tfs]
    X = np.column_stack(cols)
    if X.shape[1] != len(feature_names()):
        raise RuntimeError("feature width drifted from the pre-registered list")
    return X


def build_packs(nu: dict, atr: dict[int, float]) -> dict[str, dict]:
    hz = T.hours(nu)
    rows = hz["rows"]
    close_i = rows[:, 0] + 59
    if not np.array_equal(rows[:, 58] + 1, close_i):
        raise RuntimeError("hour rows are not contiguous")
    px = nu["c"][rows].astype(float)
    hour_close = nu["c"][close_i].astype(float)
    ret = (hour_close[:, None] / px - 1.0) * 1e4
    uh = hz["hour"] // 3_600_000
    nxt = np.searchsorted(uh, uh + 1)
    has = (nxt < uh.size) & (uh[np.clip(nxt, 0, uh.size - 1)] == uh + 1)
    next_open = np.where(has, hz["hour"][np.clip(nxt, 0, uh.size - 1)], -1)
    nclose = np.where(has, hour_close[np.clip(nxt, 0, uh.size - 1)], np.nan)
    ret_n = (nclose[:, None] / px - 1.0) * 1e4
    hour = hz["hour"]
    keep = (hour >= TRAIN_START) & (hour < TEST_END)
    packs = {}
    for name, zmat, rmat, exit_of in (
        ("rest", hz["z"], ret, lambda h, _n: h + 59 * 60_000),
        ("next", hz["zn"], ret_n, lambda _h, n: n + 59 * 60_000),
    ):
        ok = keep.copy()
        for mh in MH_GRID:
            idx = rows[:, mh]
            finite = np.isfinite(zmat[:, mh]) & np.isfinite(rmat[:, mh]) & nu["valid"][idx]
            if name == "next":
                finite &= has
            ok &= finite
        day = nu["t"][rows[ok, 0]] // 86_400_000
        atr_px = np.array([atr.get(int(d), np.nan) for d in day])
        ok_i = np.nonzero(ok)[0]
        block = {"hour": hour[ok], "day": day, "atr": atr_px, "mh": {}}
        for mh in MH_GRID:
            idx = rows[ok, mh]
            entry = px[ok, mh]
            cost = slip_cost_bps(atr_px, entry)
            good = np.isfinite(cost)
            block["mh"][mh] = {
                "X": features_at(nu, idx),
                "y": (zmat[ok, mh] > 0).astype(float),
                "z": zmat[ok, mh],
                "ret": rmat[ok, mh],
                "bal": nu["bal"][idx].astype(float),
                "cost": cost,
                "good": good,
            }
        exits = exit_of(hour[ok], next_open[ok])
        block["exit"] = exits
        block["index"] = ok_i
        packs[name] = block
    return packs


def window_mask(block: dict, t0: int, t1: int, exit_before: int | None = None) -> np.ndarray:
    m = (block["hour"] >= t0) & (block["hour"] < t1)
    if exit_before is not None:
        m &= block["exit"] < exit_before
    for mh in MH_GRID:
        m &= block["mh"][mh]["good"]
    return m


def run_target(block: dict, name: str) -> dict:
    train_m = window_mask(block, TRAIN_START, Y2025, exit_before=Y2025)
    if train_m.sum() == 0 or int(block["hour"][train_m].max()) >= Y2025:
        raise RuntimeError(f"{name} training window is empty or reaches 2025")
    X_by = {mh: block["mh"][mh]["X"][train_m] for mh in MH_GRID}
    y_by = {mh: block["mh"][mh]["y"][train_m] for mh in MH_GRID}
    chosen, table = select(X_by, y_by, block["hour"][train_m])
    mh, lam = int(chosen["mh"]), float(chosen["lam"])
    w, mu, sd = refit(X_by[mh], y_by[mh], lam)
    frozen = {
        "target": name,
        "mh": mh,
        "lam": lam,
        "cv_auc": round(chosen["cv_auc"], 4),
        "train_n": int(train_m.sum()),
        "train_last": int(block["hour"][train_m].max()),
        "grid": [{**r, "cv_auc": None if r["cv_auc"] is None else round(r["cv_auc"], 4)} for r in table],
        "coef": [round(float(c), 4) for c in w],
    }
    print(f"FROZEN {name}: minute {mh}, lambda {lam}, 2024 CV AUC {frozen['cv_auc']}, train hours {frozen['train_n']}", flush=True)

    def score_window(t0: int, t1: int) -> dict:
        m = window_mask(block, t0, t1)
        col = block["mh"][mh]
        learned = linear_score(col["X"][m], w, mu, sd)
        fade = -col["bal"][m]
        fade0 = -block["mh"][0]["bal"][m]
        day, z, ret, cost = block["day"][m], col["z"][m], col["ret"][m], col["cost"][m]
        z0, ret0, cost0 = block["mh"][0]["z"][m], block["mh"][0]["ret"][m], block["mh"][0]["cost"][m]
        return {
            "learned": trade_metrics(learned, z, ret, day, cost),
            "fade_same_minute": trade_metrics(fade, z, ret, day, cost),
            "fade_minute_0": trade_metrics(fade0, z0, ret0, day, cost0),
        }

    return {"frozen": frozen, "y2025": score_window(Y2025, Y2026), "y2026": score_window(Y2026, TEST_END)}


def main() -> dict:
    m = N.load_m1()
    atr = previous_day_atr(m)
    print(f"minutes {m['t'].size}", flush=True)
    nu = N.nucleus(m)
    del m
    packs = build_packs(nu, atr)
    del nu
    out = {"features": feature_names(), "rest": run_target(packs["rest"], "rest"), "next": run_target(packs["next"], "next")}
    path = Path(__file__).resolve().parent.parent / "out" / "lab" / "walkforward.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(out))
    print(f"wrote {path}", flush=True)
    return out


if __name__ == "__main__":
    main()
