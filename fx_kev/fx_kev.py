"""FX trading with a System One decision model (Kev self-hosted, or Jev hosted).

The model never sees raw prices as a time series. Each bar is rendered as a short text "state"
(trend, momentum, volatility, session), and the model answers typed questions about it:
  direction : Choice(long | short | flat)  -> probability per option
  tradeable : Noul(...)                    -> p(yes)
We trade only when the model's probability clears a threshold, then backtest with spread costs.

Usage:
  python fx_kev.py --csv EURUSD_H1.csv --backend rules            # offline smoke test, no model
  python fx_kev.py --synthetic --backend rules
  python fx_kev.py --csv EURUSD_H1.csv --backend kev --base-url http://127.0.0.1:8008
  python fx_kev.py --csv EURUSD_H1.csv --backend jev --api-key $TYPESAFE_API_KEY   (base_url per TypeSafe docs)

CSV columns: time,open,high,low,close (case-insensitive).
"""
import argparse

import numpy as np
import pandas as pd

OPTIONS = {
    "long": "Price is more likely to rise over the next few bars",
    "short": "Price is more likely to fall over the next few bars",
    "flat": "No clear edge; direction is unclear",
}


# ---------- data ----------
def load_csv(path):
    df = pd.read_csv(path)
    df.columns = [c.lower() for c in df.columns]
    tcol = next(c for c in ("time", "date", "datetime", "timestamp") if c in df.columns)
    df[tcol] = pd.to_datetime(df[tcol])
    return df.rename(columns={tcol: "time"}).set_index("time").sort_index()[["open", "high", "low", "close"]]


def synthetic(n=4000, seed=0):
    rng = np.random.default_rng(seed)
    ret = rng.normal(0, 0.0004, n) + 0.00008 * np.sin(np.arange(n) / 200)  # weak slow cycle
    close = 1.10 * np.exp(np.cumsum(ret))
    spread = np.abs(rng.normal(0, 0.0002, n))
    idx = pd.date_range("2024-01-01", periods=n, freq="h")
    return pd.DataFrame({"open": np.r_[close[0], close[:-1]], "high": close + spread, "low": close - spread, "close": close}, index=idx)


# ---------- features -> text state ----------
def add_features(df):
    c = df["close"]
    d = c.diff()
    up, dn = d.clip(lower=0).rolling(14).mean(), (-d.clip(upper=0)).rolling(14).mean()
    tr = pd.concat([df.high - df.low, (df.high - c.shift()).abs(), (df.low - c.shift()).abs()], axis=1).max(axis=1)
    f = pd.DataFrame(index=df.index)
    f["rsi"] = 100 - 100 / (1 + up / dn.replace(0, np.nan))
    f["atr"] = tr.rolling(14).mean()
    f["ret_5"] = c.pct_change(5) * 1e4          # pips-ish (bp)
    f["ret_20"] = c.pct_change(20) * 1e4
    f["dist_ma50_atr"] = (c - c.rolling(50).mean()) / f["atr"]
    f["atr_pct_rank"] = f["atr"].rolling(200).rank(pct=True)
    return f.dropna()


def make_state(ts, row):
    sess = "Asia" if ts.hour < 7 else "London" if ts.hour < 13 else "New York" if ts.hour < 21 else "late"
    return {
        "instrument": "EUR/USD hourly bar",
        "session": f"{sess}, {ts.day_name()}",
        "trend": f"price is {row.dist_ma50_atr:+.1f} ATR from its 50-bar average",
        "momentum": f"5-bar return {row.ret_5:+.1f} bp, 20-bar return {row.ret_20:+.1f} bp, RSI(14) {row.rsi:.0f}",
        "volatility": f"ATR is at the {row.atr_pct_rank * 100:.0f}th percentile of the last 200 bars",
    }


# ---------- decision backends ----------
class RulesModel:
    """Offline stand-in with the same interface: a naive trend/RSI heuristic. Not a real model; for plumbing tests."""
    def decide(self, row, state):
        score = np.tanh(row.dist_ma50_atr / 2) - (row.rsi - 50) / 100
        pl, ps = 1 / (1 + np.exp(-3 * score)) * 0.8, 1 / (1 + np.exp(3 * score)) * 0.8
        return {"long": pl, "short": ps, "flat": max(1 - pl - ps, 0.0)}, 0.5 + 0.4 * abs(score)


class SystemOneModel:
    def __init__(self, base_url, api_key, model):
        from typesafe_sdk import Choice, Noul, TypeSafeClient
        kw = {"base_url": base_url} if base_url else {}
        self.client = TypeSafeClient(api_key=api_key, model=model, **kw)
        self.questions = {
            "direction": Choice(instructions="Given this market snapshot, where is price most likely to go over the next 4 hours?", criteria=OPTIONS),
            "tradeable": Noul(instructions="Is the setup clean enough (not choppy, volatility adequate) to risk capital on?"),
        }

    def decide(self, row, state):
        r = self.client.system_one(state=state, questions=self.questions)
        return r.choices["direction"].probabilities, r.nouls["tradeable"].noul


def get_model(a):
    if a.backend == "rules":
        return RulesModel()
    return SystemOneModel(a.base_url if a.backend == "kev" else a.base_url or None, a.api_key, "kev-latest" if a.backend == "kev" else "jev-latest")


# ---------- backtest ----------
def backtest(df, feats, model, thr, ok_thr, hold, spread_pips, step):
    pip = 1e-4
    c = df["close"]
    trades, last_exit = [], -1
    idxs = list(range(0, len(feats) - hold - 1, step))
    for i in idxs:
        ts, row = feats.index[i], feats.iloc[i]
        pos = df.index.get_loc(ts)
        if pos <= last_exit:
            continue
        probs, p_ok = model.decide(row, make_state(ts, row))
        side, p = max(("long", probs.get("long", 0)), ("short", probs.get("short", 0)), key=lambda x: x[1])
        if p < thr or p_ok < ok_thr:
            continue
        entry, exit_ = c.iloc[pos], c.iloc[pos + hold]
        pnl = ((exit_ - entry) if side == "long" else (entry - exit_)) / pip - spread_pips
        trades.append((ts, side, p, pnl))
        last_exit = pos + hold
    return pd.DataFrame(trades, columns=["time", "side", "p", "pips"])


def report(t, name):
    if t.empty:
        print(f"{name}: no trades"); return
    eq = t.pips.cumsum()
    sharpe = t.pips.mean() / t.pips.std() * np.sqrt(len(t)) if t.pips.std() > 0 else float("nan")
    print(f"{name}: n={len(t)} win={np.mean(t.pips > 0):.1%} avg={t.pips.mean():+.2f}p total={eq.iloc[-1]:+.0f}p "
          f"maxDD={(eq.cummax() - eq).max():.0f}p trade-sharpe={sharpe:.2f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--csv"); ap.add_argument("--synthetic", action="store_true")
    ap.add_argument("--backend", choices=["rules", "kev", "jev"], default="rules")
    ap.add_argument("--base-url", default="http://127.0.0.1:8008"); ap.add_argument("--api-key", default="local")
    ap.add_argument("--thr", type=float, default=0.55, help="min probability of long/short")
    ap.add_argument("--ok-thr", type=float, default=0.5, help="min p(tradeable)")
    ap.add_argument("--hold", type=int, default=4, help="bars to hold")
    ap.add_argument("--spread", type=float, default=0.8, help="round-trip cost in pips")
    ap.add_argument("--step", type=int, default=1, help="evaluate every Nth bar (saves model calls)")
    ap.add_argument("--test-frac", type=float, default=0.4, help="out-of-sample tail used for the reported result")
    a = ap.parse_args()
    df = synthetic() if a.synthetic or not a.csv else load_csv(a.csv)
    feats = add_features(df)
    cut = int(len(feats) * (1 - a.test_frac))
    model = get_model(a)
    # Pick the threshold on the first part only; report on the untouched tail.
    best = max((backtest(df, feats.iloc[:cut], model, t, a.ok_thr, a.hold, a.spread, a.step).pips.sum(), t) for t in (0.5, 0.55, 0.6, 0.7))
    print(f"threshold chosen in-sample: {best[1]}")
    report(backtest(df, feats.iloc[:cut], model, best[1], a.ok_thr, a.hold, a.spread, a.step), "in-sample ")
    report(backtest(df, feats.iloc[cut:], model, best[1], a.ok_thr, a.hold, a.spread, a.step), "out-sample")


if __name__ == "__main__":
    main()
