"""Export the locked predictive-redesign comparison without fitting/tuning models."""
import os
os.environ.setdefault('MPLCONFIGDIR','/tmp/kev-matplotlib')
import json
from pathlib import Path
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT=Path(__file__).resolve().parents[1]
r=json.loads((ROOT/'out/nucleus/gold-predictive-redesign/results.json').read_text())
colors={'prior':'#aab8ae','raw_boost':'#78cbb1','selected':'#e4c478'}
bg='#111917';panel='#192521';ink='#e7eee8';muted='#aab8ae';pink='#e49db8'
plt.rcParams.update({'font.family':'DejaVu Sans','text.color':ink,'axes.labelcolor':muted,'xtick.color':muted,'ytick.color':muted,'axes.edgecolor':'#3b4b42','axes.facecolor':panel,'figure.facecolor':bg,'font.size':10})
fig=plt.figure(figsize=(15,12),layout='constrained');gs=fig.add_gridspec(3,3,height_ratios=[.6,1,1.35])
fig.suptitle('Predictive redesign: useful event probabilities, no demonstrated direction skill',x=.03,ha='left',fontsize=19,weight='bold')
a=fig.add_subplot(gs[0,:]);a.set_axis_off()
for x,title,body in [(0,'RANGE IDENTITY','12 ranges → unique high/low pairs\nRepeated boundaries count once'),(.34,'EVENT MEANING','Fresh breaches / rejections\nkept separate from memory expiry'),(.68,'FORECAST TARGETS','Direction · large displacement · range exit\nAnnual fits use only prior-year labels')]:
 a.text(x,.82,title,color=colors['selected'],weight='bold',transform=a.transAxes);a.text(x,.58,body,va='top',linespacing=1.7,transform=a.transAxes)
a.text(0,.04,'Configurations selected on 2013–2018 only; locked annual walk-forward evaluation on 2019–July 2026. Retrospective gold data, not independent confirmation.',color=muted,transform=a.transAxes,fontsize=9)
labels={'direction':'Direction: next close higher','large_displacement':'Large four-hour displacement','terminal_range_exit':'Terminal close outside known range'}
for j,target in enumerate(labels):
 report=r['targets'][target];metrics=report['metrics'];a=fig.add_subplot(gs[1,j]);a.set_axis_off();a.set_title(labels[target],loc='left',fontsize=12,weight='bold',pad=12)
 prior=metrics['prior']['brier'];selected=metrics['selected']['brier'];gain=(prior-selected)/prior*100
 a.text(.02,.8,f'{abs(gain):.2f}%',fontsize=34,weight='bold',color=colors['selected'] if gain>0 else pink,transform=a.transAxes)
 a.text(.02,.61,('lower' if gain>0 else 'higher')+' probability error than historical frequency',fontsize=10,transform=a.transAxes)
 a.text(.02,.43,'Selected: '+report['selected_config'],fontsize=10,color=muted,transform=a.transAxes)
 a.text(.02,.25,f'Selected Brier {selected:.6f}\nStrong raw baseline {metrics["raw_boost"]["brier"]:.6f}',linespacing=1.8,transform=a.transAxes)
 a.text(.02,.04,'No added-structure criterion passed.',color=pink,fontsize=10,transform=a.transAxes)
for j,target in enumerate(labels):
 report=r['targets'][target];a=fig.add_subplot(gs[2,j]);a.plot([0,1],[0,1],color=muted,ls='--',alpha=.7,label='Perfect calibration')
 for name in ['prior','raw_boost','selected']:
  bins=report['metrics'][name]['calibration'];x=[b['mean_forecast'] for b in bins];y=[b['observed_frequency'] for b in bins]
  a.plot(x,y,'o-',color=colors[name],label={'prior':'Frequency only','raw_boost':'Strong raw-price model','selected':'Selected model'}[name],lw=1.5)
 for b in report['metrics']['selected']['calibration']:
  a.annotate('n='+str(b['n']),(b['mean_forecast'],b['observed_frequency']),xytext=(6,7),textcoords='offset points',color=ink,fontsize=7)
 a.set_xlim(0,1);a.set_ylim(0,1);a.set_xlabel('Forecast probability');a.set_ylabel('Observed event frequency');a.set_title('Held-out probability calibration',loc='left',fontsize=11)
 a.grid(alpha=.15)
 if j==0:a.legend(loc='upper left',fontsize=8,facecolor=panel,edgecolor='#3b4b42',labelcolor=ink)
 if target=='direction':text='Selected forecasts score worse than 50/50.\nDo not interpret an upward probability as a validated signal.'
 elif target=='large_displacement':text='Improves over frequency in all 8 evaluation years.\nThe event-state layer does not beat the raw model.'
 else:text='Improves over frequency in all 8 evaluation years.\nA normal-distance geometry baseline is better still.'
 a.text(0,-.25,text,transform=a.transAxes,fontsize=9,color=muted)
out=ROOT/'docs/visuals';out.mkdir(exist_ok=True)
fig.savefig(out/'gold-predictive-redesign.png',dpi=150,bbox_inches='tight')
fig.savefig(out/'gold-predictive-redesign.pdf',bbox_inches='tight')
print('Saved predictive visual')
