# Daily ATR gate

`atr-gate` · stage **O** · decides at **fill_time** · computed

Measures how far price has travelled from the HIGH or LOW OF THE DAY, in daily ATRs. If it has fallen an ATR from the day's high, selling is vetoed and buying gains a confluence (and the reverse).

## Ports

**Inputs**
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number

**Outputs**
- `gate`: movedAtr: number, fromHighAtr: number, fromLowAtr: number, passedUp: boolean, passedDown: boolean, blocks: object, supports: object

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **atr-gate.measure-from-the-days-extreme**<br>The ATR level is measured from the HIGH (or low) OF THE DAY: with a 123-pip ATR and a day high of 128.31, the market has reached its ATR at 127.08. If it falls 101 pips from the high of the day into the level, that counts. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @45:00<br>transcript: levels webinar part 1, user-pasted transcript @46:09<br>transcript: levels webinar part 1, user-pasted transcript @1:19:57 |
| **atr-gate.surpassed-atr-is-confluence**<br>A market that is at or past its daily ATR coming into the level is a confluence: it has moved what it normally moves, so people take profits there and it is likely to turn. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @34:41<br>transcript: levels webinar part 1, user-pasted transcript @45:17<br>transcript: levels webinar part 1, user-pasted transcript @1:16:27<br>lesson: curated/taking-a-trade-1-touch-level.md (2024-10-21) |
| **atr-gate.no-same-direction**<br>Do not trade in the same direction once the market has passed its daily ATR. "70 points down on the day and the ATR is 70, I'm not selling, even on a rounded retest of the mother of all levels." | hard | stated | yes | webinar: user-pasted notes (2024-10-03) |
| **atr-gate.close-enough**<br>Being "close enough" to the ATR counts. In the EURCHF example the ATR line sat 16 pips below the level and still counted, because it matched the lower part of the level zone.<br>*This build treats 90% of the ATR as close enough; the sources give no number.* | hard | stated | yes | webinar: user-pasted notes (2024-09-24)<br>transcript: levels webinar part 1, user-pasted transcript @47:01 |
| **atr-gate.atr-trades**<br>In the passed-ATR trades of the later journal he cares less about a weekly level sitting in the way, and a daily level is not needed if the H1 level is clean. | heuristic | observed | no | journal: curated/lxpb-trades-passed-atr.md (2025-01-03)<br>journal: curated/lxpb-trades-passed-atr.md (2026-02-09) |
