# The node graph

23 nodes, 43 wires. Order of evaluation: atr → swing-depth → sfp → tuesday-rto → level-scan → market-side → level → daily-bias → atr-gate → confluence → retest → news → context → stop → target → risk → entry → management → causality → equal-liquidity → framework → htf-patterns → ltf-reversal.

Every wire is checked at load: the producing node must be knowable no later than the consuming node decides. Phases, earliest to latest: prior_bars → bar_open → fill_time → bar_close → post_fill.

```mermaid
flowchart LR
  subgraph D["D · Direction"]
    daily_bias["Higher-timeframe bias (D1 / W1)<br/><i>prior_bars</i>"]
    market_side["Which side of the market is the level on?<br/><i>prior_bars</i>"]
    framework["Direction / Objection / Location / Execution<br/><i>prior_bars</i>"]
    htf_patterns["Higher-timeframe patterns from the journal<br/><i>prior_bars</i>"]
  end
  subgraph O["O · Objection"]
    atr_gate["Daily ATR gate<br/><i>fill_time</i>"]
    target["Targets and first trouble area<br/><i>prior_bars</i>"]
    equal_liquidity["Equal highs / lows<br/><i>prior_bars</i>"]
  end
  subgraph L["L · Location"]
    swing_depth["Swing depth (swing in / swing out)<br/><i>prior_bars</i>"]
    sfp["Swing failure pattern (SFP)<br/><i>prior_bars</i>"]
    level_scan["Level scanner<br/><i>prior_bars</i>"]
    level["Support / resistance level<br/><i>prior_bars</i>"]
    retest["Retest quality and space<br/><i>prior_bars</i>"]
    confluence["Three-confluence rule<br/><i>fill_time</i>"]
  end
  subgraph E["E · Execution"]
    tuesday_rto["Tuesday weekly-open retest<br/><i>prior_bars</i>"]
    stop["Stop placement<br/><i>prior_bars</i>"]
    entry["Entry decision<br/><i>fill_time</i>"]
    ltf_reversal["M5 failed-breakdown reversal (Bund)<br/><i>prior_bars</i>"]
  end
  subgraph M["M · Management"]
    management["Trade management<br/><i>post_fill</i>"]
  end
  subgraph R["R · Risk"]
    risk["Risk and sizing<br/><i>fill_time</i>"]
  end
  subgraph X["X · Context"]
    atr["ATR yardsticks<br/><i>prior_bars</i>"]
    news["News handling<br/><i>fill_time</i>"]
    context["Time, session and mode<br/><i>prior_bars</i>"]
    causality["Causality lessons from the backtest<br/><i>prior_bars</i>"]
  end
  atr --> swing_depth
  atr --> sfp
  swing_depth --> sfp
  sfp --> daily_bias
  atr --> atr_gate
  atr --> level
  atr --> level_scan
  swing_depth --> level_scan
  market_side --> entry
  level --> retest
  atr --> retest
  daily_bias --> retest
  level --> confluence
  daily_bias --> confluence
  atr_gate --> confluence
  atr --> confluence
  swing_depth --> confluence
  atr --> news
  daily_bias --> context
  level --> stop
  atr --> stop
  swing_depth --> stop
  sfp --> stop
  tuesday_rto --> stop
  stop --> target
  level --> target
  swing_depth --> target
  daily_bias --> target
  atr --> target
  tuesday_rto --> target
  level --> entry
  retest --> entry
  confluence --> entry
  atr_gate --> entry
  news --> entry
  sfp --> entry
  daily_bias --> entry
  tuesday_rto --> entry
  context --> entry
  stop --> risk
  stop --> management
  target --> management
  context --> management
```

Rule-only cards (no wires): `framework`, `causality`, `htf-patterns`, `equal-liquidity`, `ltf-reversal`
