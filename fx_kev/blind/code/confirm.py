# ONE-SHOT confirmation of frozen hypotheses on the last 40% (after 50-bar embargo). Run once; no tuning afterwards.
import numpy as np, pandas as pd, lib, hyp
from run_disc import events, trade, stats, pval
rng=np.random.default_rng(1)
def boot(x,B=5000):
    m=[x[rng.integers(0,len(x),len(x))].mean() for _ in range(B)]; return np.percentile(m,[2.5,97.5])
out=[]
def seg(N): return int(.6*N)+50, N
# --- directional
def getsig(f,kind,hn): 
    for fam,n,s,H in hyp.build(f,kind):
        if n==hn: return s.values,H
jobs=[('SOL','SOLUSDT_H1','d1trend_vs_h1counter_resolve_H24',1),('USDJPY_M15','USDJPY_M15','hh20d_break_follow_H96',4),('USDJPY_M15','USDJPY_M15','er48>0.25_follow_H96',4)]
for inst,fn,hn,mult in jobs:
    kind='SOL' if inst=='SOL' else 'USDJPY'
    h=lib.load(fn); d1=lib.daily_from_h1(h) if kind=='SOL' else lib.load('USDJPY_D1')
    f=lib.feats(h,d1,kind)
    base=hn.replace('_H96','_H24') if mult==4 else hn
    s,H=getsig(f,kind,base); H=H*mult
    lo,hi=seg(len(f)); ev=events(s,H,lo,hi); g,nt=trade(f,s,H,ev,kind); n,m,t=stats(nt)
    th=[float(x.mean()) for x in np.array_split(nt,3)]
    out.append(dict(inst=inst,name=hn,n=n,gross=g.mean(),net=m,t=t,p=pval(t),ci=tuple(np.round(boot(nt),3)),thirds=np.round(th,2),win=(nt>0).mean()))
# --- volatility
def volconf(kind,fn,nm,condfn):
    h=lib.load(fn); d1=lib.daily_from_h1(h) if kind=='SOL' else lib.load('USDJPY_D1'); f=lib.feats(h,d1,kind)
    fh=h.high[::-1].rolling(24).max()[::-1].shift(-1); fl=h.low[::-1].rolling(24).min()[::-1].shift(-1)
    y=np.log((fh-fl)/(f.atr*np.sqrt(24))).values; lo,hi=seg(len(f))
    c=condfn(f).fillna(False).values.astype(int); ev=events(c,24,lo,hi); ye=y[ev]; ye=ye[~np.isnan(ye)]
    b=y[lo:hi-24][::24]; b=b[~np.isnan(b)]
    d=ye.mean()-b.mean(); se=np.sqrt(ye.var(ddof=1)/len(ye)+b.var(ddof=1)/len(b))
    bs=[ye[rng.integers(0,len(ye),len(ye))].mean()-b[rng.integers(0,len(b),len(b))].mean() for _ in range(5000)]
    th=[float(x.mean())-b.mean() for x in np.array_split(ye,3)]
    out.append(dict(inst=kind,name='vol24_after_'+nm,n=len(ye),gross=d,net=d,t=d/se,p=pval(d/se),ci=tuple(np.round(np.percentile(bs,[2.5,97.5]),3)),thirds=np.round(th,3),win=np.nan))
for kind,fn in (('USDJPY','USDJPY_H1'),('SOL','SOLUSDT_H1')):
    volconf(kind,fn,'comp<0.4',lambda f:f.comp<0.4)
    volconf(kind,fn,'bigbar>3atr',lambda f:f.rngx>3)
r=pd.DataFrame(out); pd.set_option('display.width',250); print(r.to_string()); r.to_pickle('../results_agent/confirm.pkl')
