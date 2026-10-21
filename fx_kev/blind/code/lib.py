import numpy as np, pandas as pd
D='/home/user/fx_kev/blind/data/'
def load(n):
    df=pd.read_csv(D+n+'.csv',parse_dates=['time']).set_index('time').sort_index()
    return df[~df.index.duplicated()]
COST={'USDJPY':0.01,'SOL':0.001}  # price units (JPY) / fraction

def daily_from_h1(h1):
    g=h1.resample('1D').agg({'open':'first','high':'max','low':'min','close':'last','volume':'sum'}).dropna()
    return g

def feats(h1, d1, kind):
    """All features causal at close of each H1 bar (row = bar open time; known at open+1h).
    d1 indexed by day-open; usable from day-open+1D."""
    c,o,h,l,v=h1.close,h1.open,h1.high,h1.low,h1.volume
    f=pd.DataFrame(index=h1.index)
    pc=c.shift(1)
    tr=pd.concat([h-l,(h-pc).abs(),(l-pc).abs()],axis=1).max(axis=1)
    atr=tr.rolling(14).mean(); f['atr']=atr
    f['ret1']=c.pct_change() if kind=='SOL' else c-pc
    f['hour']=h1.index.hour; f['dow']=h1.index.dayofweek
    # compression
    sd12=np.log(c).diff().rolling(12).std(); sd120=np.log(c).diff().rolling(120).std()
    f['comp']=sd12/sd120
    f['hh24']=h.rolling(24).max().shift(1); f['ll24']=l.rolling(24).min().shift(1)
    f['c']=c; f['o']=o; f['h']=h; f['l']=l
    # run length
    up=(c>pc).astype(int); dn=(c<pc).astype(int)
    def run(s):
        g=(s!=s.shift()).cumsum(); return s.groupby(g).cumsum()*s
    f['runup']=run(up); f['rundn']=run(dn)
    f['dd120']=(c-h.rolling(120).max())/atr; f['ru120']=(c-l.rolling(120).min())/atr
    lv=np.log1p(v); f['volz']=(lv-lv.rolling(240).mean())/lv.rolling(240).std()
    r=np.log(c).diff(); f['ac']=r.rolling(120).corr(r.shift(1))
    f['er48']=(c-c.shift(48)).abs()/c.diff().abs().rolling(48).sum()
    f['mom6']=(c-c.shift(6))/atr; f['mom24']=(c-c.shift(24))/(atr*np.sqrt(24)); f['mom8']=(c-c.shift(8))/atr
    f['rngx']=(h-l)/atr.shift(1); f['clv']=(c-l)/(h-l).replace(0,np.nan)
    # gap after time break (weekend): first bar after >=3h hole
    dt=h1.index.to_series().diff()
    first=(dt>pd.Timedelta('3h'))
    f['firstbar']=first.values
    f['gapz']=np.where(first,(c-pc)/atr.shift(1),np.nan)
    # D1 context: completed days only
    d=d1.copy(); d['avail']=d.index+pd.Timedelta('1D')
    d['sma50']=d.close.rolling(50).mean(); d['sma20']=d.close.rolling(20).mean()
    d['pdh']=d.high; d['pdl']=d.low
    d['hh20']=d.high.rolling(20).max(); d['ll20']=d.low.rolling(20).min()
    dd=d.set_index('avail')[['close','sma50','sma20','pdh','pdl','hh20','ll20']].add_prefix('d_')
    key=pd.DataFrame({'t':h1.index+pd.Timedelta('1h')},index=h1.index)
    m=pd.merge_asof(key.reset_index(),dd.reset_index().rename(columns={'avail':'t'}),on='t',direction='backward').set_index('time')
    for k in dd.columns: f[k]=m[k].values
    return f
