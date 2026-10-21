# Swing depth (swing in / swing out) (`swing-depth`)

Lists swing highs and lows and measures how "deep" each is, so only significant swings can be swept. Depth is the smaller of the swing-in and swing-out moves, compared with the daily ATR.

Decides at **prior_bars**, step 0.1 of the order of operations: Swing depth. Inputs: atr. Outputs: swings.

## swing-depth.in-out-definition

Draw a line at the swing price, run it left until it hits a candle and right until the sweep. The lowest (highest) point of each section is the swing in / swing out. Measure the SMALLER side.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-h1-sweep.md (2024-10-22); webinar curated/advanced-sfp.md (2024-09-24).

Supporting passages:
- [sources/course/curated/taking-a-trade-h1-sweep.md:1](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 53.67): "one all the way to the left (until you hit a candle) - this is the green line on the chart.and one all the way to the right (until you hit your sweep) - this is the orange line on the chart. [image]"
- [sources/course/curated/advanced-sfp.md:19](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 51.01): "you are going to draw the lowest point of each section: [image]"
- [sources/course/curated/taking-a-trade-h1-sweep.md:36](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 36.13): "but when we move to measure the right side - the highest point price reached (before the sweep) was only 250 pipettes tall. [image]"

## swing-depth.deep-threshold

An H1 swing is deep at 1x ATR or more; a D1 swing needs 3x the current day's ATR. Anything smaller is ignored when it is swept.

*hard, stated, tested by code.* Sources: webinar curated/advanced-sfp.md (2024-09-24); lesson curated/taking-a-trade-h1-sweep.md (2024-10-22). Note: Worked examples: 57.5 against an ATR of 66.8 is NOT deep; 1018 against 568 pipettes is.

Supporting passages:
- [sources/course/curated/advanced-sfp.md:19](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 40.58): "for a D1 swing to be considered 'deep' it must be 3x the size of the current day's ATR. [image]"
- [sources/course/curated/taking-a-trade-h1-sweep.md:1](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 28.46): "well for that, we use the current D1 ATR value, and the concept of 'swing in' and 'swing out'."
- [sources/course/curated/taking-a-trade-h1-sweep.md:36](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 22.66): "remembering that we need to measure these swing points against the Daily ATR, we first take our reading - and see that it is 568 pipettes. [image]"

## swing-depth.where-do-people-get-bearish

Ask where people will get bearish (bullish): only the main swing counts, not a tiny low sitting under it.

*heuristic, stated, not coded.* Sources: webinar curated/advanced-sfp.md (2024-09-24).

Supporting passages:
- [sources/course/curated/advanced-sfp.md:61](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 21.12): "just consider "where are people going to get bearish?" are they going to be bearish under this little low here? no. they'd more likely be bearish under the main low slightly to the left, or even the low before that."
- [sources/course/curated/tp-selection.md:1](../sources/course/curated/tp-selection.md) (tp-selection, score 16.27): "his target for this bearish setup is all the way down here, at the main low. none of the minor lows along the way. [image]"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 14.8): "* For good sweeps, focus on “which swing high would people get bullish on the break of?”"

## swing-depth.shallow-swing-above-deep

A deep swing can be ignored if another deep swing sits right above it, since that is easy extra liquidity. A recent, prominent high close by can also become the place people get bullish instead.

*heuristic, inferred, not coded.* Sources: webinar curated/advanced-sfp.md (2024-09-24). Note: The note-taker marked this SPECULATION.

Supporting passages:
- [sources/course/curated/advanced-sfp.md:61](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 34.78): "i guess if theres a h1 high, even a fairly deep one, it can be kind of ignored if there is another deep swing high right above it. which makes sense i guess - because thats just easy extra liquidity to grab if its right there, really close."
- [sources/course/curated/advanced-sfp.md:77](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 30.44): "i think the reason is ''you have to ask yourself, where are these people gonna get bullish?" - and if there is a recent high close by, itmight just be there that they get bullish, instead of the 1st (slightly lower) level."
- [sources/course/curated/tp-selection.md:39](../sources/course/curated/tp-selection.md) (tp-selection, score 22.47): "you can see the long wicks on the left where price started to form resistance.then a swing high as it tapped into the area.then it flipped to support when price got above the line. [image]"

## Conflicts

- `swing-depth.in-out-definition` vs `swing-depth.deep-threshold`: The notes say an H1 swing must be "1x ATR" without saying which ATR; the lesson examples measure it against the DAILY ATR. This build uses the D1 ATR for both (x1 for H1, x3 for D1).
