"""R2D: unconditional time-of-day USD order-flow seasonality, two fixed legs per day (Tokyo long-USD, New York short-USD).

Session clock (fixed assumption, verified ONLY from timestamps, never from prices/returns): bar times are UTC.
Evidence: USDJPY's first bar of the week opens Sunday 22:00 UTC in January and 21:00 UTC in July (= 17:00 ET, DST-aware);
XAUUSD/XAGUSD reopen Sunday 23:00/22:00 UTC (= 18:00 ET); SOLUSDT is Binance data (UTC, 24/7); no bar has a non-zero
minute. Bars are mapped to Asia/Tokyo (UTC+9, no DST) and America/New_York (DST-aware) with the IANA tz database. This
is a deterministic calendar mapping of the signal bar's own open time: causal.

Constants are canonical and were fixed before the first 'check': signal at 08:00 JST (entry 09:00 JST = Tokyo cash
open / start of Tokyo dealing day) and 07:00 ET (entry 08:00 ET = start of the New York FX dealing day); H = 8 hours
for both legs (09:00-17:00 JST, 08:00-16:00 ET: the standard eight-hour local business day); pad = 3.0 ATR is a
catastrophe stop only (not a trading parameter); no take-profit; w = 1. No variant was counted or evaluated.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

TOKYO_SIGNAL_HOUR = 8     # Asia/Tokyo hour of the signal bar's OPEN (08:00 JST = 23:00 UTC); entry 09:00 JST
NY_SIGNAL_HOUR = 7        # America/New_York hour of the signal bar's OPEN (DST-aware); entry 08:00 ET

PREREG = dict(
    id="R2D",
    family="time-of-day USD order-flow seasonality (two legs per day, drift-neutral)",
    desc=(
        "Treat bar times as UTC (bar index = bar OPEN time). Convert each bar's open time to Asia/Tokyo (UTC+9, no DST) "
        "and to America/New_York (DST-aware, IANA tz). Define usd = +1 for USDJPY_H1 (price rises when USD strengthens) "
        "and usd = -1 for XAUUSD_H1, XAGUSD_H1 and SOLUSDT_H1 (USD/USDT-quoted: price falls when USD strengthens). Two "
        "fixed legs per day, the same rule on all four instruments. (1) TOKYO LEG: every bar whose open time is 08:00 "
        "Asia/Tokyo (= 23:00 UTC of the previous UTC date) on a Tokyo weekday (Monday-Friday of the Asia/Tokyo date, so "
        "Monday's leg is signalled by the Sunday 23:00 UTC bar) is a signal bar with side = +usd (LONG USD: USDJPY long; "
        "gold, silver, SOL short). Entry at the open of the next bar (09:00 JST); time exit at the close of the 8th bar "
        "after entry (H = 8, i.e. 17:00 JST when bars are contiguous). (2) NEW YORK LEG: every bar whose open time is "
        "07:00 America/New_York (11:00 UTC under EDT, 12:00 UTC under EST) on a New York weekday (Monday-Friday of the "
        "New York date) is a signal bar with side = -usd (SHORT USD: USDJPY short; gold, silver, SOL long). Entry at the "
        "open of the next bar (08:00 ET); time exit at the close of the 8th bar after entry (16:00 ET). If the signal "
        "bar does not exist (holiday, data gap) there is no trade for that leg; holding is positional (8 bars), never "
        "conditioned on later bars. For both legs: ext = close of the signal bar i, atr = sweep_lib.atr(df) (ATR14 "
        "including bar i, known at its close), stop = ext - side*3.0*atr (a catastrophe stop only, ~3 H1 ATR from "
        "entry), no take-profit, weight w = 1. No price, volume, volatility or trend condition of any kind: the trade "
        "is unconditional on the clock. Legs of one day are >= 10 bars apart, so thinning with gap H = 8 keeps both."
    ),
    rationale=(
        "Ranaldo (2009, Journal of Banking & Finance, 'Segmentation and time-of-day patterns in foreign exchange "
        "markets') and Breedon & Ranaldo (2013, Journal of Money, Credit and Banking, 'Intraday patterns in FX returns "
        "and order flow') document that currencies systematically depreciate during their own domestic trading hours "
        "and appreciate during foreign hours. Breedon & Ranaldo tie the pattern to order flow: domestic investors and "
        "corporates are net buyers of foreign currency during their own business hours (importers, outward portfolio "
        "investment, hedging by domestic institutions executed through domestic dealers), which pushes the home "
        "currency down while home dealers are the main liquidity takers, and the inventory is unwound when the foreign "
        "centre opens. The effect is a liquidity/inventory premium paid by predictable, clock-driven flow, not "
        "information, so it should be unconditional on price and should not need a level or a trigger. Two legs per "
        "day follow directly: (1) Tokyo business hours (09:00-17:00 JST): JPY is the domestic currency, so JPY should "
        "weaken, i.e. USDJPY rises (long USD); (2) New York business hours (08:00-16:00 ET): USD is the domestic "
        "currency, so USD should weaken, i.e. USDJPY falls, gold, silver and SOL (USD-quoted) rise. USDJPY therefore "
        "has double support on both legs (JPY-domestic in Tokyo, USD-domestic in New York). For XAU, XAG and SOL the "
        "pattern is an out-of-sample prediction of a USD-wide domestic-flow effect: if US-hours USD selling is a broad "
        "dollar flow, dollar-quoted assets should rise in US hours and give it back (USD stronger) in Asian hours. "
        "Pairing a long-USD leg with a short-USD leg every day makes net exposure about zero, so the strong secular "
        "drifts of SOL, gold and USDJPY in 2021-25 cannot by themselves produce a positive mean. Entry and exit times "
        "are the canonical local business-day boundaries (09:00-17:00 JST; 08:00-16:00 ET, the start of the NY dealing "
        "day to the US cash close), not tuned; the 3-ATR stop exists only to cap tail risk on event days so that R is "
        "essentially the 8-hour return scaled by 3 H1 ATR, which keeps cost per R small (USDJPY 1 pip is ~0.02-0.03R)."
    ),
    novelty=(
        "This is the first UNCONDITIONAL time-of-day test in the programme. S1-S13, K1-K4 and N1-N8 trade sweeps of "
        "price levels (previous day/week, Asian range, fractal swings, round numbers) with stops beyond the wick. In "
        "this batch SESS1 trades a conditional New York opening-range breakout (direction set by price), CAL1 trades a "
        "conditional month-end/London-fix price-pressure reversal, and N3 faded a conditional sweep of the Asian range at "
        "the London open (07-08h); none of them holds a fixed-direction position over a fixed clock window every day. R2D's direction is set only by the clock and the "
        "instrument's USD orientation (never by price), it has two opposite-sign legs per day (drift-neutral by "
        "construction), its stop is a 3-ATR catastrophe stop rather than a level, and it is not an inversion of any "
        "failed method."
    ),
    data_informed=True,
    H=8,
    pad=3.0,
    notes=(
        "Round-2 lens. data_informed=True because the lens was chosen after round-1 results (sweep fades: gross edge "
        "smaller than costs with ~0.8 ATR stops; conditional session/fix methods SESS1/CAL1/N3 already tested), which "
        "motivated a wide-stop, cost-light, unconditional clock trade from the FX-seasonality literature. The rule, "
        "legs, directions, hours, H, pad and w come from that literature and the session clock; no profit/loss, R, "
        "return or win rate of this rule or any variant was computed or seen. Only 'eval_proposal.py check' "
        "(causality and counts) was run before registration. Disclosure: after the rule was written, a grep for 'N3' "
        "(to describe N3 correctly in 'novelty') printed N3's stored round-1 result line from OVERNIGHT_RUNS.txt (a "
        "different, failed sweep method); nothing in R2D was changed because of it. Known risks: (i) at an 8-hour hold 1 pip is roughly "
        "0.03-0.04 of the 8-hour return std on USDJPY, so costs subtract ~2.5-3.5 t-units over ~7800 legs, and a real "
        "but small effect may still miss P1/P3 there; (ii) for metals there is a conflicting folk pattern (Asian-hours "
        "physical gold buying would make gold rise, not fall, in Tokyo hours); (iii) SOL/USDT is a 24/7 crypto market "
        "whose USD-flow link is weaker than for FX; (iv) the published FX evidence is mostly pre-2010, so the effect "
        "may have been arbitraged away. Expected non-overlapping counts ~7800 / 2800 / 4000 / 4000."
    ),
)


def generate(df, name):
    cols = ["i", "side", "ext", "atr", "w"]
    if len(df) == 0:
        return pd.DataFrame(columns=cols)

    ix = df.index
    utc = ix.tz_localize("UTC") if ix.tz is None else ix.tz_convert("UTC")
    jst = utc.tz_convert("Asia/Tokyo")
    et = utc.tz_convert("America/New_York")

    usd = 1 if name == "USDJPY_H1" else -1               # +1: instrument price rises when USD strengthens

    # Tokyo leg: signal bar opens 08:00 JST on a Tokyo Monday-Friday -> LONG USD
    tok = (np.asarray(jst.hour) == TOKYO_SIGNAL_HOUR) & (np.asarray(jst.minute) == 0) & (np.asarray(jst.dayofweek) < 5)
    # New York leg: signal bar opens 07:00 ET (DST-aware) on a New York Monday-Friday -> SHORT USD
    ny = (np.asarray(et.hour) == NY_SIGNAL_HOUR) & (np.asarray(et.minute) == 0) & (np.asarray(et.dayofweek) < 5)

    side = np.where(tok, usd, np.where(ny, -usd, 0))
    A = atr(df).values                                    # ATR14 including bar i: known at the close of bar i
    c = df.close.values
    m = (side != 0) & np.isfinite(A) & (A > 0) & np.isfinite(c)
    k = np.flatnonzero(m)
    if len(k) == 0:
        return pd.DataFrame(columns=cols)

    out = pd.DataFrame({
        "i": k.astype(int),
        "side": side[k].astype(int),
        "ext": c[k].astype(float),                        # stop anchor = signal-bar close; stop = ext - side*3*atr
        "atr": A[k].astype(float),
        "w": 1.0,
    }, index=ix[k])
    out.index.name = "t"
    return out
