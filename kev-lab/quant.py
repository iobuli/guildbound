"""Quant toolkit for kev-lab: risk metrics, overfitting-aware Sharpe tests, bootstrap, regime statistics and sizing.

Everything works on plain numpy arrays. Formulas follow the cited papers; each has a test in tests/test_quant.py.
"""
from __future__ import annotations

import math
from statistics import NormalDist

import numpy as np

N01 = NormalDist()
EULER = 0.5772156649015329


# ---------------------------------------------------------------- risk metrics
def sharpe(daily: np.ndarray, periods: int = 365) -> float:
    """Annualised Sharpe of daily returns (crypto and this book trade every day, so 365)."""
    d = np.asarray(daily, float)
    s = d.std(ddof=1) if d.size > 1 else 0.0
    return float(d.mean() / s * math.sqrt(periods)) if s > 0 else 0.0


def sortino(daily: np.ndarray, periods: int = 365) -> float:
    d = np.asarray(daily, float)
    down = np.sqrt(np.mean(np.minimum(d, 0.0) ** 2)) if d.size else 0.0
    return float(d.mean() / down * math.sqrt(periods)) if down > 0 else 0.0


def drawdown(equity: np.ndarray) -> tuple[float, int, np.ndarray]:
    """Maximum drawdown (fraction), its longest duration in periods, and the drawdown series."""
    e = np.asarray(equity, float)
    peak = np.maximum.accumulate(e)
    dd = e / peak - 1.0
    longest, run = 0, 0
    for x in dd:
        run = run + 1 if x < 0 else 0
        longest = max(longest, run)
    return float(dd.min()) if dd.size else 0.0, longest, dd


def ulcer(equity: np.ndarray) -> float:
    """Ulcer index: root-mean-square drawdown in percent (Martin 1987)."""
    _, _, dd = drawdown(equity)
    return float(math.sqrt(np.mean((100 * dd) ** 2))) if dd.size else 0.0


def cagr(equity: np.ndarray, days: float) -> float:
    e = np.asarray(equity, float)
    return float((e[-1] / e[0]) ** (365.0 / days) - 1.0) if e.size > 1 and days > 0 and e[0] > 0 and e[-1] > 0 else 0.0


# ------------------------------------------------- overfitting-aware Sharpe tests
def psr(daily: np.ndarray, sr_benchmark: float = 0.0) -> float:
    """Probabilistic Sharpe Ratio (Bailey & Lopez de Prado 2012): P(true per-period SR > benchmark), allowing for skew and fat tails."""
    d = np.asarray(daily, float)
    n = d.size
    if n < 3 or d.std(ddof=1) == 0:
        return 0.0
    sr = d.mean() / d.std(ddof=1)
    m = d - d.mean()
    s2 = np.mean(m**2)
    skew = np.mean(m**3) / s2**1.5
    kurt = np.mean(m**4) / s2**2
    denom = math.sqrt(max(1e-12, 1 - skew * sr + (kurt - 1) / 4 * sr**2))
    return float(N01.cdf((sr - sr_benchmark) * math.sqrt(n - 1) / denom))


def expected_max_sr(trial_srs: list[float]) -> float:
    """Expected maximum per-period Sharpe among N unskilled trials with the observed spread of trial Sharpes (Bailey & Lopez de Prado 2014)."""
    n = len(trial_srs)
    if n < 2:
        return 0.0
    v = float(np.var(trial_srs, ddof=1))
    return math.sqrt(v) * ((1 - EULER) * N01.inv_cdf(1 - 1 / n) + EULER * N01.inv_cdf(1 - 1 / (n * math.e)))


def dsr(daily: np.ndarray, trial_daily: list[np.ndarray]) -> float:
    """Deflated Sharpe Ratio: PSR against the Sharpe that the best of these trials would show by luck alone."""
    srs = [float(np.mean(t) / np.std(t, ddof=1)) if np.size(t) > 2 and np.std(t, ddof=1) > 0 else 0.0 for t in trial_daily]
    return psr(daily, expected_max_sr(srs))


# ----------------------------------------------------------------- bootstrap
def stationary_bootstrap(daily: np.ndarray, start: float, paths: int = 1000, mean_block: int = 10, seed: int = 7) -> dict:
    """Politis-Romano stationary bootstrap of daily returns: equity fan, drawdown distribution and probability of ending below start."""
    rng = np.random.default_rng(seed)
    d = np.asarray(daily, float)
    n = d.size
    if n < 5:
        return {"p_loss": float("nan"), "dd_q": [], "fan": []}
    out = np.empty((paths, n))
    p = 1.0 / mean_block
    for k in range(paths):
        i = rng.integers(n)
        for j in range(n):
            out[k, j] = d[i]
            i = rng.integers(n) if rng.random() < p else (i + 1) % n
    eq = start * np.cumprod(1 + out, axis=1)
    dds = np.array([drawdown(e)[0] for e in eq])
    q = np.quantile(eq, [0.05, 0.25, 0.5, 0.75, 0.95], axis=0)
    step = max(1, n // 300)
    return {
        "p_loss": float(np.mean(eq[:, -1] < start)),
        "final_q": [float(x) for x in np.quantile(eq[:, -1], [0.05, 0.5, 0.95])],
        "dd_q": [float(x) for x in np.quantile(dds, [0.05, 0.5, 0.95])],
        "fan": [[float(v) for v in row[::step]] for row in q],
    }


# --------------------------------------------------------- regime statistics
def hurst(logret: np.ndarray, min_chunk: int = 16) -> float:
    """Hurst exponent by rescaled range: ~0.5 random walk, >0.5 trending, <0.5 mean-reverting."""
    x = np.asarray(logret, float)
    sizes, rs = [], []
    size = min_chunk
    while size <= x.size // 2:
        vals = []
        for k in range(0, x.size - size + 1, size):
            c = x[k : k + size]
            z = np.cumsum(c - c.mean())
            s = c.std()
            if s > 0:
                vals.append((z.max() - z.min()) / s)
        if vals:
            sizes.append(size)
            rs.append(np.mean(vals))
        size *= 2
    if len(sizes) < 2:
        return float("nan")
    return float(np.polyfit(np.log(sizes), np.log(rs), 1)[0])


def variance_ratio(logret: np.ndarray, k: int = 24) -> tuple[float, float]:
    """Lo-MacKinlay variance ratio VR(k) and its homoskedastic z-statistic: VR > 1 trending, < 1 mean-reverting."""
    x = np.asarray(logret, float)
    n = x.size
    if n < 2 * k:
        return float("nan"), float("nan")
    mu = x.mean()
    v1 = np.sum((x - mu) ** 2) / (n - 1)
    sk = np.convolve(x, np.ones(k), "valid")
    vk = np.sum((sk - k * mu) ** 2) / (k * (n - k + 1) * (1 - k / n))
    vr = vk / v1
    z = (vr - 1) / math.sqrt(2 * (2 * k - 1) * (k - 1) / (3 * k * n))
    return float(vr), float(z)


# ---------------------------------------------------------------------- sizing
def kelly_fraction(r_multiples: np.ndarray) -> float:
    """Continuous-approximation Kelly fraction of equity to risk per trade for a stream of R-multiples: mean / E[R^2] (0 if no edge)."""
    r = np.asarray(r_multiples, float)
    if r.size < 10 or r.mean() <= 0:
        return 0.0
    return float(r.mean() / np.mean(r**2))


def metrics(daily: np.ndarray, equity: np.ndarray, days: float, trial_daily: list[np.ndarray] | None = None) -> dict:
    mdd, dur, _ = drawdown(equity)
    c = cagr(equity, days)
    out = {
        "sharpe": sharpe(daily), "sortino": sortino(daily), "max_dd": mdd, "dd_days": dur, "cagr": c,
        "calmar": c / abs(mdd) if mdd < 0 else 0.0, "ulcer": ulcer(equity), "psr": psr(daily),
    }
    if trial_daily is not None:
        out["dsr"] = dsr(daily, trial_daily)
    return out
