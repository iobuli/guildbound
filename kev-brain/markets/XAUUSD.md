# XAUUSD

class metal; 2009-10-21 to 2026-07-27; clock UTC+0 (weekly reopen at 18:00 New York on 86% of 889 weeks with the clock read as UTC, so the clock is UTC).
Session: ny-close days, stubs merged if <= 6h, trader clock Europe/London. Costs: 1.2 bps + 0.1R (from the user's gold backtest page; slippage 0.10R per trade as on the user's pages). In-sample <= 2018.

Overrides (markets/XAUUSD/override.json): `{ "split": { "isEndYear": 2018 } }`


2026-10-21. 5 file(s).

## Files

| file | layout | tf | bars | from | to | fixes |
|---|---|---|---|---|---|---|
| bd123664-XAUUSD30.csv | MT4 history | m30 | 200000 | 2009-10-21 01:00 | 2026-07-27 18:30 | none |
| 299d826d-XAUUSD60_1.csv | MT4 history | h1 | 100345 | 2009-10-21 01:00 | 2026-07-27 18:00 | none |
| 24af8ff4-XAUUSD60_2.csv | MT4 history | h1 | 100345 | 2009-10-21 01:00 | 2026-07-27 18:00 | none |
| 84661a18-XAUUSD240.csv | MT4 history | h4 | 26835 | 2009-10-21 00:00 | 2026-07-27 16:00 | none |
| 7c855670-XAUUSD1440.csv | MT4 history | d1 | 5216 | 2009-10-21 00:00 | 2026-07-27 00:00 | none |

## Consistency between timeframes

- m30 -> h1: 99655 complete buckets compared, 0 differ (largest difference 0.000)
- m30 -> h4: 21971 complete buckets compared, 0 differ (largest difference 0.000)
- h1 -> h4: 22567 complete buckets compared, 0 differ (largest difference 0.000)
- daily file 7c855670-XAUUSD1440.csv: closes match the hourly series best with a day ending at 0:00 data clock (100.0% of 5209 days)

## H1 execution series

100345 bars, 299d826d-XAUUSD60_1.csv.

## Clock

UTC+0: weekly reopen at 18:00 New York on 86% of 889 weeks with the clock read as UTC, so the clock is UTC.

## Weekends and volume

Share of H1 bars on Saturday/Sunday by year: 2009: 2%, 2010: 2%, 2011: 2%, 2012: 1%, 2013: 1%, 2014: 1%, 2015: 1%, 2016: 1%, 2017: 1%, 2018: 2%, 2019: 2%, 2020: 2%, 2021: 2%, 2022: 1%, 2023: 1%, 2024: 1%, 2025: 1%, 2026: 2%.
Weekend closes: 875; other gaps of a day or more: 14.
Median bar volume by year: 2009: 60, 2010: 60, 2011: 60, 2012: 60, 2013: 60, 2014: 60, 2015: 60, 2016: 60, 2017: 60, 2018: 60, 2019: 60, 2020: 60, 2021: 60, 2022: 60, 2023: 60, 2024: 60, 2025: 60, 2026: 60.

## Profile

class metal; tick 0.001; session ny-close days, Sunday stubs merged if <= 6h; costs 1.2 bps + 0.1R (from the user's gold backtest page); in-sample <= 2019. Override any of these in markets/XAUUSD/override.json.

## Notes

- 24af8ff4-XAUUSD60_2.csv is identical to 299d826d-XAUUSD60_1.csv and was ignored.
- The file's own daily candles end at 0:00 on its clock (100% of closes match). kev builds its own days from the session instead (see Session).
- 14 gaps of 24h+ that are not weekends (holidays or missing data).
