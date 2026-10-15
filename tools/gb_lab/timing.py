import json,re,subprocess,sys,time
rows=[("6","3",""),("6","5",""),("6","3","--cff 3"),("6","5","--cff 3"),("6","3","--cdr 20"),("6","5","--cdr 20"),("12","3",""),("12","5",""),("24","3",""),("24","5","")]
for ss,kb,ex in rows:
  cmd=f"target/release/gb_lab stand --scaled --no_clamp --balance --kp 4000 --kd 126 --kbd 0.3 --kb {kb} --substeps {ss} {ex}".split()
  t=time.time();o=subprocess.run(cmd,capture_output=True,text=True).stdout;t=time.time()-t
  r=json.loads(re.sub(r'(-?inf|NaN)','null',o))
  print(f"ss{ss} kb{kb} {ex}".ljust(20),r['com_min'],r['com_final'],r['drift_final'],r['flips_per_sec'],r['peak_sat'],"%.3f ms/tick"%(t*1000/r['ticks']))
