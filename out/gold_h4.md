```
# XAUUSD H4 base, 2010-06-01 to 2026-07-27 (16.2 years)
25817 scans, 6096 fills (5824 valid stop). Costs: 1.2 bps spread + 0.10R slippage. Mean net R per trade [95% CI, bootstrapped over days]. In-sample 2010-2018, out-of-sample 2019+.

## 1. Everything the system would fill (overlapping fills allowed)
all fills                    n=  5824  1R -0.24 [-0.27,-0.21]  2R -0.18 [-0.22,-0.14]  3R -0.15 [-0.20,-0.10]  4R -0.12 [-0.18,-0.07]  ftaR -0.25 [-0.29,-0.21]
  in-sample                  n=  3148  1R -0.26 [-0.30,-0.22]  2R -0.20 [-0.26,-0.15]  3R -0.19 [-0.25,-0.12]  4R -0.16 [-0.23,-0.08]  ftaR -0.27 [-0.32,-0.21]
  out-of-sample              n=  2676  1R -0.21 [-0.25,-0.16]  2R -0.16 [-0.22,-0.09]  3R -0.11 [-0.19,-0.05]  4R -0.08 [-0.17,-0.01]  ftaR -0.23 [-0.30,-0.17]
  longs                      n=  3001  1R -0.22 [-0.26,-0.18]  2R -0.15 [-0.22,-0.09]  3R -0.11 [-0.19,-0.03]  4R -0.09 [-0.16,0.00]  ftaR -0.22 [-0.27,-0.16]
  shorts                     n=  2823  1R -0.25 [-0.30,-0.20]  2R -0.22 [-0.28,-0.16]  3R -0.20 [-0.28,-0.13]  4R -0.16 [-0.25,-0.08]  ftaR -0.28 [-0.33,-0.22]

## 2. One position at a time, $500 risk per R, no compounding (the way your backtest page counts)
2  R target:  1774 trades, win 32.5%, mean -0.208R, $-11436/yr, Sharpe -1.59, max DD 369.5R   | OOS only: $-7682/yr Sharpe -1.08
3  R target:  1583 trades, win 28.0%, mean -0.191R, $-9343/yr, Sharpe -1.18, max DD 333.9R   | OOS only: $-5259/yr Sharpe -0.66
4  R target:  1485 trades, win 26.0%, mean -0.184R, $-8449/yr, Sharpe -1.00, max DD 287.5R   | OOS only: $-5564/yr Sharpe -0.66
ftaR target:  2527 trades, win 36.7%, mean -0.307R, $-23998/yr, Sharpe -3.09, max DD 776.7R   | OOS only: $-21338/yr Sharpe -2.65

## 3. Against random orders (same side, distance, risk, window)
real levels                  n=  5824  1R -0.24 [-0.27,-0.21]  2R -0.18 [-0.22,-0.14]  3R -0.15 [-0.20,-0.10]  4R -0.12 [-0.18,-0.07]  ftaR -0.25 [-0.29,-0.21]
placebo                      n=  5236  1R -0.30 [-0.33,-0.27]  2R -0.25 [-0.29,-0.20]  3R -0.23 [-0.28,-0.19]  4R -0.21 [-0.27,-0.16]  ftaR n/a
  real, OOS                  n=  2676  1R -0.21 [-0.25,-0.16]  2R -0.16 [-0.22,-0.09]  3R -0.11 [-0.19,-0.05]  4R -0.08 [-0.17,-0.01]  ftaR -0.23 [-0.30,-0.17]
  placebo, OOS               n=  2408  1R -0.27 [-0.32,-0.23]  2R -0.23 [-0.28,-0.18]  3R -0.19 [-0.26,-0.12]  4R -0.17 [-0.25,-0.09]  ftaR n/a
real minus placebo: 1R 0.063 (p=0.00)   2R 0.065 (p=0.00)   3R 0.077 (p=0.00)   4R 0.083 (p=0.00)
longs vs placebo longs, shorts vs placebo shorts at 4R: long 0.091 (p=0.01)   short 0.076 (p=0.00)

## 4. By the system's verdict and tier
GO                           n=  1046  1R -0.26 [-0.33,-0.19]  2R -0.16 [-0.26,-0.06]  3R -0.13 [-0.24,-0.01]  4R -0.10 [-0.24,0.05]  ftaR -0.24 [-0.35,-0.12]
NO-GO                        n=  4778  1R -0.23 [-0.26,-0.20]  2R -0.19 [-0.23,-0.13]  3R -0.16 [-0.21,-0.09]  4R -0.13 [-0.19,-0.05]  ftaR -0.25 [-0.29,-0.21]
  GO in-sample               n=   542  1R -0.26 [-0.34,-0.17]  2R -0.10 [-0.23,0.04]  3R -0.06 [-0.24,0.12]  4R 0.00 [-0.19,0.22]  ftaR -0.21 [-0.34,-0.07]
  GO out-of-sample           n=   504  1R -0.25 [-0.36,-0.15]  2R -0.23 [-0.37,-0.10]  3R -0.20 [-0.36,-0.04]  4R -0.20 [-0.39,-0.03]  ftaR -0.28 [-0.46,-0.03]
tier blind                   n=   902  1R -0.25 [-0.33,-0.18]  2R -0.19 [-0.30,-0.09]  3R -0.10 [-0.23,0.03]  4R -0.01 [-0.18,0.15]  ftaR -0.27 [-0.35,-0.18]
tier needs-one-confluence    n=  1424  1R -0.34 [-0.41,-0.28]  2R -0.24 [-0.34,-0.16]  3R -0.20 [-0.31,-0.10]  4R -0.14 [-0.28,-0.02]  ftaR -0.30 [-0.40,-0.21]
tier needs-two-confluences   n=  3498  1R -0.19 [-0.23,-0.15]  2R -0.15 [-0.21,-0.10]  3R -0.15 [-0.21,-0.09]  4R -0.14 [-0.22,-0.08]  ftaR -0.22 [-0.27,-0.18]
GO minus NO-GO at 4R: 0.032R (p=0.55)

## 5. By year: mean net R at 4R, real vs placebo (all fills)
2010  real  -0.11 (n=  257)   placebo  -0.19 (n=  231)   GO  -0.10 (n=35)
2011  real  -0.32 (n=  420)   placebo  -0.36 (n=  386)   GO  -0.27 (n=77)
2012  real  -0.00 (n=  366)   placebo  -0.07 (n=  327)   GO  -0.23 (n=70)
2013  real  -0.09 (n=  345)   placebo  -0.16 (n=  313)   GO   0.90 (n=84)
2014  real  -0.25 (n=  337)   placebo  -0.39 (n=  300)   GO  -0.12 (n=62)
2015  real  -0.01 (n=  323)   placebo  -0.04 (n=  290)   GO   0.12 (n=58)
2016  real  -0.28 (n=  334)   placebo  -0.39 (n=  299)   GO  -0.29 (n=56)
2017  real  -0.10 (n=  409)   placebo  -0.23 (n=  357)   GO   0.03 (n=63)
2018  real  -0.20 (n=  357)   placebo  -0.26 (n=  325)   GO  -0.51 (n=37)
2019  real  -0.20 (n=  289)   placebo  -0.26 (n=  263)   GO  -0.35 (n=67)
2020  real   0.21 (n=  342)   placebo   0.12 (n=  313)   GO  -0.57 (n=47)
2021  real  -0.18 (n=  311)   placebo  -0.23 (n=  278)   GO  -0.07 (n=67)
2022  real  -0.10 (n=  408)   placebo  -0.14 (n=  371)   GO  -0.19 (n=65)
2023  real  -0.01 (n=  385)   placebo  -0.17 (n=  345)   GO   0.16 (n=84)
2024  real  -0.21 (n=  372)   placebo  -0.30 (n=  335)   GO  -0.40 (n=74)
2025  real  -0.07 (n=  347)   placebo  -0.19 (n=  311)   GO  -0.35 (n=79)
2026  real  -0.16 (n=  222)   placebo  -0.27 (n=  192)   GO   0.36 (n=21)

## 6. Each rule alone: PASS vs FAIL at 4R, with in-sample / out-of-sample consistency
target.minimum-rr                        pass n= 2786 -0.24R | fail n= 3038 -0.02R | diff  -0.22 p=0.002 q=0.016  IS -0.20 OOS -0.24
retest.needs-space-in-price              pass n= 4574 -0.11R | fail n= 1250 -0.17R | diff   0.06 p=0.248 q=0.735  IS 0.02 OOS 0.11
atr-gate.no-same-direction               pass n= 5604 -0.13R | fail n=  220 -0.05R | diff  -0.07 p=0.275 q=0.735  IS -0.12 OOS -0.02
level.chopping-through                   pass n= 5721 -0.12R | fail n=  103 -0.16R | diff   0.04 p=0.445 q=0.865  IS -0.13 OOS 0.27
entry.vetoes-stack                       pass n= 1046 -0.10R | fail n= 4778 -0.13R | diff   0.03 p=0.541 q=0.865  IS 0.19 OOS -0.15
retest.never-immediate                   pass n= 3703 -0.13R | fail n= 2121 -0.11R | diff  -0.01 p=0.802 q=0.876  IS 0.08 OOS -0.12
confluence.three-needed                  pass n= 2007 -0.14R | fail n= 3817 -0.11R | diff  -0.03 p=0.814 q=0.876  IS 0.10 OOS -0.17
retest.needs-space-in-time               pass n= 4176 -0.12R | fail n= 1648 -0.13R | diff   0.01 p=0.876 q=0.876  IS 0.13 OOS -0.14

## 7. The same pipeline on gold with its memory destroyed (block-shuffled by day), 2 seeds
shuffled #101 all fills      n=  6018  1R -0.26 [-0.30,-0.24]  2R -0.22 [-0.26,-0.18]  3R -0.21 [-0.26,-0.16]  4R -0.19 [-0.25,-0.15]  ftaR -0.25 [-0.29,-0.22]
shuffled #101 GO             n=   985  1R -0.35 [-0.42,-0.28]  2R -0.26 [-0.36,-0.16]  3R -0.21 [-0.32,-0.09]  4R -0.22 [-0.35,-0.10]  ftaR -0.28 [-0.39,-0.19]
   one-at-a-time 4R: 1571 trades, mean -0.257R, $-12479/yr, Sharpe -1.44
shuffled #202 all fills      n=  5504  1R -0.28 [-0.31,-0.25]  2R -0.22 [-0.27,-0.19]  3R -0.17 [-0.23,-0.12]  4R -0.14 [-0.21,-0.09]  ftaR -0.27 [-0.31,-0.23]
shuffled #202 GO             n=   893  1R -0.29 [-0.37,-0.22]  2R -0.18 [-0.30,-0.09]  3R -0.15 [-0.28,-0.04]  4R -0.13 [-0.27,-0.01]  ftaR -0.27 [-0.37,-0.17]
   one-at-a-time 4R: 1482 trades, mean -0.232R, $-10633/yr, Sharpe -1.29

seconds 92
```
