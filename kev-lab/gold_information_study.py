"""Fixed-plan retrospective gold information study; see the dated experiment plan."""
from __future__ import annotations
import argparse
import json
from pathlib import Path

import numpy as np
import pandas as pd

import nucleus as N

BASE = 1_800_000
TICK = .001
ROOT = Path(__file__).resolve().parents[1]
FIB = np.array([.236, .382, .618, .786])
CONTROL = np.array([.25, .375, .625, .75])
HORIZON = 8
TRAIN_END = 2018


def load():
    rows = json.loads((ROOT/'data/XAUUSD_m30.json').read_text())
    return {'t': np.array([b['t'] for b in rows], dtype=np.int64),
            **{k: np.array([b[k] for b in rows], float) for k in 'hlc'}}


def rolling(a, n, op):
    return getattr(pd.Series(a).rolling(n, min_periods=n), op)().to_numpy()


def weekly_anchors(m):
    # Epoch Thursday offset +3 aligns floor to Monday UTC; all anchors precede i.
    week = (m['t']//86_400_000 + 3)//7
    starts = np.r_[0, np.flatnonzero(np.diff(week)) + 1]
    hi = np.full(len(week), np.nan); lo = hi.copy()
    for j, begin in enumerate(starts):
        end = starts[j+1] if j+1 < len(starts) else len(week)
        if begin >= 96:
            hi[begin:end] = m['h'][begin-96:begin].max()
            lo[begin:end] = m['l'][begin-96:begin].min()
    return week, lo, hi


def organization(m, mask):
    c = m['c']; windows = {}
    for n in (18,52,104):
        windows[n] = (rolling(m['h'], n, 'max'), rolling(m['l'], n, 'min'))
    h,l = windows[104]; mid = (h+l)/2
    contiguous = np.r_[False, np.diff(m['t']) == BASE]
    eligible = mask & contiguous & np.isfinite(np.r_[np.nan,mid[:-1]]) & np.isfinite(mid)
    old = np.r_[np.nan,mid[:-1]]
    moved = eligible & (mid != old)
    new_extreme = (m['h'] > np.r_[np.nan,h[:-1]]) | (m['l'] < np.r_[np.nan,l[:-1]])
    shared = ((windows[18][0] == h).astype(float) + (windows[18][1] == l))/2
    signs = np.column_stack([np.sign(c-(a+b)/2) for a,b in windows.values()])
    known = mask & np.isfinite(signs).all(axis=1)
    result = {'midpoint_persistence': float(np.mean(mid[eligible] == old[eligible])),
              'short_long_extreme_sharing': float(shared[known].mean()),
              'same_side_synchronization': float(np.mean(np.all(signs[known] == signs[known,0,None],axis=1)))}
    extra = {'midpoint_changes': int(moved.sum()),
             'changed_without_new_extreme': int((moved & ~new_extreme).sum()),
             'expiry_only_fraction_of_changes': float((moved & ~new_extreme).sum()/max(1,moved.sum())),
             'observations': int(known.sum())}
    return result, extra


def bh(p):
    p=np.asarray(p); order=np.argsort(p); adjusted=np.minimum.accumulate((p[order]*len(p)/np.arange(1,len(p)+1))[::-1])[::-1]
    out=np.empty(len(p)); out[order]=np.minimum(1,adjusted); return out


def block_difference(values, weeks, draws=1000, seed=20261023):
    values=np.asarray(values,float); weeks=np.asarray(weeks,int)
    if not len(values): return {'n':0,'mean':None,'ci95':None,'p':1.0}
    ids=weeks-weeks.min(); count=np.bincount(ids); sums=np.bincount(ids,weights=values)
    rng=np.random.default_rng(seed); size=len(count); block=min(4,size); estimates=[]
    for _ in range(draws):
        starts=rng.integers(0,size-block+1,int(np.ceil(size/block)))
        sample=(starts[:,None]+np.arange(block)).ravel()[:size]
        denom=count[sample].sum()
        if denom: estimates.append(sums[sample].sum()/denom)
    mean=float(values.mean()); estimates=np.array(estimates)
    p=float((1+np.sum(np.abs(estimates-mean)>=abs(mean)))/(len(estimates)+1))
    return {'n':len(values),'calendar_weeks':size,'mean':mean,'ci95':np.quantile(estimates,[.025,.975]).tolist(),'p':p}


def fit_logistic(x,y,penalty=1.0):
    center=x.mean(axis=0); scale=x.std(axis=0); scale[scale<1e-10]=1
    a=np.column_stack([np.ones(len(x)),(x-center)/scale]); coef=np.zeros(a.shape[1])
    reg=np.eye(a.shape[1])*penalty; reg[0,0]=0
    def objective(b):
        z=a@b; return float(np.sum(np.logaddexp(0,z)-y*z)+b@reg@b/2)
    for _ in range(50):
        z=a@coef; p=1/(1+np.exp(-np.clip(z,-35,35)))
        gradient=a.T@(p-y)+reg@coef
        hessian=a.T@(a*(p*(1-p))[:,None])+reg
        step=np.linalg.solve(hessian,gradient)
        if np.max(np.abs(step))<1e-7: break
        rate=1.; old=objective(coef)
        while rate>1e-8 and objective(coef-rate*step)>old:
            rate/=2
        coef-=rate*step
    return center,scale,coef


def predict(model,x):
    center,scale,coef=model
    z=np.column_stack([np.ones(len(x)),(x-center)/scale])@coef
    return 1/(1+np.exp(-np.clip(z,-35,35)))


def causal_features(m):
    log=np.log(m['c']); ret=np.r_[np.nan,np.diff(log)]
    std=rolling(ret,48,'std'); fast=rolling(ret,8,'std')
    week,lo,hi=weekly_anchors(m)
    q=(m['c']-lo)/np.where(hi>lo,hi-lo,np.nan)
    prior=lambda n: log-np.r_[np.full(n,np.nan),log[:-n]]
    base=np.column_stack([prior(1)/std, prior(8)/(std*np.sqrt(8)),prior(48)/(std*np.sqrt(48)),
                          fast/std,np.clip(q,-3,3),np.clip(q,-3,3)**2,np.log(hi/lo)/(std*np.sqrt(48))])
    fib=np.exp(-np.abs(q[:,None]-FIB)/.02)
    ordinary=np.exp(-np.abs(q[:,None]-CONTROL)/.02)
    return {'base':base,'fib':fib,'ordinary':ordinary,'week':week,'lo':lo,'hi':hi,'std':std,'q':q}


def predictive(m,features,draws):
    tfs={'30m':BASE,'1h':2*BASE,'4h':8*BASE,'1D':48*BASE}
    ticks={'t':m['t'],**{k:np.rint(m[k]/TICK).astype(np.int64) for k in 'hlc'}}
    nu=N.nucleus(ticks,tfs)
    extra=np.column_stack([*[nu[f'{tf}.bal'] for tf in tfs],nu['slope_bal'],
                           *[np.clip(nu[f'{tf}.{n}.pos'],-3,3) for tf in tfs for n in N.NS]])
    idx=np.arange(192,len(m['t'])-HORIZON,HORIZON)
    idx=idx[(m['t'][idx+HORIZON]-m['t'][idx]==HORIZON*BASE) & nu['valid'][idx]]
    future=np.log(m['c'][idx+HORIZON]/m['c'][idx]); idx=idx[future!=0]
    future=np.log(m['c'][idx+HORIZON]/m['c'][idx]); y=(future>0).astype(float)
    finite=np.isfinite(features['base'][idx]).all(axis=1)&np.isfinite(extra[idx]).all(axis=1)
    idx=idx[finite];y=y[finite]
    years=pd.to_datetime(m['t'][idx]+BASE,unit='ms',utc=True).year.to_numpy()
    endyears=pd.to_datetime(m['t'][idx+HORIZON]+BASE,unit='ms',utc=True).year.to_numpy()
    train=(years<=TRAIN_END)&(endyears<=TRAIN_END); test=years>TRAIN_END
    b=features['base'][idx]; n=extra[idx]
    matrices={'baseline':b,'nucleus':np.column_stack([b,n]),
              'nucleus_fib':np.column_stack([b,n,features['fib'][idx]]),
              'nucleus_ordinary':np.column_stack([b,n,features['ordinary'][idx]]),
              'fib':np.column_stack([b,features['fib'][idx]])}
    predictions={}; metrics={}
    yt=y[test]; holdyears=years[test]
    for name,x in matrices.items():
        fitted=fit_logistic(x[train],y[train]); prob=predict(fitted,x[test]); predictions[name]=prob
        metrics[name]={'features':x.shape[1],'brier':float(np.mean((prob-yt)**2)),
                       'logloss':float(np.mean(np.logaddexp(0,np.log(prob/(1-prob)))-yt*np.log(prob/(1-prob)))),
                       'accuracy':float(np.mean((prob>.5)==yt))}
    constant=float(y[train].mean())
    metrics['training_frequency']={'probability_up':constant,'brier':float(np.mean((constant-yt)**2)),
                                   'accuracy':float(np.mean((constant>.5)==yt))}
    comparisons=[]
    for a,b in [('nucleus','baseline'),('nucleus_fib','nucleus'),('nucleus_fib','nucleus_ordinary'),('fib','baseline')]:
        differences=(predictions[a]-yt)**2-(predictions[b]-yt)**2
        effect=block_difference(differences,features['week'][idx[test]],draws)
        effect.update(name=f'{a}_minus_{b}',favorable='negative',
                      period_means={label:float(differences[mask].mean()) for label,mask in [('2019_2022',holdyears<=2022),('2023_onward',holdyears>=2023)]})
        comparisons.append(effect)
    return {'training_observations':int(train.sum()),'evaluation_observations':int(test.sum()),
            'metrics':metrics,'comparisons':comparisons}, {'index':idx[test],'years':holdyears,'labels':yt,'probabilities':predictions}


def contacts(m,f,draws):
    size=len(m['t']); years=pd.to_datetime(m['t']+BASE,unit='ms',utc=True).year.to_numpy()
    gap_ok=np.zeros(size,bool);gap_ok[:-HORIZON]=(m['t'][HORIZON:]-m['t'][:-HORIZON]==HORIZON*BASE)
    contiguous=np.r_[False,np.diff(m['t'])==BASE]
    previous=np.r_[np.nan,m['c'][:-1]]; future=np.full(size,np.nan);future[:-HORIZON]=np.log(m['c'][HORIZON:]/m['c'][:-HORIZON])
    records={}; counts={}; tables=[]
    for kind,ratios in [('fib',FIB),('ordinary',CONTROL)]:
        for pair,ratio in enumerate(ratios):
            level=f['lo']+ratio*(f['hi']-f['lo'])
            hit=(m['l']<=level)&(m['h']>=level)
            indices=np.flatnonzero(hit)
            _,first=np.unique(f['week'][indices],return_index=True)
            indices=indices[first]
            sign=np.sign(previous[indices]-level[indices])
            eligible=(years[indices]>TRAIN_END)&gap_ok[indices]&contiguous[indices]&(sign!=0)&np.isfinite(f['std'][indices])&(f['std'][indices]>0)
            indices=indices[eligible];sign=sign[eligible]
            aligned=sign*future[indices]/(f['std'][indices]*np.sqrt(HORIZON))
            for i,direction,value in zip(indices,sign,aligned):
                records[(kind,int(f['week'][i]),pair,int(direction))]=(float(value),int(years[i]),int(i))
            counts[f'{kind}_{ratio}']=len(indices)
            tables.append({'group':kind,'ratio':float(ratio),'contacts':len(indices),
                           'future_up_count':int(np.sum(future[indices]>0)),
                           'future_down_count':int(np.sum(future[indices]<0)),
                           'future_flat_count':int(np.sum(future[indices]==0)),
                           'approach_side_return_count':int(np.sum(aligned>0)),
                           'future_up_fraction':float(np.mean(future[indices]>0)),
                           'approach_side_return_fraction':float(np.mean(aligned>0)),
                           'mean_normalized_return_toward_approach':float(aligned.mean())})
    paired=[]
    for key,(value,year,index) in records.items():
        kind,week,pair,side=key
        if kind=='fib' and ('ordinary',week,pair,side) in records:
            control,cy,ci=records[('ordinary',week,pair,side)]
            paired.append((value-control,week,year,abs(m['t'][index]-m['t'][ci])/3_600_000))
    values=np.array([x[0] for x in paired]);weeks=np.array([x[1] for x in paired]);py=np.array([x[2] for x in paired])
    effect=block_difference(values,weeks,draws)
    effect.update(name='fib_contact_minus_ordinary_contact',favorable='positive',
                  period_means={label:float(values[mask].mean()) for label,mask in [('2019_2022',py<=2022),('2023_onward',py>=2023)]})
    occupied=(years>TRAIN_END)&np.isfinite(f['q'])
    occupancy={kind:float(np.mean(np.min(np.abs(f['q'][occupied,None]-ratios),axis=1)<=.005)) for kind,ratios in [('fib',FIB),('ordinary',CONTROL)]}
    return {'tables':tables,'counts':counts,'matched_pairs':len(paired),
            'median_matched_event_time_distance_hours':float(np.median([x[3] for x in paired])),
            'same_bar_matched_fraction':float(np.mean([x[3]==0 for x in paired])),
            'within_half_percent_of_anchor_width':occupancy,'comparison':effect}


def surrogate(m,groups,rng,block):
    indices=np.arange(len(m['t']))
    for group in groups:
        group=group[group>0]
        if not len(group): continue
        chunks=[group[j:j+block] for j in range(0,len(group),block)]
        indices[group]=np.concatenate([chunks[j] for j in rng.permutation(len(chunks))])
    returns=np.r_[0.,np.diff(np.log(m['c']))]
    c=np.exp(np.log(m['c'][0])+np.cumsum(returns[indices]))
    return {'t':m['t'],'c':np.rint(c/TICK)*TICK,
            'h':np.rint(c*(m['h'][indices]/m['c'][indices])/TICK)*TICK,
            'l':np.rint(c*(m['l'][indices]/m['c'][indices])/TICK)*TICK}


def null_test(m,mask,draws):
    dates=pd.to_datetime(m['t'],unit='ms',utc=True); quarters=dates.year*4+(dates.month-1)//3
    groups=[np.flatnonzero(quarters==q) for q in np.unique(quarters)]
    observed,extra=organization(m,mask); nulls={}; tests=[]
    for block in [1,48]:
        name='quarter_permuted_bars' if block==1 else 'quarter_permuted_48bar_blocks'
        samples={k:[] for k in observed};rng=np.random.default_rng(20261023+block)
        for run in range(draws):
            sim=surrogate(m,groups,rng,block); metrics,_=organization(sim,mask)
            for key,value in metrics.items():samples[key].append(value)
            if (run+1)%20==0: print(f'{name}: {run+1}/{draws}',flush=True)
        nulls[name]={}
        for key,values in samples.items():
            a=np.array(values); obs=observed[key]
            p=min(1.,2*min((1+np.sum(a>=obs))/(draws+1),(1+np.sum(a<=obs))/(draws+1)))
            item={'metric':key,'null':name,'observed':obs,'null_median':float(np.median(a)),
                  'null_ci95':np.quantile(a,[.025,.975]).tolist(),'p':float(p)}
            tests.append(item);nulls[name][key]=values
    for item,q in zip(tests,bh([x['p'] for x in tests])):item['q']=float(q)
    return {'observed':observed,'boundary_change_accounting':extra,'tests':tests,'null_samples':nulls}


def run(out,draws=99,bootstrap=1000):
    m=load();print('Loaded',len(m['t']),'gold observations',flush=True)
    f=causal_features(m);print('Computed causal raw features',flush=True)
    prediction,chart=predictive(m,f,bootstrap); print('Chronological predictive models complete',flush=True)
    contact=contacts(m,f,bootstrap);print('Fixed weekly-anchor contact controls complete',flush=True)
    comparisons=prediction['comparisons']+[contact['comparison']]
    for item,q in zip(comparisons,bh([x['p'] for x in comparisons])):
        item['q']=float(q)
        sign=-1 if item['favorable']=='negative' else 1
        ci=item['ci95'];item['candidate']=bool(q<.05 and min(sign*ci[0],sign*ci[1])>0 and all(sign*v>0 for v in item['period_means'].values()))
    mask=pd.to_datetime(m['t']+BASE,unit='ms',utc=True).year.to_numpy()>TRAIN_END
    nulls=null_test(m,mask,draws)
    result={'source':'data/XAUUSD_m30.json','source_bars':len(m['t']),'training_end_year':TRAIN_END,
            'evaluation_start_year':TRAIN_END+1,'horizon_observations':HORIZON,
            'horizon_contiguous_hours':4,'null_draws_per_family':draws,'bootstrap_draws':bootstrap,
            'predictive':prediction,'contacts':contact,'raw_organization':nulls,
            'limitations':['Retrospective holdout; historical data were previously examined.',
              'Single gold instrument. No independent market confirmation.',
              'Level contact is descriptive, not a fill or evidence that the level caused price movement.',
              'Nulls preserve quarter drift/bar geometry and calendar gaps, not every volatility dependence.',
              'Weekly block bootstrap does not prove complete absence of long-memory dependence.',
              'No hyperparameter tuning; predictive claims apply only to these fixed features, models and horizon.']}
    out.mkdir(parents=True,exist_ok=True)
    (out/'results.json').write_text(json.dumps(result,allow_nan=False,indent=2)+'\n')
    np.savez_compressed(out/'chart-data.npz',time=m['t'][chart['index']]+BASE,
                        labels=chart['labels'],**chart['probabilities'])
    print('Saved',out/'results.json',flush=True)
    return result


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,default=ROOT/'out/nucleus/gold-information-study')
    ap.add_argument('--null-draws',type=int,default=99)
    ap.add_argument('--bootstrap-draws',type=int,default=1000)
    args=ap.parse_args();run(args.out,args.null_draws,args.bootstrap_draws)
