# PRE-REGISTERED hypothesis registry (written before any test was run). Each entry: family, name, H(bars of H1), signal.
# Direction is part of the definition. Momentum ("follow") and fade variants are separate hypotheses.
import numpy as np, pandas as pd
def sg(cond_up,cond_dn):
    return pd.Series(np.where(cond_up,1,np.where(cond_dn,-1,0)),index=cond_up.index)
def build(f,kind):
    H=[]
    def add(fam,name,s,h): H.append((fam,name,s.fillna(0).astype(int),h))
    for h in (12,24):
        # A. compression -> breakout (follow / fade)
        for cq in (0.5,):
            comp=f.comp<cq
            up=comp&(f.c>f.hh24); dn=comp&(f.c<f.ll24)
            add('A_compress',f'comp<{cq}_breakout_follow_H{h}',sg(up,dn),h)
            add('A_compress',f'comp<{cq}_breakout_fade_H{h}',sg(dn,up),h)
        # plain 24-bar breakout of close, follow/fade (no compression)
        up=f.c>f.hh24; dn=f.c<f.ll24
        add('A_compress',f'brk24_follow_H{h}',sg(up,dn),h); add('A_compress',f'brk24_fade_H{h}',sg(dn,up),h)
        # E. run length 6 consecutive same-direction closes
        add('E_run',f'run6_follow_H{h}',sg(f.runup>=6,f.rundn>=6),h)
        add('E_run',f'run6_fade_H{h}',sg(f.rundn>=6,f.runup>=6),h)
        # drawdown: >6 ATR below 120-bar high -> long (mean-reversion); >6 ATR above 120-low -> short ; and follow
        add('E_dd',f'dd120_6atr_revert_H{h}',sg(f.dd120<-6,f.ru120>6),h)
        add('E_dd',f'dd120_6atr_follow_H{h}',sg(f.ru120>6,f.dd120<-6),h)
        # F. volume anomaly volz>2 with bar direction
        bu=(f.c>f.o); bd=(f.c<f.o)
        add('F_vol',f'volz2_follow_H{h}',sg((f.volz>2)&bu,(f.volz>2)&bd),h)
        add('F_vol',f'volz2_fade_H{h}',sg((f.volz>2)&bd,(f.volz>2)&bu),h)
        # G. autocorr regime
        m=f.mom6
        add('G_ac',f'ac>0.08_follow_mom6>1_H{h}',sg((f.ac>0.08)&(m>1),(f.ac>0.08)&(m<-1)),h)
        add('G_ac',f'ac<-0.08_fade_mom6>1_H{h}',sg((f.ac<-0.08)&(m<-1),(f.ac<-0.08)&(m>1)),h)
        # L. efficiency ratio trending vs continue
        add('L_er',f'er48>0.25_follow_H{h}',sg((f.er48>0.25)&(f.mom24>0),(f.er48>0.25)&(f.mom24<0)),h)
        add('L_er',f'er48<0.08_fade_mom24>1_H{h}',sg((f.er48<0.08)&(f.mom24<-1),(f.er48<0.08)&(f.mom24>1)),h)
        # J. exhaustion bar: range>3 ATR, close in outer 20%
        big=f.rngx>3
        add('J_exh',f'bigbar3_fade_H{h}',sg(big&(f.clv<0.2),big&(f.clv>0.8)),h)
        add('J_exh',f'bigbar3_follow_H{h}',sg(big&(f.clv>0.8),big&(f.clv<0.2)),h)
        # I. prior-day high/low: fake-out (wick through, close back) and breakout close
        add('I_pdhl',f'pd_fakeout_fade_H{h}',sg((f.h>f.d_pdh)&(f.c<f.d_pdh)&(f.o<f.d_pdh)==False,f.c<-1e18) if False else sg((f.l<f.d_pdl)&(f.c>f.d_pdl)&(f.o>f.d_pdl),(f.h>f.d_pdh)&(f.c<f.d_pdh)&(f.o<f.d_pdh)),h)
        add('I_pdhl',f'pd_break_follow_H{h}',sg((f.c>f.d_pdh)&(f.c.shift(1)<=f.d_pdh),(f.c<f.d_pdl)&(f.c.shift(1)>=f.d_pdl)),h)
        add('I_pdhl',f'hh20d_break_follow_H{h}',sg((f.c>f.d_hh20)&(f.c.shift(1)<=f.d_hh20),(f.c<f.d_ll20)&(f.c.shift(1)>=f.d_ll20)),h)
        add('I_pdhl',f'hh20d_break_fade_H{h}',sg((f.c<f.d_ll20)&(f.c.shift(1)>=f.d_ll20),(f.c>f.d_hh20)&(f.c.shift(1)<=f.d_hh20)),h)
        # D. multi-timeframe: D1 trend vs H1 24h counter-move resolves to D1 trend / to H1 move
        dtu=f.c>f.d_sma50; dtd=f.c<f.d_sma50
        add('D_mtf',f'd1trend_vs_h1counter_resolve_H{h}',sg(dtu&(f.mom24<-1),dtd&(f.mom24>1)),h)
        add('D_mtf',f'd1trend_vs_h1counter_h1wins_H{h}',sg(dtd&(f.mom24>1),dtu&(f.mom24<-1)),h)
        # C. weekend gap (after first bar closes)
        add('C_gap',f'wkgap0.5_fade_H{h}',sg(f.firstbar&(f.gapz<-0.5),f.firstbar&(f.gapz>0.5)),h)
        add('C_gap',f'wkgap0.5_follow_H{h}',sg(f.firstbar&(f.gapz>0.5),f.firstbar&(f.gapz<-0.5)),h)
        # H. round number rejection (JPY step 1.0; SOL step by magnitude)
        if kind=='USDJPY': step=pd.Series(1.0,index=f.index)
        else: step=pd.Series(10.0**np.floor(np.log10(f.c))/ (1 if False else 1),index=f.index)/2*1  # half-power-of-ten
        above=np.ceil(f.o/step)*step; below=np.floor(f.o/step)*step
        add('H_round',f'round_reject_fade_H{h}',sg((f.l<below)&(f.c>below)&(f.o>below),(f.h>above)&(f.c<above)&(f.o<above)),h)
        add('H_round',f'round_break_follow_H{h}',sg((f.c>above)&(f.o<above),(f.c<below)&(f.o>below)),h)
    # B. calendar: hour-of-day long-only H=1 (24) ; dow H=24 at 00:00 server (5); session momentum/fade
    for hr in range(24):
        add('B_hour',f'long_after_hour{hr:02d}_H1',pd.Series((f.hour==hr).astype(int),index=f.index),1)
    for d in range(5):
        add('B_dow',f'long_dow{d}_open_H24',pd.Series(((f.dow==d)&(f.hour==23)).astype(int),index=f.index),24)
    for h0 in (0,8,16):
        for fo in ('follow','fade'):
            ret=(f.c-f.c.shift(8))
            sig=(f.hour==(h0+7)%24)
            s=np.where(sig&(ret>0),1,np.where(sig&(ret<0),-1,0))*(1 if fo=='follow' else -1)
            add('B_sess',f'sess8h_from{h0:02d}_{fo}_H8',pd.Series(s,index=f.index),8)
    return H
