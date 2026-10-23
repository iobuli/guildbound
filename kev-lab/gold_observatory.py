"""Gold Nucleus observations. Every report is computed from a closed-bar prefix only."""
from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from functools import lru_cache
from pathlib import Path

import numpy as np
import nucleus as N
from gold_references import reference_report

BASE = 1_800_000
DAY = 86_400_000
TFS = {"30m": BASE, "1h": 2 * BASE, "4h": 8 * BASE, "1D": DAY}
SOURCE = Path(__file__).resolve().parents[1] / "data" / "XAUUSD_m30.json"
TICK = 0.001


def iso(t):
    return datetime.fromtimestamp(int(t) / 1000, timezone.utc).isoformat().replace('+00:00', 'Z')


def parse_asof(value):
    if not value:
        return None
    dt = datetime.fromisoformat(value.replace('Z', '+00:00'))
    if dt.tzinfo is None:
        raise ValueError('asof must include a timezone, e.g. 2020-01-01T12:00:00Z')
    return int(dt.timestamp() * 1000)


@lru_cache(maxsize=1)
def load_gold():
    rows = json.loads(SOURCE.read_text())
    m = {'t': np.array([r['t'] for r in rows], dtype=np.int64)}
    for k in 'hlc':
        prices = np.array([r[k] for r in rows], dtype=float)
        if not np.isfinite(prices).all():
            raise ValueError('nonfinite gold prices')
        m[k] = np.rint(prices / TICK).astype(np.int64)
    if not len(m['t']) or np.any(np.diff(m['t']) <= 0) or np.any(m['t'] % BASE):
        raise ValueError('gold bars must have unique, increasing, M30-aligned open times')
    if np.any(m['h'] < m['l']) or np.any(m['c'] > m['h']) or np.any(m['c'] < m['l']):
        raise ValueError('invalid gold high/low/close')
    return m


def entropy(codes):
    if not len(codes):
        return None
    _, counts = np.unique(codes, return_counts=True, axis=0)
    p = counts / counts.sum()
    return float(-np.sum(p * np.log2(p)))


def temporal(codes, times):
    """Descriptive plug-in estimates; transitions only across adjacent observed M30 bars."""
    eligible = np.diff(times) == BASE
    previous, current = codes[:-1][eligible], codes[1:][eligible]
    h = entropy(codes)
    conditional = None
    if len(previous):
        conditional = max(0.0, entropy(np.column_stack([previous, current])) - entropy(previous))
    prior = codes[:-1]
    matches = int(np.all(prior == codes[-1], axis=1).sum()) if len(prior) else 0
    # Fixed alphabet of seven order states per timeframe (six strict permutations + tie).
    probability = (matches + 1) / (len(prior) + 7 ** codes.shape[1])
    run_start = len(codes) - 1
    while run_start > 0 and times[run_start] - times[run_start - 1] == BASE and np.array_equal(codes[run_start - 1], codes[-1]):
        run_start -= 1
    return {'samples': len(codes), 'adjacent_transitions': len(previous),
            'unique_states': len(np.unique(codes, axis=0)), 'state_entropy_bits': h,
            'conditional_entropy_bits': conditional,
            'current_prior_occurrences': matches,
            'current_surprise_bits': float(-np.log2(probability)),
            'current_run_observations': len(codes) - run_start,
            'current_run_hours': float((times[-1] - times[run_start]) / 3_600_000),
            'changed_fraction': float(np.mean(np.any(previous != current, axis=1))) if len(previous) else None}


def number(value):
    return float(value) if np.isfinite(value) else None


def report(m, asof=None):
    end = len(m['t']) if asof is None else int(np.searchsorted(m['t'] + BASE, asof, side='right'))
    if not end:
        raise ValueError('no closed gold bars available at this time')
    # Cut before any aggregation, extrema, state counts or statistics are computed.
    m = {k: v[:end] for k, v in m.items()}
    nu = N.nucleus(m, TFS)
    references = reference_report(m, nu, TFS, N.NS, BASE, TICK, iso)
    now = int(m['t'][-1] + BASE)
    lines = []
    for name, size in TFS.items():
        tf = N.timeframe(m, size)
        _, starts = np.unique(m['t'] // size, return_index=True)
        for n in N.NS:
            L = N.line(tf, m['c'].astype(float), n)
            mid = L['mid']
            known = bool(np.isfinite(mid[-1]))
            first = int(starts[max(0, len(starts) - n)])
            change = np.flatnonzero((~np.isfinite(mid[:-1])) | (mid[1:] != mid[:-1])) + 1
            since = int(change[-1]) if known and len(change) else 0
            high = int(m['h'][first:].max())
            low = int(m['l'][first:].min())
            hi_at = first + int(np.flatnonzero(m['h'][first:] == high)[-1])
            lo_at = first + int(np.flatnonzero(m['l'][first:] == low)[-1])
            lines.append({'tf': name, 'n': n, 'known': known,
                          'observed_tf_bars': min(n, len(starts)),
                          'input_bars': end - first, 'input_hlc_values': (end - first) * 3,
                          'window_from': iso(m['t'][first]), 'window_hours': (now - int(m['t'][first])) / 3_600_000,
                          'mid': number(mid[-1] * TICK), 'pos': number(L['pos'][-1]),
                          'closed_mid': number(L['cmid'][-1] * TICK), 'closed_pos': number(L['cpos'][-1]),
                          **{k: int(nu[f'{name}.{n}.{k}'][-1]) if nu[f'{name}.{n}.{k}'][-1] != N.MISSING else None for k in ('slope', 'shift', 'lag')},
                          'range_high': high * TICK if known else None, 'range_low': low * TICK if known else None,
                          'high_age_hours': (now - int(m['t'][hi_at] + BASE)) / 3_600_000 if known else None,
                          'low_age_hours': (now - int(m['t'][lo_at] + BASE)) / 3_600_000 if known else None,
                          'mid_unchanged_observations': end - since if known else None,
                          'mid_unchanged_hours': (now - int(m['t'][since] + BASE)) / 3_600_000 if known else None})
    window = int(np.searchsorted(m['t'] + BASE, now - 30 * DAY, side='right'))
    codes = np.column_stack([nu[f'{tf}.order'] for tf in TFS])
    valid = np.all(codes != -1, axis=1)
    select = valid & (np.arange(end) >= window)
    stats = temporal(codes[select], (m['t'] + BASE)[select]) if select.any() else None
    timeline = []
    for i in range(max(0, end - 240), end):
        timeline.append({'at': iso(m['t'][i] + BASE), 'close': float(m['c'][i] * TICK),
                         'balance': number(nu['bal'][i]), 'valid': bool(nu['valid'][i]),
                         'orders': [int(nu[f'{tf}.order'][i]) for tf in TFS],
                         'balances': [number(nu[f'{tf}.bal'][i]) for tf in TFS]})
    return {'symbol': 'XAUUSD', 'asof': iso(now), 'requested_asof': iso(asof) if asof is not None else None,
            'source': 'data/XAUUSD_m30.json', 'source_resolution_minutes': 30,
            'first_open': iso(m['t'][0]), 'observed_bars': end, 'hlc_values_seen': end * 3,
            'elapsed_hours': (now - int(m['t'][0])) / 3_600_000,
            'observed_hours': end / 2, 'unobserved_half_hour_slots': int((now - m['t'][0]) // BASE) - end,
            'close': float(m['c'][-1] * TICK), 'valid': bool(nu['valid'][-1]),
            'timeframes': list(TFS), 'line_count': len(lines), 'lines': lines,
            'timeframe_states': [{'tf': tf,
                                  'order': 'unknown' if nu[f'{tf}.order'][-1] == -1 else 'tie' if nu[f'{tf}.order'][-1] == N.TIE else N.ORDERS[int(nu[f'{tf}.order'][-1])],
                                  'cloud': int(nu[f'{tf}.cloud'][-1]) if nu[f'{tf}.cloud'][-1] != N.MISSING else None,
                                  'balance': number(nu[f'{tf}.bal'][-1])} for tf in TFS],
            'representation': {'line_readings': ['pos', 'closed_pos', 'closed_mid', 'slope', 'shift', 'lag'],
                               'line_scalar_slots': len(lines) * 6,
                               'note': 'Derived, overlapping measurements, not independent information. H/L extrema compress each window; order and most of its path are discarded.'},
            'temporal_window': {'from': iso(now - 30 * DAY), 'to': iso(now), 'stats': stats},
            'timeline': timeline,
            'price_references': references,
            'method': ['Every M30 OHLC bar becomes available at open + 30 minutes. Only H/L/C and timestamps enter Nucleus; open and tick volume are not consumed.',
                       'UTC-aligned 30m/1h/4h/1D buckets. Higher-timeframe bars include only the M30 bars observed so far. Days are UTC, not broker or New York trading days.',
                       'No resampling to invented minutes; missing slots stay missing. A partial first bucket or gaps may leave a timeframe bar incomplete.',
                       'Shift and lag compare current price with historical lines and closes; no future-price comparisons. Unknown values remain null.',
                       'Age is elapsed wall time since last observed change/extreme; observation counts exclude gaps. It is not an expiry time or forecast horizon.',
                       'Entropy describes the joint four-timeframe line-order states over the trailing 30 calendar days. Conditional entropy uses adjacent M30 observations only; gaps are excluded.',
                       'Surprise uses only earlier states in that window, add-one smoothing over 7^4 states. Sparse-sample estimates and overlapping windows can create apparent structure. No null-model significance or predictive value is claimed.']}


@lru_cache(maxsize=2)
def gold_report(asof=None):
    return report(load_gold(), asof)


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--asof', help='Timezone-aware observation cutoff; defaults to the final available close')
    ap.add_argument('--out', type=Path, help='Optional JSON output path')
    args = ap.parse_args()
    result = gold_report(parse_asof(args.asof))
    body = json.dumps(result, allow_nan=False, indent=2)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(body + '\n')
    else:
        print(body)
