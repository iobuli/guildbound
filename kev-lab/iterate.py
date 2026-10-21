"""Iteration 1 (kev-brain/experiments/2026-10-21-walk-forward.md): each year, per market, pick a (filter, exit) from a fixed menu
using only earlier fills, scored by deflated Sharpe; sit out when nothing clears the bar. Same choices applied to the placebo orders."""
from __future__ import annotations

import datetime as dt

import numpy as np

import quant
from paper import KEY, Book


def agrees(r, field):
    return (r["dir"] == 1 and r[field] in ("up", "long")) or (r["dir"] == -1 and r[field] in ("down", "short"))


FILTERS = {
    "all": lambda r: True,
    "space": lambda r: r["sp"] == 1,
    "GO": lambda r: r["go"] == 1,
    "trend agrees": lambda r: agrees(r, "trendPct"),
    "bias agrees": lambda r: agrees(r, "htf"),
    "space+trend": lambda r: r["sp"] == 1 and agrees(r, "trendPct"),
    "space+bias": lambda r: r["sp"] == 1 and agrees(r, "htf"),
    "GO+trend": lambda r: r["go"] == 1 and agrees(r, "trendPct"),
    "low vol": lambda r: r["vol"] < 0.5,
    "high vol": lambda r: r["vol"] >= 0.5,
}
EXITS = ["S0.5_T1", "S0.5_T2", "S0.5_T4", "S1_T2", "S1_T4"]
YEARS = range(2021, 2027)
MIN_N = 40


def ms(y: int) -> int:
    return int(dt.datetime(y, 1, 1, tzinfo=dt.timezone.utc).timestamp() * 1000)


def per_trade_sr(r: np.ndarray) -> float:
    return float(r.mean() / r.std(ddof=1)) if r.size > 2 and r.std(ddof=1) > 0 else 0.0


def choose(rows: list[dict], before: int) -> dict:
    """Score every config on rows filled before `before` (exit also before it); return the best and its DSR."""
    train = [r for r in rows if r["t"] < before]
    scored = []
    for fname, f in FILTERS.items():
        sel = [r for r in train if f(r)]
        for ex in EXITS:
            rs = np.array([r["r"][ex] for r in sel if ex in r["r"] and r["xt"][ex] < before])
            if rs.size >= MIN_N:
                scored.append((fname, ex, rs))
    if not scored:
        return {"filter": None, "exit": None, "dsr": 0.0, "n": 0, "meanR": float("nan"), "trials": 0}
    srs = [per_trade_sr(x[2]) for x in scored]
    sr0 = quant.expected_max_sr(srs)
    best = max(scored, key=lambda x: quant.psr(x[2], sr0))
    return {"filter": best[0], "exit": best[1], "dsr": quant.psr(best[2], sr0), "n": int(best[2].size), "meanR": float(best[2].mean()), "trials": len(scored)}


def rekey(rows, ex):
    return [{**r, "r": {KEY: r["r"][ex]}, "xt": {KEY: r["xt"][ex]}} for r in rows if ex in r["r"]]


def walk_forward(sig: dict[str, dict], threshold: float) -> dict:
    plan, real_rows, plc_rows = [], [], []
    for mkt, s in sig.items():
        for y in YEARS:
            c = choose(s["trades"], ms(y))
            trade = c["filter"] is not None and c["dsr"] >= threshold
            plan.append({"mkt": mkt, "year": y, **c, "trade": trade})
            if trade:
                f, a, b = FILTERS[c["filter"]], ms(y), ms(y + 1)
                real_rows += rekey([r for r in s["trades"] if a <= r["t"] < b and f(r)], c["exit"])
                plc_rows += rekey([r for r in s["placebo"] if a <= r["t"] < b and f(r)], c["exit"])
    last = max(s["lastBar"] for s in sig.values())
    out = {"threshold": threshold, "plan": plan}
    for name, rows in (("real", real_rows), ("placebo", plc_rows)):
        b = Book().run(rows)
        eq, ret, days = b.daily(ms(YEARS[0]), last)
        rs = np.array([x["R"] for x in b.ledger])
        out[name] = {**quant.metrics(ret, eq, (days[-1] - days[0]) / 86_400_000), "trades": len(b.ledger), "final": float(eq[-1]),
                     "meanR": float(rs.mean()) if rs.size else float("nan"), "curve": [float(x) for x in eq[:: max(1, eq.size // 400)]]}
        if name == "real":
            out["days"] = [int(x) for x in days[:: max(1, days.size // 400)]]
            out["ledger"] = [{k: v for k, v in x.items()} for x in b.ledger]
    return out


def live_choice(sig: dict[str, dict], threshold: float) -> dict:
    """What the process would trade from now on, chosen with every closed fill up to today."""
    return {mkt: {**(c := choose(s["trades"], s["lastBar"] + 1)), "trade": c["filter"] is not None and c["dsr"] >= threshold} for mkt, s in sig.items()}
