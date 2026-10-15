import json,re,subprocess,time,sys,itertools
from concurrent.futures import ThreadPoolExecutor
V={'base':'--kb 5 --kbd 0.3 --kp 4000 --kd 126','kb4':'--kb 4 --kbd 0.3 --kp 4000 --kd 126','kb6':'--kb 6 --kbd 0.3 --kp 4000 --kd 126','kbd0.6':'--kb 5 --kbd 0.6 --kp 4000 --kd 126','kp3200':'--kb 5 --kbd 0.3 --kp 3200 --kd 113','kp5000':'--kb 5 --kbd 0.3 --kp 5000 --kd 141'}
O={'base':'','pull0.5':'--com_pull 0.5','pull1':'--com_pull 1','pull2':'--com_pull 2','tan':'--tan_full','tan+pull1':'--tan_full --com_pull 1'}
jobs=[]
for (o,oa),fd in itertools.product(O.items(),['', '--foot_damp 40']):
  name=o+('+fd40' if fd else '')
  for v,va in V.items(): jobs.append((name,v,f"{va} {oa} {fd}"))
  jobs.append((name,'80s',f"{V['base']} {oa} {fd} --ticks 9600"))
def run(j):
  n,v,a=j; cmd=("target/release/gb_lab stand --scaled --no_clamp --balance --substeps 12 "+a).split()
  t=time.time();o=subprocess.run(cmd,capture_output=True,text=True).stdout;t=time.time()-t
  r=json.loads(re.sub(r'(-?inf|NaN)','null',o));r['ms']=t*1000/r['ticks'];return n,v,r
with ThreadPoolExecutor(7) as ex: R=list(ex.map(run,jobs))
json.dump([[n,v,r] for n,v,r in R],open('tan/all.json','w'))
from collections import defaultdict
D=defaultdict(dict)
for n,v,r in R: D[n][v]=r
ok=lambda r:(r['drift_final'] or 9)<0.05 and (r['com_final'] or 0)>0.95 and r['flips_per_sec']<0.5 and (r['peak_sat'] or 9)<0.9
print('option | base: com min/final, drift, flips, sat, ms | drift range over 6 | passes/6 | 80s com_final/drift')
for n,d in D.items():
  b=d['base'];g=[d[k] for k in V];dr=[x['drift_final'] or 99 for x in g]
  print(f"{n:15s}| {b['com_min']:.3f}/{b['com_final']:.3f} {b['drift_final']:.3f} {b['flips_per_sec']:.2f} {b['peak_sat']:.2f} {b['ms']:.2f} | {min(dr):.3f}-{max(dr):.3f} | {sum(map(ok,g))}/6 | {d['80s']['com_final']:.3f}/{d['80s']['drift_final']:.3f} | "+' '.join(f"{k}={d[k]['drift_final']:.3f}" for k in V))
