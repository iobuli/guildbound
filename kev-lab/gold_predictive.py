"""Causal predictive state: deduplicated anchors, fresh events and expiring memory.

Feature extraction never reads future prices. Walk-forward labels/fits live in
an explicitly separate research runner. All forecasts are experimental unless
the saved evaluation independently supports their declared skill criterion.
"""
from __future__ import annotations

import numpy as np
import pandas as pd

import gold_information_study as S
from gold_references import closed_bars

TFS={'30m':S.BASE,'1h':2*S.BASE,'4h':8*S.BASE,'1D':48*S.BASE}
PERIODS=(9,26,52)
CONFIGS=('prior','raw_logistic','raw_boost','events_boost','events_fib_boost')
TARGETS=('direction','large_displacement','terminal_range_exit')


def prior_ranges(m):
    """Latest completed n-bar ranges AVAILABLE BEFORE each source bar's close."""
    size=len(m['t']); high=[];low=[]
    previous_observation=np.arange(size)-1
    for tf,width in TFS.items():
        bars=closed_bars(m,width,S.BASE)
        j=np.searchsorted(bars['at'],previous_observation,side='right')-1
        for n in PERIODS:
            h=np.full(size,np.nan);l=h.copy()
            if len(bars['t'])>=n:
                rh=S.rolling(bars['h'],n,'max');rl=S.rolling(bars['l'],n,'min')
                eligible=j>=n-1
                h[eligible]=rh[j[eligible]];l[eligible]=rl[j[eligible]]
            high.append(h);low.append(l)
    return np.column_stack(high),np.column_stack(low)


def state_features(m):
    f=S.causal_features(m)
    high,low=prior_ranges(m); n=len(m['t']);rows=np.arange(n)
    known=np.isfinite(high)&np.isfinite(low)&(high>low)
    unique=known.copy()
    for j in range(1,high.shape[1]):
        repeated=np.any((high[:,j,None]==high[:,:j])&(low[:,j,None]==low[:,:j]),axis=1)
        unique[:,j]&=~repeated
    count=unique.sum(axis=1)
    def mean(x):
        return np.sum(np.where(unique,x,0),axis=1)/np.maximum(count,1)
    span=np.where(high>low,high-low,np.nan)
    q=(m['c'][:,None]-low)/span
    qclip=np.clip(q,-2,3)
    consensus=mean(np.sign(q-.5))
    avg=mean(qclip)
    disagreement=np.sqrt(np.maximum(0,mean(qclip*qclip)-avg*avg))
    upper_breach=m['h'][:,None]>high;lower_breach=m['l'][:,None]<low
    upper_rejection=upper_breach&(m['c'][:,None]<high)
    lower_rejection=lower_breach&(m['c'][:,None]>low)
    upper_acceptance=m['c'][:,None]>high;lower_acceptance=m['c'][:,None]<low
    previous_high=np.vstack([np.full((1,high.shape[1]),np.nan),high[:-1]])
    previous_low=np.vstack([np.full((1,low.shape[1]),np.nan),low[:-1]])
    expiry_up=low>previous_low;expiry_down=high<previous_high
    age=[]
    for values,previous in [(high,previous_high),(low,previous_low)]:
        changed=(values!=previous)|~np.isfinite(previous)
        last=np.maximum.accumulate(np.where(changed,rows[:,None],0),axis=0)
        age.append(mean(np.log1p((rows[:,None]-last)/48)))
    fresh=mean(upper_breach.astype(float)-lower_breach)
    rejection=mean(lower_rejection.astype(float)-upper_rejection)
    acceptance=mean(upper_acceptance.astype(float)-lower_acceptance)
    event=np.column_stack([count/12,avg,disagreement,consensus,
                           mean(upper_breach),mean(lower_breach),rejection,acceptance,
                           mean(expiry_up),mean(expiry_down),age[0],age[1],
                           fresh*consensus,rejection*consensus,acceptance*disagreement])
    dates=pd.to_datetime(m['t']+S.BASE,unit='ms',utc=True)
    hour=dates.hour.to_numpy()+dates.minute.to_numpy()/60
    weekday=dates.day_of_week.to_numpy()
    # Honest raw baseline includes geometry, time-of-week and volatility information.
    h104=np.r_[np.nan,S.rolling(m['h'],104,'max')[:-1]]
    l104=np.r_[np.nan,S.rolling(m['l'],104,'min')[:-1]]
    risk=f['std']*np.sqrt(S.HORIZON)
    distance_up=np.log(h104/m['c'])/risk
    distance_down=np.log(m['c']/l104)/risk
    bar_range=np.log(m['h']/m['l'])/f['std']
    clv=(2*m['c']-m['h']-m['l'])/np.maximum(S.TICK,m['h']-m['l'])
    gap=np.r_[1.,np.diff(m['t'])/S.BASE]
    raw=np.column_stack([f['base'],distance_up,distance_down,bar_range,clv,
                         np.sin(2*np.pi*hour/24),np.cos(2*np.pi*hour/24),
                         np.sin(2*np.pi*weekday/7),np.cos(2*np.pi*weekday/7),np.log1p(gap)])
    fib_levels=f['lo'][:,None]+S.FIB*(f['hi']-f['lo'])[:,None]
    contact=(m['l'][:,None]<=fib_levels)&(m['h'][:,None]>=fib_levels)
    previous_close=np.r_[np.nan,m['c'][:-1]]
    approach=np.sign(previous_close[:,None]-fib_levels)
    close_side=np.sign(m['c'][:,None]-fib_levels)
    fib_state=np.column_stack([f['fib'],contact.astype(float),contact*approach*close_side])
    valid=known.all(axis=1)&np.isfinite(raw).all(axis=1)&np.isfinite(event).all(axis=1)&np.isfinite(fib_state).all(axis=1)
    return {'raw':raw,'events':np.column_stack([raw,event]),'events_fib':np.column_stack([raw,event,fib_state]),
            'valid':valid,'unique_ranges':count,'consensus':consensus,'disagreement':disagreement,
            'fresh_breach_balance':fresh,'expiry_up':mean(expiry_up),'expiry_down':mean(expiry_down),
            'rejection_balance':rejection,'acceptance_balance':acceptance,
            'week':f['week'],'std':f['std'],'prior_high104':h104,'prior_low104':l104,
            'distance_up':distance_up,'distance_down':distance_down}


def research_samples(m,features):
    """Labels are returned only for research. They are never used by state_features."""
    idx=np.arange(192,len(m['t'])-S.HORIZON,S.HORIZON)
    eligible=(m['t'][idx+S.HORIZON]-m['t'][idx]==S.HORIZON*S.BASE)&features['valid'][idx]
    idx=idx[eligible]
    move=np.log(m['c'][idx+S.HORIZON]/m['c'][idx])
    terminal=m['c'][idx+S.HORIZON]
    labels={'direction':(move>0).astype(float),
            'large_displacement':(np.abs(move)>features['std'][idx]*np.sqrt(S.HORIZON)).astype(float),
            'terminal_range_exit':((terminal>features['prior_high104'][idx])|(terminal<features['prior_low104'][idx])).astype(float)}
    valid_labels={'direction':move!=0,'large_displacement':np.ones(len(idx),bool),'terminal_range_exit':np.ones(len(idx),bool)}
    return {'index':idx,'at':m['t'][idx]+S.BASE,'label_at':m['t'][idx+S.HORIZON]+S.BASE,
            'labels':labels,'valid_labels':valid_labels,'week':features['week'][idx]}


def fit(config,x,y):
    prior=float(np.mean(y))
    if config=='prior':return {'config':config,'prior':prior,'model':None}
    if config=='raw_logistic':model=S.fit_logistic(x,y,penalty=100.)
    else:
        from sklearn.ensemble import HistGradientBoostingClassifier
        model=HistGradientBoostingClassifier(max_iter=100,learning_rate=.05,max_depth=2,max_leaf_nodes=4,
                                             min_samples_leaf=100,l2_regularization=10,early_stopping=False,
                                             random_state=20261023).fit(x,y)
    return {'config':config,'prior':prior,'model':model}


def probabilities(fitted,x):
    if fitted['model'] is None:return np.full(len(x),fitted['prior'])
    if fitted['config']=='raw_logistic':p=S.predict(fitted['model'],x)
    else:p=fitted['model'].predict_proba(x)[:,1]
    return np.clip(fitted['prior']+.5*(p-fitted['prior']),1e-6,1-1e-6)


def matrix(features,config):
    return features['events_fib'] if config=='events_fib_boost' else features['events'] if config=='events_boost' else features['raw']


def fold_masks(samples,target,year):
    cutoff=int(pd.Timestamp(f'{year}-01-01',tz='UTC').value//1_000_000)
    begin=int(pd.Timestamp(f'{year-3}-01-01',tz='UTC').value//1_000_000)
    end=int(pd.Timestamp(f'{year+1}-01-01',tz='UTC').value//1_000_000)
    eligible=samples['valid_labels'][target]
    train=(samples['at']>=begin)&(samples['at']<cutoff)&(samples['label_at']<cutoff)&eligible
    test=(samples['at']>=cutoff)&(samples['at']<end)&eligible
    return train,test


def forecast_asof(m,policy,asof=None):
    """Fit only pre-year, already observed labels; return no future outcome values."""
    size=len(m['t']) if asof is None else int(np.searchsorted(m['t']+S.BASE,asof,side='right'))
    if not size:raise ValueError('no closed gold observations at this cutoff')
    m={key:values[:size] for key,values in m.items()}
    at=int(m['t'][-1]+S.BASE);year=pd.Timestamp(at,unit='ms',tz='UTC').year
    stamp=lambda value:pd.Timestamp(int(value),unit='ms',tz='UTC').isoformat().replace('+00:00','Z')
    if year<=policy['selection_end_year']:
        return {'available':False,'asof':stamp(at),'reason':'Policy selection uses development data through 2018; forecasts begin in 2019.'}
    f=state_features(m)
    if not f['valid'][-1]:return {'available':False,'asof':stamp(at),'reason':'Insufficient valid history for the predictive state.'}
    samples=research_samples(m,f);outputs={}
    for target in TARGETS:
        config=policy['selected'][target]
        train,_=fold_masks(samples,target,year)
        if train.sum()<300:return {'available':False,'asof':stamp(at),'reason':'Fewer than 300 eligible preceding-window training observations.'}
        x=matrix(f,config);train_x=x[samples['index'][train]];y=samples['labels'][target][train]
        fitted=fit(config,train_x,y);prob=float(probabilities(fitted,x[-1:])[0])
        outputs[target]={'probability':prob,'historical_frequency':float(y.mean()),'config':config,
                         'training_n':int(train.sum()),'fitted_label_through':stamp(samples['label_at'][train].max()),
                         'status':'experimental; no validated structural increment',
                         'directional_skill_demonstrated':False if target=='direction' else None}
    from scipy.special import ndtr
    geometry=float(np.clip(ndtr(-f['distance_up'][-1])+ndtr(-f['distance_down'][-1]),0,1))
    return {'available':True,'asof':stamp(at),'source':'data/XAUUSD_m30.json','source_resolution_minutes':30,
            'policy_selected_through':policy['selection_end_year'],'horizon_hours':4,'horizon_source_bars':8,
            'target_definitions':{
                'direction':'P(close after 8 contiguous M30 observations > current close); zero-move training labels excluded.',
                'large_displacement':'P(abs(log future-close/current-close) > current trailing-48-return std × sqrt(8)).',
                'terminal_range_exit':'P(close after 8 contiguous M30 observations outside the prior 104-observation high/low). Not an intrabar barrier hit.'},
            'forecasts':outputs,'reference':{'close':float(m['c'][-1]),'prior_high104':float(f['prior_high104'][-1]),
                                            'prior_low104':float(f['prior_low104'][-1]),
                                            'large_move_log_threshold':float(f['std'][-1]*np.sqrt(8)),
                                            'normal_geometry_exit_baseline':geometry},
            'state':{'range_instances':12,'unique_ranges':int(f['unique_ranges'][-1]),
                     **{key:float(f[key][-1]) for key in ('consensus','disagreement','fresh_breach_balance','expiry_up','expiry_down','rejection_balance','acceptance_balance')}},
            'notes':['Selected configurations were locked using 2013–2018 development folds, not later outcomes.',
                     'Annual fits use only labels known before January 1 of this observation year, from its preceding three calendar years.',
                     'No future outcomes are returned. The horizon excludes gaps in research; calendar time can differ if a future gap occurs.',
                     'Event probability is not a directional signal, trade instruction or profit estimate.',
                     'All heads remain experimental; the stricter added-structure criterion did not pass.']}
