B=target/release/gb_lab; n=$1; shift; $B stand --scaled --no_clamp "$@" --out bal/$n.jsonl > bal/$n.json; python3 -c "
import json,re;fix=lambda s: re.sub(r'(-?inf|NaN)','null',s)
r=json.loads(fix(open('bal/$n.json').read()));L=[json.loads(fix(l)) for l in open('bal/$n.jsonl')][1:]
t=next((x['tick'] for x in L if x['com'][1] is None or x['com'][1]<0.5),-1)
print('$n'.ljust(10),r['com_min'],r['com_final'],r['drift_final'],r['flips_per_sec'],r['peak_sat'],'fall@',t)"
