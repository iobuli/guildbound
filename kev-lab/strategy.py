"""The four pre-registered candidates (kev-brain/experiments/2026-10-21-paper-book.md), selection by deflated Sharpe, and the frozen forward run."""
from __future__ import annotations

import math

import numpy as np

import quant
from paper import KEY, Book

CANDIDATES = {
    "A every fill": lambda r: True,
    "B space ok": lambda r: r["sp"] == 1,
    "C space ok + trend agrees": lambda r: r["sp"] == 1 and ((r["dir"] == 1 and r["trendPct"] == "up") or (r["dir"] == -1 and r["trendPct"] == "down")),
    "D GO": lambda r: r["go"] == 1,
}
DESIGN = (1577836800000, 1672531200000)  # 2020-01-01 .. 2023-01-01 (exclusive)
FORWARD_FROM = 1672531200000


def book(rows, t0, t1, rule, risk=0.005):
    sel = [r for r in rows if t0 <= r["t"] < t1 and rule(r)]
    b = Book(risk=risk).run(sel)
    eq, ret, days = b.daily(t0, min(t1, max([r["xt"][KEY] for r in sel], default=t0)))
    return b, sel, eq, ret, days


def summarise(b, sel, eq, ret, days, trials=None):
    rs = np.array([x["R"] for x in b.ledger])
    span = (days[-1] - days[0]) / 86_400_000 if days.size > 1 else 1
    m = quant.metrics(ret, eq, span, trials)
    m.update({"signals": len(sel), "trades": len(b.ledger), "skipped": dict(b.skipped), "final": float(eq[-1]) if eq.size else b.start,
              "meanR": float(rs.mean()) if rs.size else float("nan"), "win": float((rs > 0).mean()) if rs.size else float("nan"),
              "kelly": quant.kelly_fraction(rs)})
    return m


def run_all(sig: dict[str, dict]) -> dict:
    real = [r for s in sig.values() for r in s["trades"]]
    plc = [r for s in sig.values() for r in s["placebo"]]
    last = max(s["lastBar"] for s in sig.values())
    # 1. design window: every candidate, with the trial set for deflation
    design = {name: book(real, *DESIGN, rule) for name, rule in CANDIDATES.items()}
    trials = [d[3] for d in design.values()]
    table = {name: summarise(*d, trials=trials) for name, d in design.items()}
    chosen = max(table, key=lambda k: (table[k]["dsr"], table[k]["sharpe"]))
    # 2. forward: the chosen rule, frozen, once; plus the same rule on the matched random orders
    rule = CANDIDATES[chosen]
    f = book(real, FORWARD_FROM, last + 1, rule)
    p = book(plc, FORWARD_FROM, last + 1, rule)
    fwd, pl = summarise(*f), summarise(*p)
    mc = quant.stationary_bootstrap(f[3], 100_000.0, paths=600)
    per_mkt = {}
    for mkt in sig:
        xs = [x for x in f[0].ledger if x["mkt"] == mkt]
        per_mkt[mkt] = {"trades": len(xs), "pnl": float(sum(x["pnl"] for x in xs)), "meanR": float(np.mean([x["R"] for x in xs])) if xs else float("nan")}
    return {
        "candidates": table, "chosen": chosen, "forward": fwd, "placebo": pl, "montecarlo": mc, "per_market": per_mkt,
        "curves": {"days": [int(x) for x in f[4]], "real": [float(x) for x in f[2]], "placebo": [float(x) for x in np.interp(f[4], p[4], p[2])] if p[4].size else []},
        "ledger": [{k: (float(v) if isinstance(v, (np.floating,)) else v) for k, v in x.items()} for x in f[0].ledger],
        "last_bar": last,
    }
