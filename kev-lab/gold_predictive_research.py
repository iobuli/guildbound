"""Locked development selection and annual walk-forward evaluation of predictive state."""
from __future__ import annotations
import argparse
import json
from pathlib import Path

import numpy as np
import pandas as pd

import gold_information_study as S
import gold_predictive as P

ROOT=S.ROOT
OUT=ROOT/'out/nucleus/gold-predictive-redesign'


def metrics(y,p):
    p=np.clip(p,1e-6,1-1e-6)
    bins=[]
    for lo,hi in [(0,.2),(.2,.4),(.4,.6),(.6,.8),(.8,1.00001)]:
        use=(p>=lo)&(p<hi)
        if use.any():bins.append({'probability_band':[lo,min(1.,hi)],'n':int(use.sum()),
                                  'mean_forecast':float(p[use].mean()),'observed_frequency':float(y[use].mean())})
    return {'n':len(y),'frequency':float(y.mean()),'mean_probability':float(p.mean()),
            'brier':float(np.mean((p-y)**2)),
            'logloss':float(np.mean(-y*np.log(p)-(1-y)*np.log1p(-p))),
            'accuracy':float(np.mean((p>.5)==y)),'calibration':bins}


def evaluate(out=OUT):
    m=S.load();f=P.state_features(m);samples=P.research_samples(m,f)
    print('Causal feature/state rows:',len(m['t']),'research samples:',len(samples['at']),flush=True)
    idx=samples['index'];x={name:P.matrix(f,name)[idx] for name in P.CONFIGS}
    development={};selected={}
    # No evaluation-period target enters this selection loop.
    for target in P.TARGETS:
        scores={name:[] for name in P.CONFIGS};folds=[]
        for year in range(2013,2019):
            train,test=P.fold_masks(samples,target,year)
            if train.sum()<300 or test.sum()<100:continue
            y=samples['labels'][target]
            for name in P.CONFIGS:
                fitted=P.fit(name,x[name][train],y[train]);prob=P.probabilities(fitted,x[name][test])
                scores[name].extend(((prob-y[test])**2).tolist())
            folds.append({'year':year,'training_n':int(train.sum()),'validation_n':int(test.sum())})
            print('Development',target,year,'complete',flush=True)
        means={name:float(np.mean(values)) for name,values in scores.items()}
        if any(not np.isfinite(value) for value in means.values()):raise ValueError('incomplete development selection')
        selected[target]=min(P.CONFIGS,key=lambda name:means[name])
        development[target]={'brier':means,'folds':folds,'selected':selected[target]}
    print('LOCKED selections:',selected,flush=True)
    selection_path=out/'locked-selection.json';out.mkdir(parents=True,exist_ok=True)
    selection_path.write_text(json.dumps({'selection_end_year':2018,'development':development},indent=2)+'\n')
    results={};comparisons=[];arrays={}
    for target in P.TARGETS:
        names=list(dict.fromkeys(['prior','raw_logistic','raw_boost',selected[target]]))
        forecasts={name:[] for name in names};labels=[];weeks=[];years=[];times=[];folds=[]
        y=samples['labels'][target]
        for year in range(2019,2027):
            train,test=P.fold_masks(samples,target,year)
            if train.sum()<300 or test.sum()<100:continue
            last_label=int(samples['label_at'][train].max())
            if last_label>=int(pd.Timestamp(f'{year}-01-01',tz='UTC').value//1_000_000):raise AssertionError('training label leakage')
            for name in names:
                model=P.fit(name,x[name][train],y[train]);p=P.probabilities(model,x[name][test]);forecasts[name].extend(p.tolist())
            labels.extend(y[test].tolist());weeks.extend(samples['week'][test].tolist())
            years.extend([year]*int(test.sum()));times.extend(samples['at'][test].tolist())
            folds.append({'year':year,'training_n':int(train.sum()),'test_n':int(test.sum()),
                          'fitted_label_through':pd.Timestamp(last_label,unit='ms',tz='UTC').isoformat()})
            print('Evaluation',target,year,'complete',flush=True)
        yy=np.array(labels);ww=np.array(weeks);yr=np.array(years);tt=np.array(times)
        predictions={name:np.array(values) for name,values in forecasts.items()}
        predictions['selected']=predictions[selected[target]].copy()
        if target=='direction':predictions['half']=np.full(len(yy),.5)
        if target=='terminal_range_exit':
            from scipy.special import ndtr
            lookup=np.searchsorted(samples['at'],tt)
            up=f['distance_up'][samples['index'][lookup]];down=f['distance_down'][samples['index'][lookup]]
            predictions['normal_geometry']=np.clip(ndtr(-up)+ndtr(-down),1e-6,1-1e-6)
        report={'selected_config':selected[target],'metrics':{name:metrics(yy,p) for name,p in predictions.items()},'folds':folds}
        report['per_year']={str(year):{name:metrics(yy[yr==year],p[yr==year]) for name,p in predictions.items()} for year in np.unique(yr)}
        target_comparisons=[]
        for baseline in ['prior','raw_logistic','raw_boost']+(['half'] if target=='direction' else []):
            diff=(predictions['selected']-yy)**2-(predictions[baseline]-yy)**2
            effect=S.block_difference(diff,ww)
            effect.update(target=target,baseline=baseline,
                          period_means={label:float(diff[mask].mean()) for label,mask in [('2019_2022',yr<=2022),('2023_onward',yr>=2023)]},
                          improving_years=int(sum(diff[yr==year].mean()<0 for year in np.unique(yr))))
            comparisons.append(effect);target_comparisons.append(effect)
        report['comparisons']=target_comparisons;results[target]=report
        arrays[target+'_time']=tt;arrays[target+'_labels']=yy
        for name,p in predictions.items():arrays[target+'_'+name]=p
    for effect,q in zip(comparisons,S.bh([x['p'] for x in comparisons])):
        effect['q']=float(q)
        effect['passed']=bool(effect['ci95'][1]<0 and q<.05 and all(value<0 for value in effect['period_means'].values()))
    for target,report in results.items():
        criteria=report['comparisons'];raw=next(c for c in criteria if c['baseline']=='raw_boost')
        report['structural_increment_supported']=bool(all(c['passed'] for c in criteria) and raw['improving_years']>=6)
        report['status']='retrospective_candidate' if report['structural_increment_supported'] else 'experimental_no_structural_increment_demonstrated'
    final={'source':'data/XAUUSD_m30.json','source_bars':len(m['t']),
           'development_years':[2013,2018],'evaluation_years':[2019,2026],
           'refit':'Annual, preceding three calendar years; labels purged at next year boundary.',
           'horizon_hours':4,'horizon_bars':8,'development':development,'targets':results,
           'limitations':['Retrospective: gold history and previous evaluation results were already viewed.',
             'One instrument, no independent future/other-market confirmation.',
             'Annual model adaptation is causal, but prospective performance remains unknown.',
             'Probability of displacement or terminal exit does not imply directional/profit skill.']}
    (out/'results.json').write_text(json.dumps(final,allow_nan=False,indent=2)+'\n')
    np.savez_compressed(out/'forecasts.npz',**arrays)
    print('Saved',out/'results.json',flush=True)
    return final


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,default=OUT)
    args=ap.parse_args();evaluate(args.out)
