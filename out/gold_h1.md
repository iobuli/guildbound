```
# XAUUSD H1 base, 2010-06-01 to 2026-07-27 (16.2 years)
32136 scans, 14340 fills (13333 valid stop). Costs: 1.2 bps spread + 0.10R slippage. Mean net R per trade [95% CI, bootstrapped over days]. In-sample 2010-2018, out-of-sample 2019+.

## 1. Everything the system would fill (overlapping fills allowed)
all fills                    n= 13333  1R -0.23 [-0.25,-0.21]  2R -0.17 [-0.19,-0.14]  3R -0.18 [-0.21,-0.15]  4R -0.15 [-0.19,-0.12]  ftaR -0.23 [-0.27,-0.20]
  in-sample                  n=  7212  1R -0.22 [-0.24,-0.19]  2R -0.15 [-0.19,-0.12]  3R -0.16 [-0.21,-0.11]  4R -0.13 [-0.18,-0.07]  ftaR -0.20 [-0.24,-0.16]
  out-of-sample              n=  6121  1R -0.24 [-0.27,-0.21]  2R -0.19 [-0.23,-0.15]  3R -0.20 [-0.25,-0.15]  4R -0.18 [-0.24,-0.13]  ftaR -0.27 [-0.31,-0.23]
  longs                      n=  6834  1R -0.25 [-0.28,-0.22]  2R -0.19 [-0.24,-0.15]  3R -0.19 [-0.25,-0.14]  4R -0.15 [-0.22,-0.10]  ftaR -0.25 [-0.30,-0.20]
  shorts                     n=  6499  1R -0.21 [-0.24,-0.18]  2R -0.14 [-0.19,-0.10]  3R -0.17 [-0.22,-0.10]  4R -0.16 [-0.22,-0.09]  ftaR -0.22 [-0.26,-0.16]

## 2. One position at a time, $500 risk per R, no compounding (the way your backtest page counts)
2  R target:  4600 trades, win 32.9%, mean -0.179R, $-25437/yr, Sharpe -2.16, max DD 836.4R   | OOS only: $-27460/yr Sharpe -2.40
3  R target:  3692 trades, win 25.5%, mean -0.173R, $-19754/yr, Sharpe -1.53, max DD 668.5R   | OOS only: $-22080/yr Sharpe -1.74
4  R target:  3334 trades, win 21.7%, mean -0.152R, $-15711/yr, Sharpe -1.11, max DD 530.1R   | OOS only: $-19563/yr Sharpe -1.41
ftaR target:  6095 trades, win 35.6%, mean -0.249R, $-47060/yr, Sharpe -3.26, max DD 1539.1R   | OOS only: $-52183/yr Sharpe -3.80

## 3. Against random orders (same side, distance, risk, window)
real levels                  n= 13333  1R -0.23 [-0.25,-0.21]  2R -0.17 [-0.19,-0.14]  3R -0.18 [-0.21,-0.15]  4R -0.15 [-0.19,-0.12]  ftaR -0.23 [-0.27,-0.20]
placebo                      n= 11462  1R -0.30 [-0.32,-0.28]  2R -0.27 [-0.29,-0.24]  3R -0.28 [-0.32,-0.25]  4R -0.26 [-0.29,-0.21]  ftaR n/a
  real, OOS                  n=  6121  1R -0.24 [-0.27,-0.21]  2R -0.19 [-0.23,-0.15]  3R -0.20 [-0.25,-0.15]  4R -0.18 [-0.24,-0.13]  ftaR -0.27 [-0.31,-0.23]
  placebo, OOS               n=  5272  1R -0.30 [-0.33,-0.27]  2R -0.26 [-0.30,-0.22]  3R -0.27 [-0.32,-0.23]  4R -0.25 [-0.31,-0.20]  ftaR n/a
real minus placebo: 1R 0.075 (p=0.00)   2R 0.098 (p=0.00)   3R 0.102 (p=0.00)   4R 0.103 (p=0.00)
longs vs placebo longs, shorts vs placebo shorts at 4R: long 0.120 (p=0.00)   short 0.085 (p=0.00)

## 4. By the system's verdict and tier
GO                           n=  2155  1R -0.28 [-0.32,-0.23]  2R -0.22 [-0.29,-0.15]  3R -0.24 [-0.32,-0.16]  4R -0.22 [-0.31,-0.13]  ftaR -0.29 [-0.37,-0.21]
NO-GO                        n= 11178  1R -0.22 [-0.24,-0.20]  2R -0.16 [-0.19,-0.13]  3R -0.17 [-0.20,-0.13]  4R -0.14 [-0.18,-0.10]  ftaR -0.22 [-0.26,-0.19]
  GO in-sample               n=  1204  1R -0.25 [-0.32,-0.19]  2R -0.20 [-0.30,-0.10]  3R -0.20 [-0.32,-0.09]  4R -0.17 [-0.30,-0.05]  ftaR -0.29 [-0.39,-0.15]
  GO out-of-sample           n=   951  1R -0.32 [-0.40,-0.26]  2R -0.24 [-0.35,-0.14]  3R -0.29 [-0.41,-0.17]  4R -0.28 [-0.42,-0.13]  ftaR -0.30 [-0.39,-0.19]
tier blind                   n=  2648  1R -0.28 [-0.33,-0.24]  2R -0.19 [-0.26,-0.14]  3R -0.20 [-0.27,-0.13]  4R -0.15 [-0.23,-0.07]  ftaR -0.26 [-0.34,-0.20]
tier needs-one-confluence    n=  3297  1R -0.31 [-0.35,-0.27]  2R -0.23 [-0.28,-0.17]  3R -0.24 [-0.30,-0.17]  4R -0.22 [-0.29,-0.14]  ftaR -0.32 [-0.39,-0.26]
tier needs-two-confluences   n=  7388  1R -0.18 [-0.20,-0.15]  2R -0.13 [-0.17,-0.10]  3R -0.15 [-0.19,-0.10]  4R -0.12 [-0.18,-0.07]  ftaR -0.18 [-0.23,-0.14]
GO minus NO-GO at 4R: -0.080R (p=0.04)

## 5. By year: mean net R at 4R, real vs placebo (all fills)
2010  real  -0.03 (n=  519)   placebo  -0.18 (n=  449)   GO  -0.00 (n=77)
2011  real  -0.04 (n=  845)   placebo  -0.24 (n=  724)   GO  -0.25 (n=138)
2012  real   0.02 (n=  812)   placebo  -0.18 (n=  688)   GO   0.25 (n=158)
2013  real  -0.25 (n= 1009)   placebo  -0.35 (n=  860)   GO  -0.33 (n=193)
2014  real  -0.21 (n=  882)   placebo  -0.25 (n=  765)   GO  -0.41 (n=154)
2015  real  -0.16 (n=  811)   placebo  -0.25 (n=  706)   GO  -0.10 (n=148)
2016  real  -0.27 (n=  787)   placebo  -0.39 (n=  678)   GO  -0.26 (n=122)
2017  real  -0.08 (n=  777)   placebo  -0.26 (n=  663)   GO  -0.17 (n=117)
2018  real  -0.09 (n=  770)   placebo  -0.21 (n=  657)   GO  -0.19 (n=97)
2019  real  -0.22 (n=  777)   placebo  -0.30 (n=  666)   GO  -0.33 (n=132)
2020  real   0.07 (n=  749)   placebo  -0.11 (n=  632)   GO  -0.13 (n=109)
2021  real  -0.23 (n=  806)   placebo  -0.29 (n=  709)   GO  -0.13 (n=153)
2022  real  -0.20 (n=  828)   placebo  -0.30 (n=  710)   GO  -0.45 (n=132)
2023  real  -0.42 (n=  854)   placebo  -0.47 (n=  737)   GO  -0.53 (n=119)
2024  real  -0.15 (n=  799)   placebo  -0.19 (n=  687)   GO  -0.30 (n=135)
2025  real  -0.13 (n=  835)   placebo  -0.13 (n=  727)   GO  -0.03 (n=116)
2026  real  -0.09 (n=  473)   placebo  -0.15 (n=  404)   GO  -0.38 (n=55)

## 6. Each rule alone: PASS vs FAIL at 4R, with in-sample / out-of-sample consistency
atr-gate.no-same-direction               pass n=12770 -0.16R | fail n=  563 -0.01R | diff  -0.15 p=0.002 q=0.018  IS -0.06 OOS -0.27
entry.vetoes-stack                       pass n= 2155 -0.22R | fail n=11178 -0.14R | diff  -0.08 p=0.046 q=0.157  IS -0.05 OOS -0.12
retest.needs-space-in-price              pass n=10779 -0.17R | fail n= 2554 -0.09R | diff  -0.07 p=0.054 q=0.157  IS -0.10 OOS -0.06
level.opposite-last                      pass n=13265 -0.15R | fail n=   68 -0.07R | diff  -0.08 p=0.070 q=0.157  IS 0.32 OOS -0.53
retest.never-immediate                   pass n= 7985 -0.18R | fail n= 5348 -0.11R | diff  -0.07 p=0.170 q=0.305  IS -0.09 OOS -0.06
retest.needs-space-in-time               pass n= 8183 -0.17R | fail n= 5150 -0.12R | diff  -0.05 p=0.477 q=0.716  IS -0.00 OOS -0.11
target.minimum-rr                        pass n= 7140 -0.18R | fail n= 6193 -0.12R | diff  -0.06 p=0.625 q=0.803  IS -0.02 OOS -0.10
level.chopping-through                   pass n=10788 -0.16R | fail n= 2545 -0.12R | diff  -0.04 p=0.906 q=0.984  IS -0.04 OOS -0.05
confluence.three-needed                  pass n= 4890 -0.15R | fail n= 8443 -0.15R | diff  -0.00 p=0.984 q=0.984  IS 0.05 OOS -0.06

## 7. The same pipeline on gold with its memory destroyed (block-shuffled by day), 2 seeds
shuffled #101 all fills      n= 13342  1R -0.24 [-0.26,-0.22]  2R -0.20 [-0.22,-0.17]  3R -0.18 [-0.21,-0.14]  4R -0.16 [-0.20,-0.12]  ftaR -0.24 [-0.27,-0.21]
shuffled #101 GO             n=  2023  1R -0.27 [-0.32,-0.22]  2R -0.23 [-0.31,-0.16]  3R -0.18 [-0.29,-0.10]  4R -0.16 [-0.28,-0.07]  ftaR -0.24 [-0.33,-0.13]
   one-at-a-time 4R: 3203 trades, mean -0.211R, $-20884/yr, Sharpe -1.56
shuffled #202 all fills      n= 13382  1R -0.24 [-0.26,-0.22]  2R -0.20 [-0.23,-0.17]  3R -0.18 [-0.22,-0.15]  4R -0.16 [-0.20,-0.12]  ftaR -0.25 [-0.28,-0.21]
shuffled #202 GO             n=  2022  1R -0.28 [-0.33,-0.22]  2R -0.22 [-0.30,-0.14]  3R -0.18 [-0.27,-0.09]  4R -0.14 [-0.24,-0.02]  ftaR -0.25 [-0.34,-0.15]
   one-at-a-time 4R: 3097 trades, mean -0.181R, $-17332/yr, Sharpe -1.29

seconds 205
```
