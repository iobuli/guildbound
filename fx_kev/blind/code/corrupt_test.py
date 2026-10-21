import numpy as np, pandas as pd
from lib import *
for name,kind in [('USDJPY_H1','USDJPY'),('SOLUSDT_H1','SOL')]:
    h1=load(name).iloc[-6000:].astype(float)
    d1=load('USDJPY_D1') if kind=='USDJPY' else daily_from_h1(h1)
    if kind=='USDJPY': d1=d1[d1.index>=h1.index[0]-pd.Timedelta('120D')]
    rng=np.random.default_rng(0); bad=0
    full=feats(h1,d1,kind)
    for t in h1.index[[3000,4200,5500]]:
        h2=h1.copy(); i=h2.index.get_loc(t)+1   # bars after bar t (t closed at t+1h)
        n=len(h2)-i
        for col in ['open','high','low','close','volume']: h2.iloc[i:,h2.columns.get_loc(col)]=rng.random(n)*100+1
        d2=d1.copy().astype(float); d2.loc[d2.index+pd.Timedelta('1D')>t+pd.Timedelta('1h'),['open','high','low','close','volume']]=rng.random()*100+1
        if kind=='SOL': d2=daily_from_h1(h2); 
        c2=feats(h2,d2,kind)
        a=full.loc[:t].drop(columns=[]); b=c2.loc[:t]
        ok=np.allclose(a.values.astype(float),b.values.astype(float),equal_nan=True)
        if not ok:
            bad+=1; print(name,t,'DIFF cols',[k for k in a.columns if not np.allclose(a[k].astype(float),b[k].astype(float),equal_nan=True)])
    print(name,'corruption test failures:',bad)
