"""Immutable price references, Ichimoku range versions, contacts and causal MSBs."""
from collections import defaultdict

import numpy as np

FIBS = (0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0)
FRACTAL_N = 3  # Same left-strict/right-nonstrict confirmation as core/ta.ts.


def closed_bars(m, size, base):
    ids = m['t'] // size
    starts = np.r_[0, np.flatnonzero(np.diff(ids)) + 1]
    ends = np.r_[starts[1:] - 1, len(ids) - 1]
    close_at = (ids[starts] + 1) * size
    available = np.searchsorted(m['t'] + base, close_at, side='left')
    keep = available < len(ids)
    return {'t': m['t'][starts][keep], 'end': close_at[keep], 'at': available[keep],
            'h': np.maximum.reduceat(m['h'], starts)[keep],
            'l': np.minimum.reduceat(m['l'], starts)[keep], 'c': m['c'][ends][keep],
            'count': (ends - starts + 1)[keep]}


def structure_events(bars, tf, iso, start_index, radius=FRACTAL_N):
    """Consume completed bars; a confirmed pivot is usable only from the next TF bar."""
    highs, lows, used = [], [], {'high': None, 'low': None}
    out, seeds = [], {}
    for j in range(len(bars['t'])):
        for side, pivots, direction in [('high', highs, 'up'), ('low', lows, 'down')]:
            if not pivots:
                continue
            pivot = pivots[-1]
            crosses = bars['c'][j] > pivot['ticks'] if side == 'high' else bars['c'][j] < pivot['ticks']
            if crosses and used[side] != pivot['j']:
                used[side] = pivot['j']
                if bars['at'][j] >= start_index:
                    previous = pivots[-2] if len(pivots) > 1 else None
                    msb = previous is not None and (pivot['ticks'] < previous['ticks'] if side == 'high' else pivot['ticks'] > previous['ticks'])
                    out.append({'kind': 'msb' if msb else 'structure_break', 'tf': tf,
                                'direction': direction, 'ticks': pivot['ticks'], 'at': int(bars['at'][j]),
                                'pivot_at': iso(bars['t'][pivot['j']]),
                                'confirmed_bar_close_at': iso(bars['end'][pivot['confirmed_j']]),
                                'confirmed_index': int(bars['at'][pivot['confirmed_j']]),
                                'bar_close_at': iso(bars['end'][j]), 'tf_source_bars': int(bars['count'][j])})
        p = j - radius
        if p < radius:
            continue
        for side, pivots, key in [('high', highs, 'h'), ('low', lows, 'l')]:
            x = bars[key][p]
            left, right = bars[key][p-radius:p], bars[key][p+1:j+1]
            is_pivot = (np.all(x > left) and np.all(x >= right)) if side == 'high' else (np.all(x < left) and np.all(x <= right))
            if is_pivot:
                pivots.append({'j': p, 'ticks': int(x), 'confirmed_j': j})
                event = {'kind': 'pivot_confirmed', 'tf': tf, 'direction': side,
                         'ticks': int(x), 'at': int(bars['at'][j]),
                         'pivot_at': iso(bars['t'][p]), 'confirmed_index': int(bars['at'][j]),
                         'confirmed_bar_close_at': iso(bars['end'][j]),
                         'bar_close_at': iso(bars['end'][j]), 'tf_source_bars': int(bars['count'][j])}
                if bars['at'][j] >= start_index:
                    out.append(event)
                else:
                    seeds[side] = dict(event, kind='pivot_seed', at=start_index)
                if len(pivots) > 2:
                    del pivots[0]
    return list(seeds.values()) + out


def reference_report(m, nu, tfs, periods, base, tick, iso, days=30):
    now = int(m['t'][-1] + base)
    start = int(np.searchsorted(m['t'] + base, now - days * 86_400_000, side='right'))
    start = min(start, len(m['t']) - 1)
    scheduled_anchors, scheduled_breaks = defaultdict(list), defaultdict(list)
    for tf, size in tfs.items():
        b = closed_bars(m, size, base)
        for event in structure_events(b, tf, iso, start):
            scheduled_breaks[event['at']].append(event)
        for n in periods:
            if len(b['t']) < n:
                continue
            highs = np.lib.stride_tricks.sliding_window_view(b['h'], n).max(axis=1)
            lows = np.lib.stride_tricks.sliding_window_view(b['l'], n).min(axis=1)
            changes = np.r_[n-1, np.flatnonzero((np.diff(highs) != 0) | (np.diff(lows) != 0)) + n]
            prior = changes[b['at'][changes] < start]
            selected = np.r_[prior[-1:], changes[b['at'][changes] >= start]]
            for j in selected:
                high, low = int(highs[j-n+1]), int(lows[j-n+1])
                # The preceding closed range seeds references known before the replay window.
                at = max(start, int(b['at'][j]))
                scheduled_anchors[at].append({'id': f'{tf}:{n}:{int(b["t"][j])}',
                                             'tf': tf, 'n': n, 'nnn': [n, n, n],
                                             'known_at': iso(m['t'][b['at'][j]] + base),
                                             'source_close_at': iso(b['end'][j]),
                                             'source_bars': int(b['count'][j-n+1:j+1].sum()),
                                             'low_ticks': low, 'high_ticks': high,
                                             'low': low * tick, 'mid': (low + high) / 2 * tick, 'high': high * tick,
                                             'ichimoku': {'conversion': (low+high)/2*tick,
                                                          'base': (low+high)/2*tick, 'span_b': (low+high)/2*tick}})
    refs, bins, anchors, current = {}, defaultdict(dict), {}, {}
    events, observations = [], []
    event_counts = defaultdict(int)

    def reference(ticks, at, origin):
        ticks = int(ticks)
        rid = f'XAUUSD:{ticks}'
        if rid not in refs:
            prior_contact = (start > 0 and at < iso(m['t'][start-1] + base)
                             and m['l'][start-1] <= ticks <= m['h'][start-1])
            refs[rid] = {'id': rid, 'ticks': ticks, 'price': ticks * tick,
                         'known_at': at, 'origins': [], 'contacts': 0, 'touches_by_tf': {},
                         'last_contact_at': None, 'carried_contact_from_before_window': bool(prior_contact),
                         'last_hit_index': start-1 if prior_contact else None}
            bins[ticks // 10_000][ticks] = refs[rid]
            if at >= iso(m['t'][start] + base):
                events.append({'kind': 'reference_created', 'observed_at': at, 'context_id': at,
                               'reference_id': rid, 'price': ticks * tick, 'tf': origin['tf']})
                event_counts['reference_created'] += 1
        ref = refs[rid]
        ref['origins'].append(origin)
        if at < ref['known_at']:
            ref['known_at'] = at
        return ref

    def register(anchor):
        anchor['fib_levels'] = []
        for ratio in FIBS:
            ticks = int(np.rint(anchor['low_ticks'] + ratio * (anchor['high_ticks'] - anchor['low_ticks'])))
            ref = reference(ticks, anchor['known_at'], {'anchor_id': anchor['id'], 'tf': anchor['tf'],
                                                     'n': anchor['n'], 'ratio': ratio, 'known_at': anchor['known_at']})
            anchor['fib_levels'].append({'ratio': ratio, 'reference_id': ref['id'], 'price': ref['price']})
        anchors[anchor['id']] = anchor
        current[(anchor['tf'], anchor['n'])] = anchor['id']

    for i in range(start, len(m['t'])):
        at = iso(m['t'][i] + base)
        pending = scheduled_anchors[i]
        for anchor in pending:
            if anchor['known_at'] < at:
                register(anchor)
        for event in scheduled_breaks[i]:
            if event['kind'] == 'pivot_seed':
                known = iso(m['t'][event['confirmed_index']] + base)
                reference(event['ticks'], known, {'tf': event['tf'], 'kind': 'confirmed_pivot',
                                                 'pivot_at': event['pivot_at'], 'known_at': known})
        low, high = int(m['l'][i]), int(m['h'][i])
        touched = []
        for bucket in range(low // 10_000, high // 10_000 + 1):
            for price, ref in bins.get(bucket, {}).items():
                if low <= price <= high:
                    last = ref['last_hit_index']
                    continuation = last == i - 1 and m['t'][i] - m['t'][i-1] == base
                    ref['last_hit_index'] = i
                    if continuation:
                        continue
                    scopes = sorted({o['tf'] for o in ref['origins'] if o['known_at'] < at})
                    ref['contacts'] += 1
                    ref['last_contact_at'] = at
                    for tf in scopes:
                        ref['touches_by_tf'][tf] = ref['touches_by_tf'].get(tf, 0) + 1
                    event = {'kind': 'contact', 'observed_at': at, 'context_id': at,
                             'reference_id': ref['id'], 'price': ref['price'], 'timeframes': scopes,
                             'episode': ref['contacts'], 'after_gap': bool(i > 0 and m['t'][i] - m['t'][i-1] != base)}
                    event['contact_type'] = ('observed_after_gap' if event['after_gap'] else
                                             'return' if last is not None else 'first_observed_in_window')
                    events.append(event)
                    event_counts['contact'] += 1
                    touched.append(ref['id'])
        for event in scheduled_breaks[i]:
            known = iso(m['t'][event['confirmed_index']] + base)
            if event['kind'] == 'pivot_seed':
                continue
            if event['kind'] == 'pivot_confirmed':
                ref = reference(event['ticks'], known, {'tf': event['tf'], 'kind': 'confirmed_pivot',
                                                      'pivot_at': event['pivot_at'], 'known_at': known})
            else:
                ref = refs[f'XAUUSD:{event["ticks"]}']
            payload = {k: v for k, v in event.items() if k not in ('ticks', 'at', 'confirmed_index')}
            payload.update(observed_at=at, context_id=at, reference_id=ref['id'], price=ref['price'], confirmed_at=known)
            events.append(payload)
            event_counts[event['kind']] += 1
        for anchor in pending:
            if anchor['known_at'] == at:
                register(anchor)
        instance_states = []
        for (tf, n), aid in sorted(current.items()):
            a = anchors[aid]
            span = a['high_ticks'] - a['low_ticks']
            instance_states.append({'tf': tf, 'n': n, 'anchor_id': aid,
                                    'fib_position': (int(m['c'][i]) - a['low_ticks']) / span if span else None,
                                    'pos': float(nu[f'{tf}.{n}.pos'][i]) if np.isfinite(nu[f'{tf}.{n}.pos'][i]) else None,
                                    **{k: int(nu[f'{tf}.{n}.{k}'][i]) if nu[f'{tf}.{n}.{k}'][i] != -9 else None for k in ('slope','shift','lag')}})
        observations.append({'id': at, 'at': at, 'close': float(m['c'][i] * tick),
                             'orders': [int(nu[f'{tf}.order'][i]) for tf in tfs],
                             'balances': [float(nu[f'{tf}.bal'][i]) if np.isfinite(nu[f'{tf}.bal'][i]) else None for tf in tfs],
                             'instances': instance_states, 'new_contacts': touched})
    for ref in refs.values():
        del ref['last_hit_index']
    return {'scope': {'from': iso(m['t'][start] + base), 'to': iso(now), 'days': days,
                       'note': 'Contact counts and anchor archive cover this trailing replay only, not lifetime history. Prior closed ranges seed the window.'},
            'definition': {'touch': 'Exact fixed-price inclusion in an observed M30 high-low range, released at close. Consecutive intersecting bars form one episode; a gap starts a new observed episode. No claim about intrabar order or a confirmed swing reaction.',
                           'msb': 'First completed-timeframe close through the last confirmed lower high (up) or higher low (down). Fractals use 3 existing bars on each side; pivot usable from the following TF bar.',
                           'fib': 'Low-to-high coordinate of a frozen closed Ichimoku range; 0, .236, .382, .5, .618, .786, 1. References quantized to 0.001; raw midpoint can be half-tick.',
                           'nnn': 'Independent 9-9-9, 26-26-26 and 52-52-52 instances per timeframe. Equal-period conversion/base/span-B coincide; these are not three independent values.',
                           'availability': 'Only closed TF bars create anchors; their own source bars cannot count as subsequent touches. Missed TF boundaries become known at the next available source close, never backdated.'},
            'event_counts': dict(event_counts), 'reference_count': len(refs), 'anchor_count': len(anchors),
            'references': list(refs.values()), 'anchors': list(anchors.values()),
            'current_anchor_ids': list(current.values()), 'observations': observations, 'events': events}
