B=target/release/gb_lab; n=$1; shift; $B stand "$@" --out diag/$n.jsonl > diag/$n.json; python3 -c "
import json,re;fix=lambda s: re.sub(r'(-?inf|NaN)','null',s)
r=json.loads(fix(open('diag/$n.json').read()));L=[json.loads(fix(l)) for l in open('diag/$n.jsonl')][1:]
t=next((x['tick'] for x in L if x['com'][1] is not None and x['com'][1]<0.5),-1)
print('$n'.ljust(6),r['com_min'],r['com_final'],r['drift_final'],r['flips_per_sec'],r['peak_sat'],'collapse@',t,'fell',r['fell_through_floor'])"
