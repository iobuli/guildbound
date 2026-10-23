"""Availability, deduplication and label purging for the redesigned predictive state."""
import sys
import unittest
from pathlib import Path
import numpy as np
import pandas as pd
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import gold_predictive as P
import gold_information_study as S


def sample(n=6000):
    rng=np.random.default_rng(9)
    t=pd.Timestamp('2018-10-01',tz='UTC').value//1_000_000+np.arange(n,dtype=np.int64)*S.BASE
    c=np.exp(np.log(1200)+np.cumsum(rng.normal(0,.001,n)))
    return {'t':t,'c':c,'h':c*1.002,'l':c*.998}


class PredictiveState(unittest.TestCase):
    def test_asof_forecast_is_unchanged_by_future_append_or_tamper(self):
        m=sample(24000)
        m['t']=pd.Timestamp('2017-12-01',tz='UTC').value//1_000_000+np.arange(24000,dtype=np.int64)*S.BASE
        cut=21000;at=int(m['t'][cut]+S.BASE)
        policy={'selection_end_year':2018,'selected':{target:'prior' for target in P.TARGETS}}
        expected=P.forecast_asof({k:v[:cut+1] for k,v in m.items()},policy,at)
        self.assertTrue(expected['available'])
        changed={k:v.copy() for k,v in m.items()}
        for key in 'hlc':changed[key][cut+1:]*=3
        self.assertEqual(expected,P.forecast_asof(changed,policy,at))
        cutoff='2019-01-01T00:00:00Z'
        for output in expected['forecasts'].values():
            self.assertLess(output['fitted_label_through'],cutoff)
            self.assertNotIn('label',output)

    def test_selection_policy_cannot_forecast_before_development_ends(self):
        policy={'selection_end_year':2018,'selected':{target:'prior' for target in P.TARGETS}}
        report=P.forecast_asof(sample(500),policy)
        self.assertFalse(report['available'])
        self.assertIn('2018',report['reason'])

    def test_all_state_features_are_prefix_invariant(self):
        m=sample();cut=4500
        a=P.state_features(m)
        changed={k:v.copy() for k,v in m.items()}
        for key in 'hlc':changed[key][cut+1:]*=2
        for other in [changed,{k:v[:cut+1] for k,v in m.items()}]:
            b=P.state_features(other)
            for key in a:
                np.testing.assert_allclose(a[key][:cut+1],b[key][:cut+1],equal_nan=True,err_msg=key)
        self.assertTrue(a['valid'][cut])

    def test_current_bar_cannot_create_its_own_prior_range(self):
        m=sample();i=4000
        before=P.prior_ranges(m)
        changed={k:v.copy() for k,v in m.items()};changed['h'][i]*=2
        after=P.prior_ranges(changed)
        for a,b in zip(before,after):np.testing.assert_allclose(a[:i+1],b[:i+1],equal_nan=True)
        self.assertNotEqual(before[0][i+1,0],after[0][i+1,0])

    def test_identical_ranges_count_once(self):
        m=sample();m['h'][:]=1300;m['l'][:]=1100;m['c']=1200+np.sin(np.arange(len(m['t'])))*2
        f=P.state_features(m)
        self.assertTrue(f['valid'][-1]);self.assertEqual(f['unique_ranges'][-1],1)
        self.assertEqual(abs(f['consensus'][-1]),1)

    def test_fold_training_labels_precede_evaluation_start(self):
        m=sample(12000);f=P.state_features(m);s=P.research_samples(m,f)
        for target in P.TARGETS:
            train,test=P.fold_masks(s,target,2019)
            cutoff=pd.Timestamp('2019-01-01',tz='UTC').value//1_000_000
            self.assertTrue(train.any());self.assertTrue(test.any())
            self.assertTrue(np.all(s['label_at'][train]<cutoff))
            self.assertFalse(np.any(train&test))

    def test_horizon_gaps_are_not_labeled_as_four_hours(self):
        m=sample();m={k:np.delete(v,[4001,4002]) for k,v in m.items()}
        f=P.state_features(m);s=P.research_samples(m,f)
        i=s['index'];self.assertTrue(np.all(m['t'][i+8]-m['t'][i]==8*S.BASE))

    def test_prior_forecast_does_not_depend_on_covariates(self):
        x=np.arange(20.)[:,None];y=np.array([0.,1.]*10)
        fit=P.fit('prior',x,y)
        np.testing.assert_array_equal(P.probabilities(fit,np.array([[0.],[999.]])),[.5,.5])

if __name__=='__main__':unittest.main()
