import os
os.environ.setdefault('MPLCONFIGDIR', '/tmp/kev-matplotlib')
import sys,json
from pathlib import Path
import numpy as np
import pandas as pd
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import matplotlib.dates as md
import gold_information_study as S
root=S.ROOT
r=json.loads((root/'out/nucleus/gold-information-study/results.json').read_text())
out=root/'docs/visuals';out.mkdir(exist_ok=True)
m=S.load(); t=pd.to_datetime(m['t']+S.BASE,unit='ms',utc=True)
ink='#e7eee8'; muted='#aab8ae'; bg='#111917'; panel='#192521'; gold='#e4c478'; teal='#78cbb1';pink='#e49db8'
plt.rcParams.update({'font.family':'DejaVu Sans','text.color':ink,'axes.labelcolor':muted,'xtick.color':muted,'ytick.color':muted,'axes.edgecolor':'#3b4b42','axes.facecolor':panel,'figure.facecolor':bg,'font.size':10})
fig=plt.figure(figsize=(15,13),layout='constrained');gs=fig.add_gridspec(3,2,height_ratios=[1.5,1,1])
fig.suptitle('Gold: visible structure does not automatically predict the next move',fontsize=20,fontweight='bold',x=.03,ha='left')
a=fig.add_subplot(gs[0,:]);sel=(t>=pd.Timestamp('2026-07-01',tz='UTC'))
h=S.rolling(m['h'],104,'max');l=S.rolling(m['l'],104,'min');mid=(h+l)/2
prevh=np.r_[np.nan,h[:-1]];prevl=np.r_[np.nan,l[:-1]];prev=np.r_[np.nan,mid[:-1]]
expiry=(mid!=prev)&~((m['h']>prevh)|(m['l']<prevl))&np.isfinite(prev)
gap=np.flatnonzero(np.diff(m['t'][sel])!=S.BASE)+1
plot_t=np.insert(md.date2num(t[sel].to_pydatetime()),gap,np.nan)
broken=lambda values: np.insert(values[sel],gap,np.nan)
a.plot(plot_t,broken(m['c']),color=gold,lw=1,label='Raw observed close')
a.plot(plot_t,broken(h),color=muted,lw=1,alpha=.6,label='104-bar retained high / low');a.plot(plot_t,broken(l),color=muted,lw=1,alpha=.6)
a.plot(plot_t,broken(mid),color=teal,lw=1.4,label='Their midpoint')
a.scatter(t[sel&expiry],mid[sel&expiry],s=13,color=pink,zorder=5,label='Midpoint changed without a new high/low')
a.set_title('1  Looking outside Kev: old extrema can drag a line after price has moved elsewhere',loc='left',weight='bold',fontsize=13,pad=12)
a.set_xlim(t[sel][0],t[sel][-1]);a.xaxis.set_major_formatter(md.DateFormatter('%d %b'));a.xaxis_date();a.set_ylabel('XAUUSD');a.grid(alpha=.12)
a.legend(loc='upper left',fontsize=9,facecolor=panel,edgecolor='#3b4b42',labelcolor=ink)
a.text(.01,.02,f'2019 onward: {r["raw_organization"]["boundary_change_accounting"]["expiry_only_fraction_of_changes"]:.1%} of midpoint changes are expiry-only; {r["raw_organization"]["observed"]["midpoint_persistence"]:.1%} are unchanged observations. Gaps are not interpolated.',transform=a.transAxes,color=ink,bbox=dict(facecolor=panel,edgecolor='none',alpha=.9),fontsize=10)
a=fig.add_subplot(gs[1,0]);tables=r['contacts']['tables'];fib=tables[:4];control=tables[4:];x=np.arange(4)
a.bar(x-.18,[z['approach_side_return_fraction']*100 for z in fib],width=.35,color=gold,label='Fibonacci');a.bar(x+.18,[z['approach_side_return_fraction']*100 for z in control],width=.35,color=teal,label='Nearby ordinary fraction')
a.axhline(50,color=muted,ls='--',lw=1);a.set_ylim(35,62);a.set_xticks(x,[f'{f["ratio"]} / {c["ratio"]}' for f,c in zip(fib,control)]);a.set_ylabel('Move toward approach side after contact (%)')
a.set_title('2  Fibonacci contacts do not stand out from nearby levels',loc='left',weight='bold',fontsize=11,pad=12)
a.legend(fontsize=9,facecolor=panel,edgecolor='#3b4b42',labelcolor=ink)
a.text(.0,-.28,'First contact per level/week; next 8 contiguous M30 bars.\nMatched displacement difference CI: −0.028 to +0.035 volatility units.',transform=a.transAxes,color=muted,fontsize=9)
a=fig.add_subplot(gs[1,1]);metrics=r['predictive']['metrics'];names=['training_frequency','baseline','nucleus','nucleus_fib']
a.bar(np.arange(4),[metrics[n]['brier'] for n in names],color=[muted,teal,gold,pink],width=.6)
a.set_xticks(np.arange(4),['Constant\nfrequency','Simple\nprice model','+ Nucleus','+ Nucleus\n+ Fibonacci']);a.set_ylim(.2498,.2511);a.axhline(.25,color=muted,ls='--',lw=1)
a.set_ylabel('Held-out probability error (Brier; lower is better)');a.set_title('3  Added structure does not improve this forecast test',loc='left',weight='bold',fontsize=11,pad=12)
for i,n in enumerate(names):a.text(i,metrics[n]['brier']+.000045,f'{metrics[n]["brier"]:.5f}',ha='center',fontsize=9)
a.text(0,-.28,'Fit through 2018; evaluate 2019–July 2026.\n9,219 non-overlapping, gap-free four-hour target observations.',transform=a.transAxes,color=muted,fontsize=9)
a=fig.add_subplot(gs[2,0]);nulls=r['raw_organization']['null_samples'];key='same_side_synchronization'
vals=[nulls['quarter_permuted_bars'][key],nulls['quarter_permuted_48bar_blocks'][key]]
a.boxplot([np.array(z)*100 for z in vals],positions=[1,2],widths=.35,patch_artist=True,boxprops=dict(facecolor=teal,color=teal),medianprops=dict(color=bg),whiskerprops=dict(color=muted),capprops=dict(color=muted),flierprops=dict(markeredgecolor=muted))
a.axhline(r['raw_organization']['observed'][key]*100,color=gold,lw=2,label='Actual gold: 62.35%');a.set_xticks([1,2],['Shuffled bars\nwithin quarters','Shuffled 48-bar blocks\nwithin quarters']);a.set_ylabel('Three raw-window signs agree (%)');a.set_title('4  A structural lead: real gold is LESS synchronized',loc='left',weight='bold',fontsize=11,pad=12)
a.legend(fontsize=9,facecolor=panel,edgecolor='#3b4b42',labelcolor=ink)
a.text(0,-.32,'18 / 52 / 104 raw-M30 lookbacks; 99 paths per control.\nBoth corrected q = .03; nulls do not preserve all volatility dependence.',transform=a.transAxes,color=muted,fontsize=9)
a=fig.add_subplot(gs[2,1]);a.set_axis_off();a.set_title('What we learned — and what remains unproven',loc='left',weight='bold',fontsize=12,pad=12)
notes=[('MEMORY EXPIRY','A line can move because an old extreme disappears.\nThat is different from a new price impulse.'),('LEVEL DENSITY',f'{r["contacts"]["same_bar_matched_fraction"]:.1%} of matched Fibonacci/control contacts share a bar.\nVisual alignment alone is weak evidence.'),('STRUCTURAL DIFFERENCE','Cross-scale disagreement survives these two controls.\nThat is a research lead, not proof of emergence.'),('PREDICTION','No tested predictive increment passed the fixed rule.\nThis does not rule out every possible representation.')]
for i,(title,body) in enumerate(notes):
 y=.9-i*.24;a.text(.0,y,title,color=gold,weight='bold',fontsize=10,transform=a.transAxes);a.text(.0,y-.065,body,color=ink,fontsize=10,va='top',transform=a.transAxes)
fig.savefig(out/'gold-information-study.png',dpi=150,bbox_inches='tight')
fig.savefig(out/'gold-information-study.pdf',bbox_inches='tight')
print('Saved plots')
