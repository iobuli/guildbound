"""Offline GLB -> normalized numeric tracks. The viewer never loads the source model."""
import argparse
import json
import math
import struct
from pathlib import Path
import numpy as np

NAMES = ['B_Pelvis', 'B_Spine2', 'B_Head', 'B_L_UpperArm', 'B_L_Forearm',
         'B_R_UpperArm', 'B_R_Forearm', 'B_L_Thigh', 'B_L_Calf',
         'B_R_Thigh', 'B_R_Calf', 'B_L_Foot', 'B_R_Foot']
PARENTS = [None, 0, 1, 1, 3, 1, 5, 0, 7, 0, 9, 8, 10]

def rotation(q):
    x, y, z, w = q / np.linalg.norm(q)
    return np.array([[1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)],
                     [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)],
                     [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)]])

def quaternion(m):
    if np.trace(m) > 0:
        s = math.sqrt(np.trace(m)+1)*2
        q = [(m[2,1]-m[1,2])/s, (m[0,2]-m[2,0])/s, (m[1,0]-m[0,1])/s, s/4]
    else:
        i = int(np.argmax(np.diag(m))); j = (i+1)%3; k = (i+2)%3
        s = math.sqrt(max(0, 1+m[i,i]-m[j,j]-m[k,k]))*2
        q = [0.,0.,0.,0.]; q[i]=s/4; q[j]=(m[j,i]+m[i,j])/s
        q[k]=(m[k,i]+m[i,k])/s; q[3]=(m[k,j]-m[j,k])/s
    q = np.array(q); return (q/np.linalg.norm(q)).tolist()

def bake(path, output):
    data = path.read_bytes(); size = struct.unpack_from('<I', data, 12)[0]
    doc = json.loads(data[20:20+size]); binary = data[28+size:]
    nodes = doc['nodes']; parents = {}
    for i, n in enumerate(nodes):
        for c in n.get('children', []): parents[c]=i
    by_name = {n.get('name'): i for i,n in enumerate(nodes)}
    bones = [by_name[n] for n in NAMES]
    def accessor(index):
        a=doc['accessors'][index]; v=doc['bufferViews'][a['bufferView']]
        assert a['componentType']==5126 and 'sparse' not in a
        count={'SCALAR':1,'VEC3':3,'VEC4':4}[a['type']]
        return np.ndarray((a['count'],count),dtype='<f4',buffer=binary,
            offset=v.get('byteOffset',0)+a.get('byteOffset',0),
            strides=(v.get('byteStride',count*4),4)).copy()
    clips={}
    for kind, name in [('idle','Unarmed-Idle'),('walk','nc_walk_forward'),('run','nc_run_forward')]:
        anim=next(a for a in doc['animations'] if a.get('name')==name)
        channels=[]
        for c in anim['channels']:
            s=anim['samplers'][c['sampler']]
            assert s.get('interpolation','LINEAR') in ['LINEAR','STEP']
            channels.append((c['target']['node'],c['target']['path'],accessor(s['input'])[:,0],
                             accessor(s['output']),s.get('interpolation','LINEAR')))
        clips[kind]=(name, channels)
    def pose(channels, time):
        local=[{'translation':np.array(n.get('translation',[0,0,0]),dtype=float),
                'rotation':np.array(n.get('rotation',[0,0,0,1]),dtype=float),
                'scale':np.array(n.get('scale',[1,1,1]),dtype=float)} for n in nodes]
        for node, prop, times, values, mode in channels:
            if prop not in local[node]: continue
            i=max(0,min(len(times)-1,int(np.searchsorted(times,time,side='right'))-1)); j=min(i+1,len(times)-1)
            u=0 if mode=='STEP' or times[j]<=times[i] else float(np.clip((time-times[i])/(times[j]-times[i]),0,1))
            a=values[i].astype(float); b=values[j].astype(float)
            if prop=='rotation':
                dot=float(a@b)
                if dot<0: b=-b; dot=-dot
                if dot<0.9995:
                    angle=math.acos(min(1,dot)); v=(a*math.sin((1-u)*angle)+b*math.sin(u*angle))/math.sin(angle)
                else: v=a*(1-u)+b*u
                v/=np.linalg.norm(v)
            else: v=a*(1-u)+b*u
            local[node][prop]=v
        cache={}
        def world(i):
            if i in cache: return cache[i]
            n=local[i]; m=np.eye(4); m[:3,:3]=rotation(n['rotation'])@np.diag(n['scale']); m[:3,3]=n['translation']
            if 'matrix' in nodes[i] and not channels: m=np.array(nodes[i]['matrix']).reshape(4,4).T
            cache[i]=world(parents[i])@m if i in parents else m
            return cache[i]
        return [world(i) for i in bones]
    base=pose(clips['idle'][1],0)
    reach=np.mean([np.linalg.norm(base[h][:3,3]-base[k][:3,3])+np.linalg.norm(base[k][:3,3]-base[f][:3,3])
                   for h,k,f in [(7,8,11),(9,10,12)]])
    metadata=json.loads(Path(str(path)+'.motion.json').read_text())
    assert metadata.get('verified') is True, 'Motion contact metadata must be verified'
    out={'schema':1,'source':path.name,'source_leg_reach_m':float(reach),'clips':{}}
    for kind,(name,channels) in clips.items():
        duration=max(float(c[2][-1]) for c in channels); count=max(2,round(duration*120))
        poses=[pose(channels,i*duration/count) for i in range(count)]
        floors=[min(p[f][1,3] for p in poses) for f in [11,12]]
        contacts=metadata['foot_contacts'].get(name)
        assert contacts, f'No verified contact curve for {name}'
        frames=[]
        for i,p in enumerate(poses):
            r=[m[:3,:3]/np.linalg.norm(m[:3,:3],axis=0) for m in p]
            rb=[m[:3,:3]/np.linalg.norm(m[:3,:3],axis=0) for m in base]
            delta=[a@b.T for a,b in zip(r,rb)]
            local=[np.eye(3) if parent is None else delta[parent].T@delta[j] for j,parent in enumerate(PARENTS)]
            feet=[]
            for side,f in enumerate([11,12]):
                rel=p[f][:3,3]-p[0][:3,3]
                feet.append([float(rel[0]/reach),float(max(0,p[f][1,3]-floors[side])/reach),float(rel[2]/reach)])
            phase=i/count*(len(contacts)-1); a=int(phase); b=min(a+1,len(contacts)-1)
            contact=[contacts[a][s]+(contacts[b][s]-contacts[a][s])*(phase-a) for s in [0,1]]
            frames.append({'feet':feet,'contact':contact,'rotations':[quaternion(m) for m in local]})
        out['clips'][kind]={'duration':duration,'frames':frames}
        print(kind, count, 'frames,',round(duration,3),'s')
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(out,separators=(',',':')),encoding='utf-8')
    print('Saved',output,'source leg reach',round(float(reach),4))

if __name__=='__main__':
    p=argparse.ArgumentParser(); p.add_argument('--source',type=Path,default=Path('assets/packs/hero/master.glb'))
    p.add_argument('--out',type=Path,default=Path('assets/motion/neutral_tracks.json'))
    a=p.parse_args(); bake(a.source,a.out)
