```
# Backtest of the node graph on SOL/USDT H1, 2021-01-01 to 2026-10-20
Resting limit orders at the last high/low before each flipped level's breakout; filled on touch; stop from the stop node; costs 0.10R per trade; stop wins ties.
Mean net R per trade [95% CI, bootstrapped over days]. 16905 scans, 7276 fills (7101 with a valid stop), 29s.

## 1. Everything the system would fill
all fills                      n= 7101  1R -0.21 [-0.24,-0.19]  2R -0.18 [-0.22,-0.14]  3R -0.15 [-0.20,-0.11]  4R -0.13 [-0.18,-0.08]  ftaR -0.16 [-0.22,-0.09]
  in-sample 2021-23            n= 3650  1R -0.21 [-0.25,-0.17]  2R -0.16 [-0.21,-0.11]  3R -0.12 [-0.20,-0.06]  4R -0.11 [-0.20,-0.04]  ftaR -0.15 [-0.24,-0.07]
  out-of-sample 2024+          n= 3451  1R -0.22 [-0.26,-0.18]  2R -0.20 [-0.25,-0.14]  3R -0.19 [-0.26,-0.11]  4R -0.16 [-0.24,-0.08]  ftaR -0.16 [-0.25,-0.04]

## 2. Against random orders (same side, same distance, same risk, same window)
real levels (all tiers)        n= 7101  1R -0.21 [-0.24,-0.19]  2R -0.18 [-0.22,-0.14]  3R -0.15 [-0.20,-0.11]  4R -0.13 [-0.18,-0.08]  ftaR -0.16 [-0.22,-0.09]
placebo                        n= 6055  1R -0.30 [-0.33,-0.27]  2R -0.28 [-0.32,-0.24]  3R -0.27 [-0.31,-0.22]  4R -0.26 [-0.32,-0.20]  ftaR n/a
real levels, OOS               n= 3451  1R -0.22 [-0.26,-0.18]  2R -0.20 [-0.25,-0.14]  3R -0.19 [-0.26,-0.11]  4R -0.16 [-0.24,-0.08]  ftaR -0.16 [-0.25,-0.04]
placebo, OOS                   n= 2946  1R -0.32 [-0.36,-0.28]  2R -0.31 [-0.36,-0.25]  3R -0.30 [-0.36,-0.23]  4R -0.29 [-0.36,-0.21]  ftaR n/a
real minus placebo: 1R diff 0.085 (p=0.02)   2R diff 0.107 (p=0.00)   3R diff 0.112 (p=0.00)   4R diff 0.124 (p=0.01)   ftaR diff n/a (p=n/a)

## 3. By the system's own verdict and tier
GO (every veto clears)         n= 1109  1R -0.22 [-0.29,-0.16]  2R -0.18 [-0.28,-0.10]  3R -0.11 [-0.23,0.01]  4R -0.05 [-0.19,0.09]  ftaR 0.03 [-0.26,0.34]
NO-GO                          n= 5992  1R -0.21 [-0.24,-0.18]  2R -0.17 [-0.21,-0.13]  3R -0.16 [-0.21,-0.11]  4R -0.15 [-0.21,-0.09]  ftaR -0.19 [-0.25,-0.14]
  GO, in-sample                n=  600  1R -0.21 [-0.31,-0.10]  2R -0.18 [-0.31,-0.04]  3R -0.12 [-0.29,0.05]  4R -0.03 [-0.23,0.21]  ftaR -0.02 [-0.26,0.38]
  GO, out-of-sample            n=  509  1R -0.24 [-0.34,-0.16]  2R -0.20 [-0.33,-0.05]  3R -0.10 [-0.25,0.09]  4R -0.07 [-0.26,0.12]  ftaR 0.08 [-0.33,0.77]
tier blind                     n= 1643  1R -0.23 [-0.28,-0.18]  2R -0.15 [-0.23,-0.07]  3R -0.10 [-0.21,-0.00]  4R -0.07 [-0.19,0.05]  ftaR -0.28 [-0.39,-0.17]
tier needs-one-confluence      n= 1870  1R -0.27 [-0.31,-0.22]  2R -0.20 [-0.26,-0.13]  3R -0.20 [-0.29,-0.12]  4R -0.15 [-0.25,-0.04]  ftaR -0.06 [-0.21,0.10]
tier needs-two-confluences     n= 3588  1R -0.18 [-0.21,-0.14]  2R -0.17 [-0.22,-0.11]  3R -0.15 [-0.22,-0.08]  4R -0.15 [-0.22,-0.07]  ftaR -0.16 [-0.23,-0.08]
GO minus NO-GO at 2R: -0.010 R per trade (p=0.75)

## 4. Each rule on its own: fills that PASS it vs FAIL it, at a 2R target
atr-gate.no-same-direction                   pass n=6807 -0.18R | fail n= 294 -0.08R | diff  -0.10  p=0.02 q=0.09  IS -0.04 OOS -0.17
retest.needs-space-in-price                  pass n=5568 -0.20R | fail n=1533 -0.08R | diff  -0.12  p=0.02 q=0.09  IS -0.25 OOS 0.01
retest.never-immediate                       pass n=4067 -0.20R | fail n=3034 -0.15R | diff  -0.05  p=0.24 q=0.54  IS -0.09 OOS -0.01
target.minimum-rr                            pass n=4023 -0.21R | fail n=3078 -0.14R | diff  -0.07  p=0.27 q=0.54  IS -0.11 OOS -0.03
level.chopping-through                       pass n=5483 -0.18R | fail n=1618 -0.15R | diff  -0.04  p=0.70 q=0.90  IS -0.07 OOS 0.00
entry.vetoes-stack                           pass n=1109 -0.18R | fail n=5992 -0.17R | diff  -0.01  p=0.75 q=0.90  IS -0.02 OOS 0.00
retest.needs-space-in-time                   pass n=4293 -0.17R | fail n=2808 -0.18R | diff   0.01  p=0.86 q=0.90  IS -0.01 OOS 0.02
confluence.three-needed                      pass n=2917 -0.17R | fail n=4184 -0.18R | diff   0.00  p=0.90 q=0.90  IS 0.04 OOS -0.03

## 5. The same pipeline on SOL with its memory destroyed (block-shuffled), 2 seeds
shuffled #1 all fills          n= 7052  1R -0.25 [-0.28,-0.22]  2R -0.21 [-0.24,-0.17]  3R -0.18 [-0.23,-0.14]  4R -0.20 [-0.25,-0.15]  ftaR -0.24 [-0.29,-0.18]
shuffled #1 GO                 n=  987  1R -0.32 [-0.38,-0.25]  2R -0.29 [-0.39,-0.20]  3R -0.28 [-0.40,-0.17]  4R -0.29 [-0.42,-0.16]  ftaR -0.33 [-0.44,-0.20]
shuffled #2 all fills          n= 6976  1R -0.22 [-0.24,-0.18]  2R -0.17 [-0.21,-0.14]  3R -0.16 [-0.21,-0.12]  4R -0.15 [-0.20,-0.10]  ftaR -0.20 [-0.25,-0.15]
shuffled #2 GO                 n=  886  1R -0.21 [-0.29,-0.13]  2R -0.13 [-0.24,-0.02]  3R -0.19 [-0.33,-0.04]  4R -0.14 [-0.31,0.01]  ftaR -0.22 [-0.38,-0.03]
seconds total 92
```
