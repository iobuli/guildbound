# Gold Nucleus observatory

Purpose: represent the market's observed organization, not produce orders or optimize profit.

## Run

From the repository root, with numpy and pandas installed:

```sh
python kev-lab/server.py --port 8766
```

Open the server's `/gold.html` page in a browser. The observation cutoff is UTC.
The existing SOL minute-based Nucleus page remains separate. No minute data or
network feed is required for this gold view.

Export a reproducible observation without running a server:

```sh
python kev-lab/gold_observatory.py --asof 2020-01-02T12:00:00Z --out out/nucleus/gold-2020.json
```

Omitting `--asof` selects the final available close. JSON reports are also available
at `/api/gold/nucleus?asof=2020-01-02T12:00:00Z`. The cache retains only two reports;
restart the server after changing source data.

## What enters

`data/XAUUSD_m30.json` contains 200,000 observed broker M30 OHLC and tick-volume
records. Nucleus consumes timestamp, high, low and close. Open and volume remain
outside this representation. Prices are quantized to the profile's 0.001 tick;
midpoints may fall on half ticks.

These are bar-open timestamps. A record becomes usable at open + 30 minutes.
The input is cut at the requested time **before** aggregation, statistics or
age calculations. A cutoff inside a source bar cannot reveal that bar's high,
low or close. Missing records are never filled or interpolated.

The 30m, 1h, 4h and 1D buckets are UTC aligned, matching Nucleus's existing
aggregation semantics. Daily bars here are not New York-close trading days.
A bucket exists if it contains an observed M30 bar; gaps and a partial first
bucket can leave it incomplete. The smallest measurable changes are M30-close
changes; paths and ordering inside those bars are unknown.

## What it becomes

Each timeframe has n = 9, 26, 52 lines. A forming line contains the n−1 earlier
existing timeframe bars and the portion of its current bar observed so far.
It retains the highest high and lowest low, their midpoint, and normalized price
position. It also describes the previously closed line, midpoint slope, price
against the historical shifted line, and price against a past close.

Shift is a comparison to an already observed historical midpoint, not a future
line value. No visual forward projection supplies observations. Unknowns remain
null until their historical dependencies exist.

The 12 lines yield 72 scalar line readings (position, closed position, closed
midpoint, slope, shift and lag), plus timeframe order/cloud/balance and aggregate
balances. These are overlapping transformations of the same prices, not 72
independent information sources. The midpoint discards much of the price path;
counting derived scalars does not measure net information gain.

Per-line window counts describe the **forming midpoint** only. Shift, lag, closed
lines and clouds may require older history. Counts across lines must not be
summed as unique observations.

## Temporal value

- Window hours versus actual source-bar counts separates elapsed support from
  observed support, including market closures and gaps.
- Age of the latest occurrence of each range extreme shows how old the retained
  boundary observation is. A repeated extreme resets its age.
- Midpoint age and unchanged-observation count measure persistence since the
  first observation of its current run. A fresh midpoint has age zero and one
  observation. Warm-up has no known age. These ages do not estimate expiry.
- The trailing 30-calendar-day joint order state uses four timeframe orders:
  six strict permutations and one tie each. Unknown orders are excluded.
- Empirical state entropy H(S) measures observed variety in bits.
- Conditional entropy H(S_current | S_previous) is computed from empirical pairs
  separated by exactly 30 minutes. Gaps cannot become inferred transitions.
- Current surprise is −log2((earlier_matches + 1)/(earlier_samples + 7^4)),
  using earlier states in the window only. This smoothed descriptive rarity is
  not a calibrated forecast, hypothesis test or evidence of an emergent law.

Entropy and conditional entropy use different sample populations when gaps exist;
do not subtract them to claim mutual information. These plug-in estimates are
sample dependent, biased with sparse states, and affected by overlapping windows.
This view identifies candidates for later investigation. It does not establish
organization beyond suitable null models, irreducible information, profitability
or predictive power. It never examines subsequent returns.

## Validation

```sh
python -m unittest discover -s kev-lab/tests -v
npm run typecheck
npm test
```

Gold tests compare whole report objects from a prefix, a longer input, and an
input with extreme future-price tampering at the same cutoff. They also exercise
bar-close availability, raw-window extrema/counts, gap-aware transitions, timezone
validation and warm-up nulls. Existing Nucleus tests continue to cover the original
SOL implementation; real SOL-minute tests require separate unavailable files.

## Fixed references, n–n–n ranges and temporal events

The `price_references` section extends the observation, rather than turning it
into an execution signal. It includes `references`, `anchors`, `events` and
`observations`. An event's `context_id` joins an observation with all timeframe
orders/balances and each active instance's position, slope, shift, lag and fixed
anchor Fibonacci coordinate.

Each timeframe has independent 9-9-9, 26-26-26 and 52-52-52 instances. In an
equal-period Ichimoku, conversion, base and span-B are the same midpoint, not
three independent measurements. All three named values are retained in each
anchor's `ichimoku` record. Each completed n-bar high/low range becomes a new
**immutable anchor version** when its extrema change. The moving line can keep
changing; the anchor's old prices do not move with it.

Fibonacci references are low + ratio × (high − low), for ratios 0, .236, .382,
.5, .618, .786 and 1. This is a fixed low-to-high coordinate, not an inferred
swing direction. Values outside [0,1] indicate price outside that anchor's range.
Reference prices are rounded to the gold tick (0.001, nearest-even rounding);
the unrounded midpoint is retained separately and can be half a tick.

A reference ID is `XAUUSD:<integer price ticks>`. Different timeframe/instance
origins at the same price share an ID, but retain their anchor IDs, Fibonacci
ratios and availability times. The full ledger counts each fixed-price contact
once, rather than once for every origin. Per-timeframe counts include only origins
known when the contact occurred. Nearby prices remain distinct; no unmentioned
tolerance or clustering is applied.

A contact is an observed M30 high/low range containing an already registered
price, released at that bar's close. Consecutive intersecting bars are a single
episode. An outside bar followed by an intersecting one starts a new episode.
After a missing interval, an intersection starts a new **observed** episode; no
activity inside the gap is inferred. A contact already continuing across the
start of the replay is not mislabelled as a new return. This is a raw contact
measurement, not the existing strategy's stricter swing-reaction touch rule.

Confirmed swings use the repository's three-existing-bars-on-each-side fractal:
strict on the left, non-strict on the right. Confirmation is timestamped when
the third right-hand completed bar becomes available. It is usable for a break
only from the following timeframe bar. The first completed-timeframe close
through the latest confirmed swing is a structure break; it is an **MSB** when
that swing is a lower high for an upward break, or a higher low for a downward
break, matching `src/study/events.ts`. This observation layer does not require
ATR readiness to record structure. Pivots are registered at confirmation, not
retroactively at their historical extremum or only after their eventual break.

Higher-timeframe boundaries missed because of missing source bars become known
at the next observed M30 close. Events carry both `bar_close_at` (nominal TF
boundary) and `observed_at` (availability). Incomplete buckets retain their
actual source-bar count. A not-yet-closed bucket cannot create a frozen anchor
or MSB, even though the original Nucleus forming-line observations remain useful.

The anchor/contact archive currently covers the **trailing 30 calendar days**,
seeded by preceding closed ranges and confirmed pivots. Old versions created
within that replay remain eligible for new contacts even after replacement.
Counts are not lifetime counts, and references older than the seed/archive are
not comprehensively restored. Select a historical cutoff to replay that period;
all event contexts are recomputed using only its closed-data prefix.

The page shows twelve active instances, twenty nearby fixed prices, timeframe
and event filters, the latest sixty matching events and MSB markers on the
recent-history chart. Selecting a price filters its events and draws its fixed
price on the chart when within view, beginning no earlier than its availability.
The JSON export retains the entire scoped archive, events and context joins.
Contacts can be numerous because the reference dictionary becomes dense. Their
raw count is not independent information, significance or emergence.

`reference_created` events distinguish new constant prices from their later
contacts. Contact events include `contact_type`: `first_observed_in_window`,
`return`, or `observed_after_gap`. These describe the replay's observed history,
not a claim that price had never encountered that value before the archive.
The first event at a timestamp is not necessarily the first intrabar action;
multiple events released by one close have no inferred internal ordering.
