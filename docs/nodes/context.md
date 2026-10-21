# Time, session and mode

`context` · stage **X** · decides at **prior_bars** · computed

Works out the session, whether it is a late-Friday or flatten-by-close situation, and whether the trade is a with-bias trade or a quick no-bias scalp.

## Ports

**Inputs**
- `bias` (from daily-bias.bias): d1: object, w1: object, htf: long|short|neutral

**Outputs**
- `context`: weekday: number, hourUtc: number, session: asian|london|ny|off, fridayLate: boolean, flattenBy: none|end-of-week|end-of-day, mode: with-bias|no-bias, conservativeTargets: boolean

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **context.friday-afternoon**<br>Context matters: at 2pm on a Friday price is not going to move much, so without a strong bias targets should be conservative.<br>*Treated as Friday from 13:00 UTC; the source gives only "2pm".* | hard | stated | yes | webinar: user-pasted notes (2024-09-24) |
| **context.flatten-by-close**<br>Forex positions are closed at the end of the week. FTSE and Bund trades are closed at the end of the day. | hard | stated | yes | webinar: user-pasted notes (2024-09-24) |
| **context.no-bias-means-quick**<br>Without a daily bias the trade is in and out quickly. 80% of his trades are like this at 0.8-1R on average. | hard | stated | yes | webinar: user-pasted notes (2024-09-24) |
| **context.crosspairs-squeeze**<br>Be careful on cross pairs (anything not paired with USD). They seem to squeeze more, so the stop may need to be safer. | hard | stated | no | webinar: user-pasted notes (2024-09-25)<br>webinar: curated/notes-images.md (2024-09-23) |
| **context.ltf-against-htf**<br>Do not trade M5 against the H1 bias, just as H1 against the D1 bias is avoided. | hard | stated | no | webinar: user-pasted notes (2024-10-18) |
| **context.markets-traded**<br>Markets traded: forex majors and crosses (GBPUSD, EURUSD, USDCHF, USDCAD, USDJPY, NZDUSD, AUDUSD, EURCHF, EURGBP, EURJPY, GBPJPY), Bund, spot gold and silver, crude oil, DAX and SPX futures. Later posts also mention Eurostoxx 50, ES and silver. | hard | observed | no | webinar: curated/notes-images.md (2024-09-30)<br>tweet: curated tweets (2026-09-21) |
| **context.crypto-not-covered**<br>None of the sources cover crypto. Applying the method to SOL assumes forex-style behaviour carries over.<br>*This is the biggest untested assumption in the system.* | heuristic | inferred | no | webinar: curated/notes-images.md (2024-09-30) |
