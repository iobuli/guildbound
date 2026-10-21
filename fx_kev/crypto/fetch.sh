#!/bin/sh
# Download Binance spot 1h monthly klines 2018-01..2025-12 (2026 deliberately excluded).
cd "$(dirname "$0")/raw"
for s in BTCUSDT ETHUSDT BNBUSDT XRPUSDT ADAUSDT LTCUSDT LINKUSDT DOGEUSDT; do
  for y in 2018 2019 2020 2021 2022 2023 2024 2025; do
    for m in 01 02 03 04 05 06 07 08 09 10 11 12; do
      f="$s-1h-$y-$m.zip"; [ -s "$f" ] && continue
      curl -sf -m 60 -o "$f" "https://data.binance.vision/data/spot/monthly/klines/$s/1h/$f" || rm -f "$f"
    done
  done
  echo "$s $(ls $s-1h-*.zip 2>/dev/null | wc -l) months"
done
