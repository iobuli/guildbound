```
# XAUUSD M30 base, 2010-06-01 to 2026-07-27 (16.2 years)
32022 scans, 22264 fills (19818 valid stop). Costs: 1.2 bps spread + 0.10R slippage. Mean net R per trade [95% CI, bootstrapped over days]. In-sample 2010-2018, out-of-sample 2019+.

## 1. Everything the system would fill (overlapping fills allowed)
all fills                    n= 19818  1R -0.22 [-0.24,-0.21]  2R -0.18 [-0.20,-0.16]  3R -0.15 [-0.18,-0.13]  4R -0.13 [-0.17,-0.11]  ftaR -0.23 [-0.26,-0.20]
  in-sample                  n= 10733  1R -0.22 [-0.24,-0.20]  2R -0.16 [-0.19,-0.14]  3R -0.14 [-0.17,-0.09]  4R -0.11 [-0.16,-0.06]  ftaR -0.21 [-0.25,-0.17]
  out-of-sample              n=  9085  1R -0.23 [-0.25,-0.21]  2R -0.20 [-0.22,-0.16]  3R -0.17 [-0.20,-0.13]  4R -0.16 [-0.20,-0.11]  ftaR -0.26 [-0.31,-0.21]
  longs                      n= 10211  1R -0.22 [-0.25,-0.20]  2R -0.18 [-0.21,-0.14]  3R -0.15 [-0.19,-0.10]  4R -0.11 [-0.16,-0.06]  ftaR -0.25 [-0.29,-0.20]
  shorts                     n=  9607  1R -0.22 [-0.25,-0.20]  2R -0.18 [-0.22,-0.14]  3R -0.15 [-0.20,-0.10]  4R -0.16 [-0.21,-0.10]  ftaR -0.21 [-0.26,-0.15]

## 2. One position at a time, $500 risk per R, no compounding (the way your backtest page counts)
2  R target:  7696 trades, win 32.3%, mean -0.204R, $-48554/yr, Sharpe -3.15, max DD 1581.0R   | OOS only: $-51694/yr Sharpe -3.39
3  R target:  6222 trades, win 25.4%, mean -0.169R, $-32584/yr, Sharpe -1.89, max DD 1083.7R   | OOS only: $-31794/yr Sharpe -1.83
4  R target:  5379 trades, win 20.7%, mean -0.167R, $-27741/yr, Sharpe -1.52, max DD 921.2R   | OOS only: $-29373/yr Sharpe -1.62
ftaR target:  9237 trades, win 34.1%, mean -0.244R, $-69780/yr, Sharpe -3.52, max DD 2268.0R   | OOS only: $-72966/yr Sharpe -3.80

## 3. Against random orders (same side, distance, risk, window)
real levels                  n= 19818  1R -0.22 [-0.24,-0.21]  2R -0.18 [-0.20,-0.16]  3R -0.15 [-0.18,-0.13]  4R -0.13 [-0.17,-0.11]  ftaR -0.23 [-0.26,-0.20]
placebo                      n= 16519  1R -0.34 [-0.36,-0.32]  2R -0.32 [-0.35,-0.30]  3R -0.30 [-0.33,-0.27]  4R -0.28 [-0.31,-0.25]  ftaR n/a
  real, OOS                  n=  9085  1R -0.23 [-0.25,-0.21]  2R -0.20 [-0.22,-0.16]  3R -0.17 [-0.20,-0.13]  4R -0.16 [-0.20,-0.11]  ftaR -0.26 [-0.31,-0.21]
  placebo, OOS               n=  7555  1R -0.34 [-0.36,-0.32]  2R -0.32 [-0.35,-0.29]  3R -0.29 [-0.33,-0.25]  4R -0.27 [-0.32,-0.22]  ftaR n/a
real minus placebo: 1R 0.114 (p=0.00)   2R 0.144 (p=0.01)   3R 0.144 (p=0.05)   4R 0.143 (p=0.09)
longs vs placebo longs, shorts vs placebo shorts at 4R: long 0.159 (p=0.00)   short 0.127 (p=0.02)

## 4. By the system's verdict and tier
GO                           n=  2818  1R -0.29 [-0.33,-0.25]  2R -0.23 [-0.28,-0.16]  3R -0.18 [-0.25,-0.10]  4R -0.15 [-0.23,-0.05]  ftaR -0.27 [-0.36,-0.17]
NO-GO                        n= 17000  1R -0.21 [-0.23,-0.19]  2R -0.17 [-0.20,-0.15]  3R -0.15 [-0.18,-0.12]  4R -0.13 [-0.16,-0.10]  ftaR -0.22 [-0.26,-0.18]
  GO in-sample               n=  1591  1R -0.28 [-0.33,-0.22]  2R -0.22 [-0.30,-0.14]  3R -0.17 [-0.26,-0.06]  4R -0.12 [-0.22,-0.00]  ftaR -0.29 [-0.41,-0.15]
  GO out-of-sample           n=  1227  1R -0.30 [-0.36,-0.25]  2R -0.23 [-0.32,-0.15]  3R -0.21 [-0.30,-0.10]  4R -0.19 [-0.31,-0.07]  ftaR -0.25 [-0.40,-0.09]
tier blind                   n=  4144  1R -0.28 [-0.32,-0.25]  2R -0.21 [-0.26,-0.16]  3R -0.19 [-0.25,-0.12]  4R -0.14 [-0.21,-0.06]  ftaR -0.28 [-0.35,-0.21]
tier needs-one-confluence    n=  4859  1R -0.28 [-0.31,-0.25]  2R -0.23 [-0.27,-0.19]  3R -0.18 [-0.23,-0.13]  4R -0.18 [-0.25,-0.12]  ftaR -0.28 [-0.35,-0.21]
tier needs-two-confluences   n= 10815  1R -0.17 [-0.20,-0.15]  2R -0.14 [-0.18,-0.11]  3R -0.12 [-0.16,-0.08]  4R -0.11 [-0.16,-0.06]  ftaR -0.19 [-0.23,-0.14]
GO minus NO-GO at 4R: -0.017R (p=0.64)

## 5. By year: mean net R at 4R, real vs placebo (all fills)
2010  real  -0.03 (n=  720)   placebo  -0.20 (n=  605)   GO   0.35 (n=112)
2011  real  -0.00 (n= 1299)   placebo  -0.20 (n= 1060)   GO   0.15 (n=179)
2012  real  -0.10 (n= 1269)   placebo  -0.28 (n= 1052)   GO  -0.09 (n=214)
2013  real  -0.26 (n= 1324)   placebo  -0.43 (n= 1110)   GO  -0.49 (n=217)
2014  real  -0.12 (n= 1296)   placebo  -0.31 (n= 1104)   GO  -0.19 (n=222)
2015  real  -0.16 (n= 1212)   placebo  -0.27 (n= 1025)   GO  -0.17 (n=203)
2016  real  -0.20 (n= 1218)   placebo  -0.38 (n= 1023)   GO  -0.23 (n=165)
2017  real  -0.04 (n= 1206)   placebo  -0.20 (n= 1020)   GO  -0.06 (n=152)
2018  real  -0.09 (n= 1189)   placebo  -0.26 (n=  965)   GO  -0.01 (n=127)
2019  real  -0.18 (n= 1255)   placebo  -0.26 (n= 1039)   GO  -0.09 (n=191)
2020  real  -0.05 (n= 1088)   placebo  -0.24 (n=  904)   GO  -0.18 (n=138)
2021  real  -0.06 (n= 1122)   placebo  -0.12 (n=  932)   GO   0.08 (n=164)
2022  real  -0.29 (n= 1273)   placebo  -0.38 (n= 1065)   GO  -0.52 (n=150)
2023  real  -0.32 (n= 1267)   placebo  -0.41 (n= 1062)   GO  -0.38 (n=156)
2024  real  -0.01 (n= 1184)   placebo  -0.16 (n=  989)   GO  -0.02 (n=187)
2025  real  -0.15 (n= 1187)   placebo  -0.24 (n=  989)   GO  -0.24 (n=139)
2026  real  -0.19 (n=  709)   placebo  -0.33 (n=  575)   GO  -0.31 (n=102)

## 6. Each rule alone: PASS vs FAIL at 4R, with in-sample / out-of-sample consistency
level.opposite-last                      pass n=19547 -0.13R | fail n=  271 -0.22R | diff   0.08 p=0.014 q=0.099  IS 0.04 OOS 0.13
atr-gate.no-same-direction               pass n=19021 -0.14R | fail n=  797 -0.06R | diff  -0.08 p=0.022 q=0.099  IS 0.05 OOS -0.23
retest.needs-space-in-price              pass n=16286 -0.13R | fail n= 3532 -0.17R | diff   0.04 p=0.331 q=0.844  IS 0.07 OOS -0.00
target.minimum-rr                        pass n=11065 -0.18R | fail n= 8753 -0.08R | diff  -0.10 p=0.379 q=0.844  IS -0.04 OOS -0.16
level.chopping-through                   pass n=13295 -0.13R | fail n= 6523 -0.14R | diff   0.01 p=0.589 q=0.844  IS 0.09 OOS -0.08
entry.vetoes-stack                       pass n= 2818 -0.15R | fail n=17000 -0.13R | diff  -0.02 p=0.633 q=0.844  IS -0.00 OOS -0.04
retest.needs-space-in-time               pass n=11462 -0.13R | fail n= 8356 -0.14R | diff   0.01 p=0.701 q=0.844  IS 0.08 OOS -0.07
confluence.three-needed                  pass n= 7366 -0.14R | fail n=12452 -0.13R | diff  -0.02 p=0.752 q=0.844  IS -0.00 OOS -0.04
retest.never-immediate                   pass n=12233 -0.13R | fail n= 7585 -0.14R | diff   0.00 p=0.844 q=0.844  IS 0.02 OOS -0.02

## 7. The same pipeline on gold with its memory destroyed (block-shuffled by day), 2 seeds
shuffled #101 all fills      n= 19599  1R -0.24 [-0.26,-0.22]  2R -0.20 [-0.22,-0.18]  3R -0.18 [-0.21,-0.15]  4R -0.16 [-0.19,-0.13]  ftaR -0.25 [-0.29,-0.22]
shuffled #101 GO             n=  2520  1R -0.29 [-0.33,-0.25]  2R -0.24 [-0.30,-0.18]  3R -0.23 [-0.30,-0.15]  4R -0.21 [-0.29,-0.12]  ftaR -0.18 [-0.30,-0.03]
   one-at-a-time 4R: 4918 trades, mean -0.164R, $-24949/yr, Sharpe -1.42
shuffled #202 all fills      n= 20304  1R -0.24 [-0.26,-0.23]  2R -0.20 [-0.22,-0.18]  3R -0.17 [-0.20,-0.15]  4R -0.17 [-0.20,-0.14]  ftaR -0.23 [-0.26,-0.21]
shuffled #202 GO             n=  2637  1R -0.30 [-0.35,-0.26]  2R -0.25 [-0.31,-0.19]  3R -0.21 [-0.29,-0.14]  4R -0.20 [-0.29,-0.13]  ftaR -0.24 [-0.32,-0.17]
   one-at-a-time 4R: 4899 trades, mean -0.169R, $-25616/yr, Sharpe -1.45

seconds 323
```
