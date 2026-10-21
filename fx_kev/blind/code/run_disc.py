import sys, json, numpy as np, pandas as pd
import lib, hyp
from math import erf, sqrt
def pval(t): return 1-erf(abs(t)/sqrt(2))
def events(sig,H,lo,hi):
    idx=np.flatnonzero(sig[lo:hi])+lo; out=[]; last=-1
    for i in idx:
        if i<=last: continue
        if i+1+H>=len(sig): break
        out.append(i); last=i+H   # next signal must be at/after exit bar
    return np.array(out,int)
def trade(f,sig,H,ev,kind):
    o=f.o.values; e=o[ev+1]; x=o[ev+1+H]; d=sig[ev]
    if kind=='USDJPY': g=d*(x-e)/0.01; cost=1.0   # pips
    else: g=d*(x/e-1)*100; cost=0.1                 # percent
    return g, g-cost
def stats(net):
    n=len(net); m=net.mean() if n else np.nan
    se=net.std(ddof=1)/np.sqrt(n) if n>2 else np.nan
    return n,m,m/se if se==se and se>0 else np.nan
def run(kind,name,d1):
    h1=lib.load(name); f=lib.feats(h1,d1,kind); Hs=hyp.build(f,kind); N=len(h1)
    cut=int(.6*N); rows=[]
    for fam,hn,s,H in Hs:
        sig=s.values; ev=events(sig,H,0,cut-50)
        if len(ev)<30: rows.append(dict(inst=kind,fam=fam,name=hn,H=H,n=len(ev),gross=np.nan,net=np.nan,t=np.nan,p=1.0)); continue
        g,n_=trade(f,sig,H,ev,kind); n,m,t=stats(n_)
        # sub-period sign stability (thirds)
        th=np.array_split(n_,3); sg3=[float(x.mean()) for x in th]
        rows.append(dict(inst=kind,fam=fam,name=hn,H=H,n=n,gross=g.mean(),net=m,t=t,p=pval(t),thirds=sg3))
    return pd.DataFrame(rows)
if __name__=='__main__':
    a=run('USDJPY','USDJPY_H1',lib.load('USDJPY_D1'))
    h=lib.load('SOLUSDT_H1'); b=run('SOL','SOLUSDT_H1',lib.daily_from_h1(h))
    r=pd.concat([a,b],ignore_index=True)
    r.to_pickle('../results_agent/disc_h1.pkl')
    pd.set_option('display.width',250); pd.set_option('display.max_rows',500)
    print(len(r),'hypotheses'); 
    print(r.sort_values('p').head(30)[['inst','name','n','gross','net','t','p']])
