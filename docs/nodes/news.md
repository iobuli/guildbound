# News handling

`news` · stage **X** · decides at **fill_time** · computed

Keeps resting orders from filling into red news: no fills 15 minutes either side, pull a limit if price is close to it, leave it if price is far away, and hold anything already open.

## Ports

**Inputs**
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number

**Outputs**
- `news`: redNow: boolean, minutesToNextRed: number,null, minutesSinceLastRed: number,null, noNewFills: boolean, action: ok|pull-limit|keep-limit|reinsert-later, holdIfInTrade: boolean

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **news.no-fills-window**<br>Red-folder news: no trading 15 minutes before or after. | hard | stated | yes | webinar: user-pasted notes (2024-10-03) |
| **news.hold-open-trades**<br>If already in a trade at the release, hold it. Do not lighten up or cut; keep the stop where the technicals put it. | hard | stated | yes | webinar: user-pasted notes (2024-09-24)<br>webinar: user-pasted notes (2024-10-03) |
| **news.pull-limit-when-close**<br>If price is close to the level at the release (about 10-20 pips on EURUSD), take the order out and put it back after the news. 2012 webinar: give the news five minutes. 2024 notes: about ten. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @1:18:01<br>webinar: curated/notes-images.md (2024-09-19)<br>webinar: user-pasted notes (2024-09-24) |
| **news.leave-it-when-far**<br>If price is far away (about 60 pips on big news), do not remove the limit. At 20+ pips away the news may even be what tags you in with a retest. | hard | stated | yes | webinar: curated/notes-images.md (2024-09-19)<br>webinar: user-pasted notes (2024-10-03) |
| **news.no-m5-through-news**<br>He does not like holding an M5 trade through news. | hard | stated | no | webinar: user-pasted notes (2024-10-18) |
| **news.non-fx-calibration**<br>Pip thresholds only make sense for forex. For other instruments this build substitutes ATR multiples (pull within 1 H1 ATR, leave beyond 3). | heuristic | inferred | yes | webinar: curated/notes-images.md (2024-09-19) |

## Conflicts

- `news.pull-limit-when-close` vs `news.no-fills-window`: The 2012 webinar reinserts the order five minutes after the release; the 2024 notes say ten; the red-news window itself is fifteen minutes either side. This build uses five for "reinsert" and fifteen for "no fills".
