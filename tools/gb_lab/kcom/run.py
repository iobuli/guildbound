import json,re,subprocess,time
from concurrent.futures import ThreadPoolExecutor
V={'base':'--kb 5 --kbd 0.3 --kp 4000 --kd 126','kb4':'--kb 4 --kbd 0.3 --kp 4000 --kd 126','kb6':'--kb 6 --kbd 0.3 --kp 4000 --kd 126','kbd0.6':'--kb 5 --kbd 0.6 --kp 4000 --kd 126','kp3200':'--kb 5 --kbd 0.3 --kp 3200 --kd 113','kp5000':'--kb 5 --kbd 0.3 --kp 5000 --kd 141'}
K=['0.07','0.08','0.09','0.10','0.11','0.12']
jobs=[(k,v,va) for k in K for v,va in V.items()]+[(k,'80s',V['base']+' --ticks 9600') for k in K]
def run(j):
  k,v,a=j; path=f"kcom/k{k}_{v}.json"
  cmd=f"target/release/gb_lab stand --scaled --no_clamp --balance --substeps 12 --foot_damp 40 --k_com {k} {a}".split()
  t=time.time();o=subprocess.run(cmd,capture_output=True,text=True).stdout;t=time.time()-t
  open(path,'w').write(o); r=json.loads(re.sub(r'(-?inf|NaN)','null',o)); r['ms']=t*1000/r['ticks']; return k,v,r
with ThreadPoolExecutor(7) as ex: R=list(ex.map(run,jobs))
ok=lambda r:(r['drift_final'] or 9)<0.05 and (r['com_final'] or 0)>0.95 and r['flips_per_sec']<0.5 and (r['peak_sat'] or 9)<0.9
for k in K:
  d={v:r for kk,v,r in R if kk==k}; g=[d[v] for v in V]; dr=[x['drift_final'] or 99 for x in g]
  e=d['80s']
  print(f"k_com {k}: passes {sum(map(ok,g))}/6, drift {min(dr):.3f}-{max(dr):.3f}, com_final min {min(x['com_final'] or 0 for x in g):.3f}, sat max {max(x['peak_sat'] for x in g):.2f}, ms {d['base']['ms']:.2f} | 80s {e['com_final']:.3f}/{e['drift_final']:.3f} {'PASS' if ok(e) else 'fail'} | "+' '.join(f"{v}={d[v]['drift_final']:.3f}" for v in V))
