"""Frozen rules. NO tradable directional pattern survived confirmation (see FINDINGS.md).
signal_null always returns 0. vol_compression_flag_usdjpy is a NON-directional volatility-state flag (not a +/-1 signal)."""
import numpy as np, pandas as pd
TIMEFRAME = "H1"
HORIZON_BARS = 24
EXIT_RULE = "n/a (no directional claim); vol flag horizon = next 24 H1 bars"

def signal_null(d, t):
    return 0

def vol_compression_flag_usdjpy(d, t):
    """True if 12-bar log-return std < 0.4 * 120-bar std at the close of the last H1 bar closed by t.
    Finding: next-24h range (in units of ATR14*sqrt(24)) is larger than average (vol mean reversion). Says nothing about direction."""
    h1 = d["USDJPY_H1"]
    h1 = h1[h1.index + pd.Timedelta("1h") <= t].iloc[-200:]
    r = np.log(h1.close).diff()
    return bool(r.iloc[-12:].std() / r.iloc[-120:].std() < 0.4)
