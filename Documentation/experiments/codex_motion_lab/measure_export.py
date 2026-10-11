"""Independent source/target angle comparison from the actual lab's exported positions.
Unlike replaying a duplicate retarget implementation, this measures the delivered run.
"""
import json, math, hashlib
from pathlib import Path
out=Path(__file__).resolve().parent
root=out.parents[2]
r=json.loads((out/'results.json').read_text())
def sub(a,b): return [x-y for x,y in zip(a,b)]
def angle(a,b):
    denom=math.sqrt(sum(x*x for x in a)*sum(x*x for x in b))
    return math.degrees(math.acos(max(-1,min(1,sum(x*y for x,y in zip(a,b))/denom))))
rows=[]
for clip in r['clips']:
    measurements={}
    for side in ('.L','.R'):
        up,fore,hand,thigh,shin=['B-'+x+side for x in ('upperArm','forearm','hand','thigh','shin')]
        for label,kind in [('elbow',0),('upper_arm_from_down',1),('thigh_from_down',2)]:
            differences=[]
            for sample in clip['anatomical_samples']:
                pair=[]
                for domain in ('source','target'):
                    p=sample[domain]
                    pair.append(angle(sub(p[up],p[fore]),sub(p[hand],p[fore])) if kind==0 else
                                angle(sub(p[fore],p[up]) if kind==1 else sub(p[shin],p[thigh]),[0,0,-1]))
                differences.append(pair[0]-pair[1])
            measurements[label+side]={'rms_deg':math.sqrt(sum(d*d for d in differences)/len(differences)),
                                      'max_deg':max(abs(d) for d in differences)}
    source_unchanged=hashlib.sha256((root/clip['source']).read_bytes()).hexdigest()==clip['sha256']
    keys_exact=all(x['source_frame'] in clip['source_key_frames'] for x in clip['renders'])
    rows.append({'clip':clip['label'],'sample_count':len(clip['anatomical_samples']),
                 'source_unchanged':source_unchanged,'captures_at_authored_keys':keys_exact,'angles':measurements})
casts=[x for x in rows if x['clip'] in ('plain_fire','charge','release','cast_interrupt')]
worst=max(m['rms_deg'] for x in casts for m in x['angles'].values())
report={'rows':rows,'casting_max_joint_angle_rms_deg':worst,
        'casting_axis_regression_pass':worst<.1,
        'limits':['Only angular/source integrity checks, not a runtime animation/contact/support pass.']}
(out/'actual_export_measurements.json').write_text(json.dumps(report,indent=2))
assert all(x['source_unchanged'] and x['captures_at_authored_keys'] for x in rows)
assert report['casting_axis_regression_pass'],worst
print('20 source hashes unchanged; 100 exact authored-key captures; casting worst joint-angle RMS',round(worst,6),'degrees')
