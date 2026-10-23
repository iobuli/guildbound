"""Fixed-level identity, contact episodes, delayed confirmation and event context."""
import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gold_observatory as G
import gold_references as R
import nucleus as N


def series(h, l, c, indices=None):
    indices = np.arange(len(h)) if indices is None else np.array(indices)
    return {'t': (1_600_041_600_000 + indices * G.BASE).astype(np.int64),
            'h': np.array(h, dtype=np.int64), 'l': np.array(l, dtype=np.int64),
            'c': np.array(c, dtype=np.int64)}


def ledger(m, periods=(2,)):
    # The reference layer uses the report's already computed Nucleus measurements.
    # Tiny fixture periods exercise anchors while Nucleus warm-up stays explicitly unknown.
    nu = N.nucleus(m, {'30m': G.BASE})
    for n in periods:
        if n not in N.NS:
            for key in ('pos','slope','shift','lag'):
                nu[f'30m.{n}.{key}'] = nu[f'30m.9.{key}']
    return R.reference_report(m, nu, {'30m': G.BASE}, periods, G.BASE, G.TICK, G.iso)


class References(unittest.TestCase):
    def test_replay_boundary_does_not_invent_a_new_contact_or_anchor_birth(self):
        m=series([110]*60,[90]*60,[100]*60)
        nu=N.nucleus(m,{'30m':G.BASE})
        r=R.reference_report(m,nu,{'30m':G.BASE},(9,),G.BASE,G.TICK,G.iso,days=1)
        ref=next(x for x in r['references'] if x['ticks']==100)
        self.assertTrue(ref['carried_contact_from_before_window'])
        self.assertEqual(ref['contacts'],0)
        self.assertEqual(r['anchors'][0]['known_at'],G.iso(m['t'][8]+G.BASE))

    def test_birth_bar_is_not_touch_and_contiguous_contacts_are_one_episode(self):
        m = series([110,110,110,110,130,110], [90,90,90,90,120,90], [100,100,100,100,125,100])
        r = ledger(m)
        ref = next(x for x in r['references'] if x['ticks'] == 100)
        touches = [e for e in r['events'] if e['kind']=='contact' and e['reference_id']==ref['id']]
        self.assertEqual([e['observed_at'] for e in touches], [G.iso(m['t'][2]+G.BASE), G.iso(m['t'][5]+G.BASE)])
        self.assertEqual(ref['contacts'],2)
        self.assertEqual(ref['touches_by_tf'],{'30m':2})
        anchor = r['anchors'][0]
        self.assertEqual(anchor['mid'],.1)
        self.assertEqual([x['ratio'] for x in anchor['fib_levels']], list(R.FIBS))
        self.assertEqual(anchor['ichimoku']['conversion'],anchor['ichimoku']['span_b'])

    def test_old_prices_stay_fixed_and_previous_anchors_can_be_revisited(self):
        m=series([110,110,130,130,110],[90,90,120,120,90],[100,100,125,125,100])
        r=ledger(m)
        first=r['anchors'][0]
        self.assertEqual((first['low'],first['high']),(.09,.11))
        self.assertNotIn(first['id'],r['current_anchor_ids'])
        ref=next(x for x in r['references'] if x['ticks']==100)
        self.assertTrue(any(e['reference_id']==ref['id'] and e['observed_at']==G.iso(m['t'][-1]+G.BASE) for e in r['events']))
        contexts={o['id']:o for o in r['observations']}
        for event in r['events']:
            self.assertEqual(contexts[event['context_id']]['at'],event['observed_at'])
        short=ledger({k:v[:3] for k,v in m.items()})
        a=next(x for x in short['anchors'] if x['id']==first['id'])
        self.assertEqual(a,first)

    def test_gap_opens_a_new_observed_episode_not_an_inferred_touch(self):
        m=series([110]*4,[90]*4,[100]*4,[0,1,2,6])
        r=ledger(m)
        touches=[e for e in r['events'] if e['kind']=='contact' and e['price']==.1]
        self.assertEqual(len(touches),2)
        self.assertTrue(touches[-1]['after_gap'])

    def test_msb_is_confirmed_lower_high_break_not_retroactive_pivot(self):
        # Radius 1 fixture: high 20 confirmed at j2, lower high 18 confirmed j4,
        # then first later close above 18 at j5.
        b={'t':np.arange(7)*G.BASE,'end':(np.arange(7)+1)*G.BASE,
           'at':np.arange(7),'h':np.array([10,20,11,18,12,21,22]),
           'l':np.array([1,2,1,2,1,2,3]),'c':np.array([5,10,5,10,10,19,20]),'count':np.ones(7,int)}
        events=R.structure_events(b,'30m',G.iso,0,radius=1)
        msb=[e for e in events if e['kind']=='msb']
        self.assertEqual(len(msb),1)
        self.assertEqual((msb[0]['ticks'],msb[0]['at'],msb[0]['confirmed_index']),(18,5,4))
        prefix={k:v[:5] for k,v in b.items()}
        self.assertFalse(any(e['kind']=='msb' for e in R.structure_events(prefix,'30m',G.iso,0,radius=1)))

    def test_incomplete_tf_and_missing_boundary_are_not_closed_early(self):
        m=series([110]*3,[90]*3,[100]*3,[0,1,4])
        b=R.closed_bars(m,G.BASE*2,G.BASE)
        self.assertEqual(list(b['at']),[1])
        # The bar opening at index 4 has not completed its 1h bucket.
        self.assertEqual(len(b['t']),1)
        m=series([110]*2,[90]*2,[100]*2,[0,4])
        b=R.closed_bars(m,G.BASE*2,G.BASE)
        self.assertEqual(list(b['at']),[1])
        self.assertGreater(m['t'][b['at'][0]]+G.BASE,b['end'][0])


if __name__=='__main__':
    unittest.main()
