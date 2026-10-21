# ETHUSD

class crypto; 2018-01-01 to 2026-10-21; clock UTC+0 (matches Binance ETHUSDT at an offset of 0h (hourly return correlation 0.989, 3 windows)).
Session: utc days, stubs merged if <= 6h, trader clock Europe/London. Costs: 5 bps + 0.1R (assumed broker CFD spread; set 0 for exchange spot via override.json; slippage 0.10R per trade as on the user's pages). In-sample <= 2022.



2026-10-21. 5 file(s).

## Files

| file | layout | tf | bars | from | to | fixes |
|---|---|---|---|---|---|---|
| 6f384fbe-ETHUSD1.csv | MT4 history | m1 | 200000 | 2026-06-03 11:41 | 2026-10-21 15:59 | none |
| db3ff727-ETHUSD5.csv | MT4 history | m5 | 200000 | 2024-11-24 02:25 | 2026-10-21 15:55 | none |
| 3eb81ca4-ETHUSD15.csv | MT4 history | m15 | 200000 | 2021-01-14 16:45 | 2026-10-21 15:45 | none |
| baac8c7d-ETHUSD30.csv | MT4 history | m30 | 139593 | 2018-01-01 22:00 | 2026-10-21 15:30 | none |
| 7c2692aa-ETHUSD240.csv | MT4 history | h4 | 17646 | 2018-01-01 20:00 | 2026-10-21 12:00 | none |

## Consistency between timeframes

- m1 -> m5: 38878 complete buckets compared, 0 differ (largest difference 0.0)
- m1 -> m15: 12322 complete buckets compared, 0 differ (largest difference 0.0)
- m1 -> m30: 5872 complete buckets compared, 0 differ (largest difference 0.0)
- m1 -> h4: 580 complete buckets compared, 0 differ (largest difference 0.0)
- m5 -> m15: 66660 complete buckets compared, 0 differ (largest difference 0.0)
- m5 -> m30: 33324 complete buckets compared, 0 differ (largest difference 0.0)
- m5 -> h4: 4156 complete buckets compared, 0 differ (largest difference 0.0)
- m15 -> m30: 99945 complete buckets compared, 0 differ (largest difference 0.0)
- m15 -> h4: 12411 complete buckets compared, 0 differ (largest difference 0.0)
- m30 -> h4: 17243 complete buckets compared, 0 differ (largest difference 0.0)

## H1 execution series

69850 bars, assembled from m1, m5, m15, m30. 106 hours built from incomplete buckets (marked in notes).

## Clock

UTC+0: matches Binance ETHUSDT at an offset of 0h (hourly return correlation 0.989, 3 windows).

## Weekends and volume

Share of H1 bars on Saturday/Sunday by year: 2018: 2%, 2019: 2%, 2020: 13%, 2021: 29%, 2022: 26%, 2023: 28%, 2024: 28%, 2025: 29%, 2026: 29%.
Weekend closes: 142; other gaps of a day or more: 7.
Median bar volume by year: 2018: 99200, 2019: 130000, 2020: 138000, 2021: 212000, 2022: 203000, 2023: 20000, 2024: 180000, 2025: 41300, 2026: 1940.

## Profile

class crypto; tick 0.1; session utc days, Sunday stubs merged if <= 6h; costs 5 bps + 0.1R (assumed broker CFD spread; set 0 for exchange spot via override.json); in-sample <= 2022. Override any of these in markets/ETHUSD/override.json.

## Notes

- 106 H1 bars were built from incomplete finer buckets; their high/low may be slightly understated.
- Weekend trading changes over time (2018: 2%, 2019: 2%, 2020: 13%, 2021: 29%, 2022: 26%, 2023: 28%, 2024: 28%, 2025: 29%, 2026: 29%): the market was closed at weekends in some years and open in others. Sessions merge the short Sunday reopening into Monday in the closed years.
- 7 gaps of 24h+ that are not weekends (holidays or missing data).
- Volume scale changes 110x between years: only use volume relative to a trailing window.
