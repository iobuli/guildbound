"""CAL1: month-end London-fix price-pressure reversal (fade the London session into the month-end 16:00 fix).

Clock (fixed assumption, checked only from timestamps, never from prices/returns): bar times are UTC. For USDJPY and
the metals the last bar of the trading week opens Fri 21:00 in Dec-Feb and Fri 20:00 in Jun-Aug (= 16:00 New York in
both regimes), which is what UTC stamps produce; SOLUSDT is Binance data (UTC). Bars are mapped to London local time
with the IANA tz database (Europe/London, DST-aware). The month-end date is a deterministic calendar fact (last Monday-
Friday of the calendar month), known in advance, so the rule is causal.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

PREREG = dict(
    id="CAL1",
    family="calendar flow: month-end London-fix price-pressure reversal",
    desc=(
        "Map bar open times (UTC) to London local time (Europe/London, DST-aware). Month-end day = the last Monday-Friday "
        "date of each calendar month by the London calendar (calendar rule only, holidays ignored: a London date d with "
        "weekday Mon-Fri is month-end if the next weekday after d falls in a different month). On each month-end day take "
        "the H1 bar that opens at 08:00 London (session open price O = its open) and the H1 bar that opens at 15:00 London "
        "(signal bar, it closes at 16:00 London = the WM/Refinitiv 4pm fix; its close C). Both bars must exist on that "
        "London date, otherwise no trade that month. Session move m = C - O. If m > 0 go SHORT, if m < 0 go LONG (fade the "
        "London-session move into the month-end fix); m == 0: no trade. At most one signal per month. Enter at the open of "
        "the next bar (16:00 London). Stop: ext = C - side*2.0*ATR14 with pad = 0, i.e. a fixed volatility stop 2.0 x ATR14 "
        "(H1 ATR14 including the signal bar, known at its close) from the signal close. No take-profit; time exit at the "
        "close of the 24th bar after entry (H=24). Same rule, clock and parameters on all four instruments; no filters, "
        "no weights."
    ),
    rationale=(
        "Month-end is the most predictable liquidity-demand window in USD markets: global portfolio managers rebalance "
        "currency hedges and multi-asset weights, and pay/collect cash, at the month-end London 4pm WM/Refinitiv fix, so "
        "large uninformed orders arrive at a known time. Dealers that take fix orders pre-position during the London "
        "session, pushing prices in the direction of the net flow; once the fix prints the demand is gone and the "
        "inventory-driven price pressure should partly revert (temporary price impact compensating liquidity providers: "
        "Grossman & Miller 1988; Campbell, Grossman & Wang 1993). This pattern is reported for FX around the London fix "
        "and is strongest at month-end (Melvin & Prins 2015, month-end equity-hedge rebalancing at the 4pm fix; Evans 2018, "
        "price behaviour around the WMR fix; Krohn, Mueller & Whelan 2024, dollar moves into fixes that reverse afterwards), "
        "and month-turn institutional cash needs move USD and asset prices (Etula, Rinne, Suominen & Vaittinen 2020, 'Dash "
        "for Cash'); the same pre-fix / post-fix reversal is documented at the Tokyo fix (Ito & Yamada 2017). Because the "
        "sign of the month-end flow changes month to month (it depends on that month's relative asset performance, which "
        "we do not observe), the rule infers it from the price itself: the London-session move into the fix is taken as "
        "the footprint of the flow and faded after the fix. All four instruments are USD-quoted, so USD-side month-end "
        "demand reaches each of them; the metals also have their LBMA benchmark auctions inside the window (silver 12:00, "
        "gold 15:00 London), which is where month-end fund rebalancing in metals executes; crypto reference rates used for "
        "fund valuation and futures settlement (CME CF, computed 15:00-16:00 London) put the crypto month-end flow in the "
        "same hour. The SOL link is the weakest part of the story and is stated as such. Parameters are canonical, not "
        "tuned: last business day of the month, London session open (08:00) to the 4pm fix, H=24 (protocol default: the "
        "first trading day of the new month), and a standard 2 x ATR volatility stop. The stop width was set wide on "
        "purpose so that round-trip costs are a small fraction of 1R (~0.03-0.05R on all four instruments), in light of "
        "the REPORT's cost diagnostic for 0.8-ATR sweep stops; it was not chosen from any result of this rule."
    ),
    novelty=(
        "None of the 24 tested methods (S1-S13, K1-K3, N1-N8) uses the calendar: they are all price-level wick-sweep "
        "fades/follows (previous day/week extremes, Asian range, fractal swings, round numbers, equal highs/lows, multi-day "
        "clusters) or Kev judgements of such sweeps; N8 used a clock window (Monday 00-06h) but still traded a sweep of the "
        "previous week's high/low. CAL1 has no level and no sweep: it trades once a month at a scheduled flow event (the "
        "month-end 4pm London fix), its direction comes from the session move into that event, and its stop is a 2-ATR "
        "volatility stop rather than 0.25 ATR beyond a wick. It is not an inversion of any failed method, and it differs "
        "from the batch's SESS1 (daily New York opening-range breakout, follow) in calendar, session, trigger and direction."
    ),
    data_informed=False,   # direction, window and horizon come from the cited flow literature and the calendar only;
                           # no price/return result of this rule or any variant was computed. Only the stop width was
                           # set with the REPORT's general cost diagnostic in mind (disclosed in the rationale).
    H=24,
    pad=0.0,
)

SESSION_OPEN_HOUR = 8    # London bar-open hour whose open is the session anchor (08:00 London)
FIX_BAR_HOUR = 15        # London bar-open hour of the signal bar (15:00-16:00 London, closes at the 4pm fix)
STOP_ATR = 2.0           # stop distance from the signal close, in ATR14 units


def _month_end_flags(lon):
    """True for bars whose London calendar date is the last Mon-Fri weekday of its month (pure calendar)."""
    d = lon.tz_localize(None).normalize()
    wd = np.asarray(d.dayofweek)
    step = np.where(wd == 4, 3, np.where(wd == 5, 2, 1))          # days to the next Mon-Fri weekday
    nxt = d + pd.to_timedelta(step, unit="D")
    return (wd < 5) & (np.asarray(nxt.month) != np.asarray(d.month))


def generate(df, name):
    empty = pd.DataFrame(columns=["i", "side", "ext", "atr"])
    if len(df) == 0:
        return empty
    lon = df.index.tz_localize("UTC").tz_convert("Europe/London")
    hr = np.asarray(lon.hour)
    dkey = np.asarray(lon.year) * 10000 + np.asarray(lon.month) * 100 + np.asarray(lon.day)   # London calendar date
    me = _month_end_flags(lon)
    o, c = df.open.values, df.close.values
    a = atr(df).values                                   # ATR14 including bar i: known at the close of bar i
    rows = []
    cur, sess_open = None, np.nan
    for i in range(len(df)):                             # single forward pass: state at i uses bars 0..i only
        if dkey[i] != cur:
            cur, sess_open = dkey[i], np.nan
        if not me[i]:
            continue
        if hr[i] == SESSION_OPEN_HOUR:
            sess_open = o[i]
        elif hr[i] == FIX_BAR_HOUR and np.isfinite(sess_open):
            m = c[i] - sess_open
            if m == 0 or not (np.isfinite(a[i]) and a[i] > 0):
                continue
            side = -1 if m > 0 else 1                    # fade the session move into the fix
            rows.append((df.index[i], i, side, c[i] - side * STOP_ATR * a[i], a[i]))
    if not rows:
        return empty
    return pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")
