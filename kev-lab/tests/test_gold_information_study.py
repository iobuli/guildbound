"""Research labels stay separate from causal features; controls are reproducible."""
import sys
import unittest
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import gold_information_study as S


def sample(n=2000):
    rng=np.random.default_rng(5)
    t=pd.Timestamp('2018-12-01',tz='UTC').value//1_000_000+np.arange(n,dtype=np.int64)*S.BASE
    c=np.exp(np.log(1200)+np.cumsum(rng.normal(0,.001,n)))
    return {'t':t,'c':c,'h':c*1.002,'l':c*.998}


class Study(unittest.TestCase):
    def test_future_prices_do_not_change_causal_raw_features(self):
        m=sample();cut=1500
        a=S.causal_features(m)
        changed={k:v.copy() for k,v in m.items()}
        for k in 'hlc':changed[k][cut+1:]*=1.5
        b=S.causal_features(changed)
        prefix=S.causal_features({k:v[:cut+1] for k,v in m.items()})
        for key in a:
            np.testing.assert_allclose(a[key][:cut+1],b[key][:cut+1],equal_nan=True,err_msg=key)
            np.testing.assert_allclose(a[key][:cut+1],prefix[key],equal_nan=True,err_msg=key)

    def test_weekly_anchor_excludes_its_current_bar(self):
        m=sample();week,lo,hi=S.weekly_anchors(m)
        i=int(np.flatnonzero(np.diff(week))[1]+1)
        changed={k:v.copy() for k,v in m.items()};changed['h'][i]*=2
        _,otherlo,otherhi=S.weekly_anchors(changed)
        self.assertEqual(hi[i],otherhi[i]);self.assertEqual(lo[i],otherlo[i])
        self.assertEqual(hi[i],m['h'][i-96:i].max())

    def test_logistic_generalizes_a_known_relation(self):
        rng=np.random.default_rng(3);x=rng.normal(size=(1000,2));y=(x[:,0]>0).astype(float)
        fit=S.fit_logistic(x[:700],y[:700]);p=S.predict(fit,x[700:])
        self.assertGreater(np.mean((p>.5)==y[700:]),.98)

    def test_identical_paired_predictions_have_zero_increment(self):
        effect=S.block_difference(np.zeros(80),np.repeat(np.arange(20),4),draws=100)
        self.assertEqual(effect['mean'],0);self.assertEqual(effect['ci95'],[0,0]);self.assertEqual(effect['p'],1)

    def test_null_preserves_timestamps_and_quarter_endpoint(self):
        m=sample(); dates=pd.to_datetime(m['t'],unit='ms',utc=True)
        quarters=dates.year*4+(dates.month-1)//3
        groups=[np.flatnonzero(quarters==q) for q in np.unique(quarters)]
        for block in [1,48]:
            null=S.surrogate(m,groups,np.random.default_rng(7),block)
            np.testing.assert_array_equal(null['t'],m['t'])
            self.assertTrue(np.all(null['h']>=null['c']));self.assertTrue(np.all(null['l']<=null['c']))
            for group in groups:
                self.assertAlmostEqual(null['c'][group[-1]],m['c'][group[-1]],delta=S.TICK)


if __name__=='__main__':unittest.main()
