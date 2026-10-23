"""Causal cutoffs and temporal information accounting for the gold view."""
import json
import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gold_observatory as G


def sample(n=6000):
    rng = np.random.default_rng(42)
    t = 1_600_041_600_000 + np.arange(n, dtype=np.int64) * G.BASE
    c = 1_000_000 + np.cumsum(rng.integers(-20, 21, n))
    keep = np.ones(n, dtype=bool)
    keep[1500:1600] = False
    return {'t': t[keep], 'h': (c + 50)[keep], 'l': (c - 40)[keep], 'c': c[keep]}


class GoldObservation(unittest.TestCase):
    def test_future_append_and_tamper_do_not_change_any_report_field(self):
        m = sample()
        cutoff = int(m['t'][5300] + G.BASE)
        prefix = {k: v[:5301] for k, v in m.items()}
        changed = {k: v.copy() for k, v in m.items()}
        changed['h'][5301:] += 9_000_000
        changed['l'][5301:] -= 9_000_000
        changed['c'][5301:] *= 2
        expected = G.report(prefix, cutoff)
        self.assertTrue(expected['valid'])
        for other in (m, changed):
            self.assertEqual(expected, G.report(other, cutoff))
        json.dumps(expected, allow_nan=False)

    def test_bar_is_unknown_until_close_even_inside_a_forming_higher_timeframe(self):
        m = sample(500)
        k = 450
        cutoff = int(m['t'][k] + G.BASE - 1)
        r = G.report(m, cutoff)
        self.assertEqual(r['observed_bars'], k)
        self.assertEqual(r['asof'], G.iso(m['t'][k - 1] + G.BASE))
        self.assertEqual(G.report(m, cutoff + 1)['observed_bars'], k + 1)
        with self.assertRaisesRegex(ValueError, 'no closed'):
            G.report(m, int(m['t'][0]))

    def test_midpoint_window_count_and_extreme_age_match_raw_prefix(self):
        m = sample(800)
        r = G.report(m)
        for line in r['lines']:
            n = line['n']
            size = G.TFS[line['tf']]
            ids = np.unique(m['t'] // size)[-n:]
            mask = np.isin(m['t'] // size, ids)
            self.assertEqual(line['input_bars'], int(mask.sum()))
            if line['known']:
                hi, lo = m['h'][mask].max(), m['l'][mask].min()
                self.assertAlmostEqual(line['mid'], (hi + lo) / 2 * G.TICK)
                idx = np.flatnonzero(mask & (m['h'] == hi))[-1]
                age = (m['t'][-1] - m['t'][idx]) / 3_600_000
                self.assertEqual(line['high_age_hours'], age)

    def test_gaps_are_not_counted_as_adjacent_transitions(self):
        codes = np.array([[0, 0], [0, 0], [1, 1], [1, 1]])
        times = np.array([0, G.BASE, 10 * G.BASE, 11 * G.BASE])
        s = G.temporal(codes, times)
        self.assertEqual(s['adjacent_transitions'], 2)
        self.assertEqual(s['changed_fraction'], 0)
        self.assertEqual(s['state_entropy_bits'], 1)
        self.assertEqual(s['conditional_entropy_bits'], 0)
        self.assertEqual(s['current_prior_occurrences'], 1)

    def test_real_gold_load_and_unknown_warmup(self):
        m = G.load_gold()
        self.assertEqual(len(m['t']), 200_000)
        r = G.report({k: v[:10] for k, v in m.items()})
        self.assertEqual(r['hlc_values_seen'], 30)
        self.assertFalse(r['valid'])
        self.assertIsNone(r['lines'][-1]['mid'])
        self.assertEqual(r['lines'][-1]['observed_tf_bars'], 1)
        json.dumps(r, allow_nan=False)

    def test_timezone_is_required(self):
        with self.assertRaisesRegex(ValueError, 'timezone'):
            G.parse_asof('2020-01-01T00:00:00')
        self.assertEqual(G.parse_asof('2020-01-01T01:00:00+01:00'), G.parse_asof('2020-01-01T00:00:00Z'))


if __name__ == '__main__':
    unittest.main()
