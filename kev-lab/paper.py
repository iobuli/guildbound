"""Paper broker: one account in dollars, risk sized from current equity, P&L booked at each trade's exit time."""
from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

KEY = "S0.5_T2"
DAY = 86_400_000


@dataclass
class Book:
    start: float = 100_000.0
    risk: float = 0.005  # fraction of equity risked per trade
    max_open_risk: float = 0.015  # cap on total risk across open positions
    equity: float = field(init=False)
    ledger: list = field(default_factory=list)
    skipped: dict = field(default_factory=lambda: {"market busy": 0, "risk cap": 0})

    def __post_init__(self):
        self.equity = self.start

    def run(self, rows: list[dict]) -> "Book":
        """rows: signal rows (dicts with mkt, t, dir, fill, r[KEY], xt[KEY]) in any order."""
        rows = sorted((r for r in rows if KEY in r["r"]), key=lambda r: r["t"])
        open_pos: list[dict] = []
        for r in rows:
            # close everything that exited before this fill
            for p in sorted([p for p in open_pos if p["exit_t"] <= r["t"]], key=lambda p: p["exit_t"]):
                self._close(p)
                open_pos.remove(p)
            if any(p["mkt"] == r["mkt"] for p in open_pos):
                self.skipped["market busy"] += 1
                continue
            risk_usd = self.equity * self.risk
            if sum(p["risk_usd"] for p in open_pos) + risk_usd > self.equity * self.max_open_risk:
                self.skipped["risk cap"] += 1
                continue
            open_pos.append({"mkt": r["mkt"], "t": r["t"], "exit_t": r["xt"][KEY], "dir": r["dir"], "fill": r["fill"], "R": r["r"][KEY], "risk_usd": risk_usd})
        for p in sorted(open_pos, key=lambda p: p["exit_t"]):
            self._close(p)
        return self

    def _close(self, p: dict):
        pnl = p["risk_usd"] * p["R"]
        self.equity += pnl
        self.ledger.append({**p, "pnl": pnl, "equity": self.equity})

    def daily(self, t0: int, t1: int) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Calendar-day equity (closing), daily returns and day timestamps from t0 to t1 (ms)."""
        days = np.arange(t0 // DAY * DAY, t1 // DAY * DAY + DAY, DAY)
        pnl = np.zeros(days.size)
        for x in self.ledger:
            k = int((x["exit_t"] // DAY * DAY - days[0]) // DAY)
            if 0 <= k < days.size:
                pnl[k] += x["pnl"]
        eq = self.start + np.cumsum(pnl)
        prev = np.concatenate([[self.start], eq[:-1]])
        return eq, pnl / prev, days
