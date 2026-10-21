# SOLUSDT

class crypto; 2020-08-11 to 2026-10-20; clock UTC+0 (matches Binance SOLUSDT at an offset of 0h (hourly return correlation 1, 3 windows)).
Session: utc days, stubs merged if <= 6h, trader clock Europe/London. Costs: 0 bps + 0.1R (Binance spot: no spread modelled, 0.10R slippage, the same costs as every earlier SOL run). In-sample <= 2023.

Overrides (markets/SOLUSDT/override.json): `{ "costs": { "spreadBps": 0, "slipR": 0.1, "note": "Binance spot: no spread modelled, 0.10R slippage, the same costs as every earlier SOL run" } }`


2026-10-21. 3 file(s).

## Files

| file | layout | tf | bars | from | to | fixes |
|---|---|---|---|---|---|---|
| SOLUSDT_1h.json | json candles | h1 | 54256 | 2020-08-11 06:00 | 2026-10-20 17:00 | none |
| SOLUSDT_4h.json | json candles | h4 | 13569 | 2020-08-11 04:00 | 2026-10-20 12:00 | none |
| a66a532d-Solana_daily_data_2018_2024.csv | header | d1 | 1368 | 2021-01-01 00:00 | 2024-09-29 00:00 | 3 OHLC repaired |

## Consistency between timeframes

- h1 -> h4: 13555 complete buckets compared, 0 differ (largest difference 0.0000)
- daily file a66a532d-Solana_daily_data_2018_2024.csv: closes match the hourly series best with a day ending at 0:00 data clock (14.4% of 1368 days)

## H1 execution series

54256 bars, SOLUSDT_1h.json.

## Clock

UTC+0: matches Binance SOLUSDT at an offset of 0h (hourly return correlation 1, 3 windows).

## Weekends and volume

Share of H1 bars on Saturday/Sunday by year: 2020: 28%, 2021: 28%, 2022: 29%, 2023: 29%, 2024: 28%, 2025: 28%, 2026: 29%.
Weekend closes: 0; other gaps of a day or more: 0.
Median bar volume by year: 2020: 57000, 2021: 139000, 2022: 108000, 2023: 143000, 2024: 138000, 2025: 120000, 2026: 79800.

## Profile

class crypto; tick 0.0001; session utc days, Sunday stubs merged if <= 6h; costs 5 bps + 0.1R (assumed broker CFD spread; set 0 for exchange spot via override.json); in-sample <= 2023. Override any of these in markets/SOLUSDT/override.json.

## Notes

- Daily file a66a532d-Solana_daily_data_2018_2024.csv does not match the hourly series at any day boundary (best 14%): it comes from a different feed, so it is kept as a cross-check only.
