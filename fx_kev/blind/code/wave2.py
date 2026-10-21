# WAVE 2 pre-registered (written after wave-1 discovery, before any wave-2 test). All counted.
import numpy as np, pandas as pd, lib, hyp
from run_disc import events, trade, stats, pval
res=[]
def rec(inst,fam,name,H,n,gross,net,t,note=''):
    res.append(dict(inst=inst,fam=fam,name=name,H=H,n=n,gross=gross,net=net,t=t,p=pval(t) if t==t else 1.0,note=note))
def evalsig(inst,fam,name,f,sig,H,kind,cutfrac=.6,costmult=1):
    N=len(f); cut=int(cutfrac*N); ev=events(sig,H,0,cut-50)
    if len(ev)<30: rec(inst,fam,name,H,len(ev),np.nan,np.nan,np.nan); return
    g,nt=trade(f,sig,H,ev,kind); n,m,t=stats(nt); rec(inst,fam,name,H,n,g.mean(),m,t)
# ---------- H1 wave-2 directional + vol
for kind,name in (('USDJPY','USDJPY_H1'),('SOL','SOLUSDT_H1')):
    h1=lib.load(name); d1=lib.load('USDJPY_D1') if kind=='USDJPY' else lib.daily_from_h1(h1)
    f=lib.feats(h1,d1,kind); N=len(f); cut=int(.6*N)
    # V: volatility (non-directional): y = log(range of next 24 bars / (atr*sqrt24))
    fh=h1.high[::-1].rolling(24).max()[::-1].shift(-1); fl=h1.low[::-1].rolling(24).min()[::-1].shift(-1)
    y=np.log((fh-fl)/(f.atr*np.sqrt(24))).values
    for nm,cond in (('comp<0.4',f.comp<0.4),('volz>2.5',f.volz>2.5),('bigbar>3atr',f.rngx>3),('firstbar_weekend',f.firstbar),('er48<0.08',f.er48<0.08)):
        c=cond.fillna(False).values
        ev=events(c.astype(int),24,0,cut-50)
        base=y[:cut-50]; base=base[~np.isnan(base)]
        ye=y[ev]; ye=ye[~np.isnan(ye)]
        if len(ye)<30: rec(kind,'V_vol',f'vol24_after_{nm}',24,len(ye),np.nan,np.nan,np.nan); continue
        # compare vs random thinned baseline (every 24th bar)
        b=base[::24]; d=ye.mean()-b.mean(); se=np.sqrt(ye.var(ddof=1)/len(ye)+b.var(ddof=1)/len(b))
        rec(kind,'V_vol',f'vol24_after_{nm}',24,len(ye),d,d,d/se,'NON-TRADEABLE log-range diff')
    # D1 directional hypotheses on daily bars
    D=d1.copy(); D.columns=[c for c in D.columns]
    fd=pd.DataFrame(index=D.index); c=D.close
    pc=c.shift(1); tr=pd.concat([D.high-D.low,(D.high-pc).abs(),(D.low-pc).abs()],axis=1).max(axis=1); atr=tr.rolling(14).mean()
    dl=c.diff(); gain=dl.clip(lower=0).rolling(2).mean(); loss=(-dl.clip(upper=0)).rolling(2).mean(); rsi2=100-100/(1+gain/loss.replace(0,np.nan))
    fd['o']=D.open; fd['c']=c
    rng=D.high-D.low; nr7=rng<=rng.rolling(7).min(); clv=(c-D.low)/rng.replace(0,np.nan)
    hh20=D.high.rolling(20).max().shift(1); ll20=D.low.rolling(20).min().shift(1)
    mom60=c-c.shift(60); mom5=c-c.shift(5)
    dn3=(dl<0)&(dl.shift(1)<0)&(dl.shift(2)<0); up3=(dl>0)&(dl.shift(1)>0)&(dl.shift(2)>0)
    def S(a,b): return pd.Series(np.where(a,1,np.where(b,-1,0)),index=D.index)
    DH=[('rsi2<10_long/>90_short',S(rsi2<10,rsi2>90),3),('rsi2_follow',S(rsi2>90,rsi2<10),3),
        ('3dn_long_3up_short',S(dn3,up3),2),('3d_follow',S(up3,dn3),2),
        ('brk20d_follow',S(c>hh20,c<ll20),5),('brk20d_fade',S(c<ll20,c>hh20),5),
        ('brk20d_follow',S(c>hh20,c<ll20),10),('brk20d_fade',S(c<ll20,c>hh20),10),
        ('nr7_clv_follow',S(nr7&(clv>.7),nr7&(clv<.3)),3),('nr7_clv_fade',S(nr7&(clv<.3),nr7&(clv>.7)),3),
        ('mom60_follow',S(mom60>0,mom60<0),20),('mom60_fade',S(mom60<0,mom60>0),20),
        ('rev5d_fade',S(mom5<-1.5*atr,mom5>1.5*atr),5),('rev5d_follow',S(mom5>1.5*atr,mom5<-1.5*atr),5)]
    for nm,s,H in DH:
        evalsig(kind,'D_daily',f'D1_{nm}_H{H}',fd,s.values,H,kind)
    # session range breakout on H1 (server-hour windows 0-7 -> signal at hour 8 bar close; 8-15 -> hour 16)
    day=h1.index.normalize()
    for (a,b) in ((0,7),(8,15)):
        m=(h1.index.hour>=a)&(h1.index.hour<=b)
        hi=h1.high.where(m).groupby(day).transform('max'); lo=h1.low.where(m).groupby(day).transform('min')
        at=(h1.index.hour==b+1)
        up=at&(h1.close>hi.values); dn=at&(h1.close<lo.values)
        for fo,(u,d_) in (('follow',(up,dn)),('fade',(dn,up))):
            s=pd.Series(np.where(u,1,np.where(d_,-1,0)),index=h1.index)
            evalsig(kind,'S_session',f'sessrng{a}-{b}_{fo}_H8',f,s.values,8,kind)
    # liquidation / climax reversion
    if kind=='SOL':
        for h in (6,12):
            cl=(f.volz>3)&(f.rngx>3)
            evalsig(kind,'J_climax',f'climax_volz3_rng3_fade_H{h}',f,np.where(cl&(f.c<f.o),1,np.where(cl&(f.c>f.o),-1,0)),h,kind)
            evalsig(kind,'J_climax',f'climax_volz3_rng3_follow_H{h}',f,np.where(cl&(f.c>f.o),1,np.where(cl&(f.c<f.o),-1,0)),h,kind)
# cross-asset lead-lag
u=lib.load('USDJPY_H1'); s=lib.load('SOLUSDT_H1')
fs=lib.feats(s,lib.daily_from_h1(s),'SOL'); fu=lib.feats(u,lib.load('USDJPY_D1'),'USDJPY')
for lead,lag,fl,fg,kg in (('USDJPY','SOL',fu,fs,'SOL'),('SOL','USDJPY',fs,fu,'USDJPY')):
    # lead signal known at close of its bar (open+1h); map to lag bars whose open >= that close time: align by shifting lead's index by 1h, asof to lag bar CLOSE time
    m24=fl.mom24.copy(); m24.index=m24.index+pd.Timedelta('1h')
    lagclose=pd.Series(fg.index+pd.Timedelta('1h'),index=fg.index)
    mm=pd.merge_asof(lagclose.rename('t').reset_index(),m24.rename('m').reset_index().rename(columns={'time':'t'}),on='t',direction='backward')['m'].values
    # require the lead timestamp to be recent (within 5h)
    for fo,sgn in (('follow',1),('fade',-1)):
        sig=np.where(mm>1.5,sgn,np.where(mm<-1.5,-sgn,0))
        # restrict to hours when both trade: mask lag bars where timestamps exist in lead
        evalsig(kg,'X_cross',f'{lead}_mom24>1.5_{fo}_H24',fg,sig,24,kg)
# M15 USDJPY replication of wave-1 families at H=48,96 bars
m15=lib.load('USDJPY_M15'); fm=lib.feats(m15,lib.load('USDJPY_D1'),'USDJPY')
Hs=hyp.build(fm,'USDJPY')
for fam,hn,s,H in Hs:
    if fam in('B_hour','B_dow','B_sess'): continue
    H2=H*4
    evalsig('USDJPY_M15','M15_'+fam,hn.replace(f'_H{H}',f'_H{H2}'),fm,s.values,H2,'USDJPY')
r=pd.DataFrame(res); r.to_pickle('../results_agent/disc_wave2.pkl')
pd.set_option('display.width',250); pd.set_option('display.max_rows',500)
print(len(r)); r['tm']=np.nan
print(r.sort_values('t',key=abs,ascending=False).head(25)[['inst','name','n','gross','net','t']].to_string())
print('nan',r.t.isna().sum())
