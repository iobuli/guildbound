import bisect, json, math, struct
from pathlib import Path
root = Path(__file__).resolve().parents[1]
raw = (root / 'assets/packs/hero/fighter.glb').read_bytes()
size = struct.unpack_from('<I', raw, 12)[0]
doc = json.loads(raw[20:20+size])
binary = raw[28+size:]
def accessor(index):
    a = doc['accessors'][index]
    v = doc['bufferViews'][a['bufferView']]
    count = {'SCALAR':1, 'VEC3':3, 'VEC4':4}[a['type']]
    offset = v.get('byteOffset',0) + a.get('byteOffset',0)
    return [struct.unpack_from('<' + 'f'*count, binary, offset+i*v.get('byteStride',count*4)) for i in range(a['count'])]
def mul(a,b):
    x,y,z,w=a; X,Y,Z,W=b
    return (w*X+x*W+y*Z-z*Y,w*Y-x*Z+y*W+z*X,w*Z+x*Y-y*X+z*W,w*W-x*X-y*Y-z*Z)
def rotate(q,v):
    return mul(mul(q,(*v,0)),(-q[0],-q[1],-q[2],q[3]))[:3]
nodes=doc['nodes']
parents={child:i for i,n in enumerate(nodes) for child in n.get('children',[])}
hand=next(i for i,n in enumerate(nodes) if n.get('name')=='B_R_Hand')
hip=next(i for i,n in enumerate(nodes) if n.get('name')=='B_Pelvis')
timing_path = root / 'assets/weapon_motion.json'
timings = json.loads(timing_path.read_text()) if timing_path.exists() else {}
for animation in doc['animations']:
    if 'sword' not in animation.get('name',''): continue
    tracks=[]
    for channel in animation['channels']:
        sampler=animation['samplers'][channel['sampler']]
        tracks.append((channel['target']['node'],channel['target']['path'],[v[0] for v in accessor(sampler['input'])],accessor(sampler['output'])))
    duration=max(t[-1] for _,_,t,_ in tracks)
    points=[]
    for frame in range(round(duration*30)+1):
        t=min(frame/30,duration)
        local=[{k:n.get(k, d) for k,d in [('translation',(0,0,0)),('rotation',(0,0,0,1)),('scale',(1,1,1))]} for n in nodes]
        for index,path,times,values in tracks:
            a=max(0,bisect.bisect_right(times,t)-1); b=min(a+1,len(times)-1)
            u=(t-times[a])/max(times[b]-times[a],1e-8)
            A=values[a]; B=values[b]
            if path=='rotation' and sum(x*y for x,y in zip(A,B))<0: B=tuple(-x for x in B)
            v=tuple(x+(y-x)*u for x,y in zip(A,B))
            if path=='rotation':
                mag=math.sqrt(sum(x*x for x in v)); v=tuple(x/mag for x in v)
            local[index][path]=v
        cache={}
        def world(i):
            if i in cache:return cache[i]
            n=local[i]; p=n['translation']; q=n['rotation']; s=n['scale']
            if i in parents:
                P,Q,S=world(parents[i]); move=rotate(Q,tuple(x*y for x,y in zip(p,S)))
                p=tuple(x+y for x,y in zip(P,move)); q=mul(Q,q); s=tuple(x*y for x,y in zip(S,s))
            cache[i]=(p,q,s);return cache[i]
        P,Q,S=world(hand); H,_,_=world(hip)
        tip=rotate(Q,(-0.9,0.075,0.015))
        points.append(tuple(P[i]+tip[i]-H[i] for i in range(3)))
    speed=[math.dist(a,b)*30 for a,b in zip(points,points[1:])]
    peak=max(range(len(speed)),key=speed.__getitem__)
    if animation['name'] in ['u1_sword_attack','u2_sword_regular_c']:
        start=end=peak
        cutoff=speed[peak]*0.35
        while start>0 and speed[start-1]>=cutoff:start-=1
        while end+1<len(speed) and speed[end+1]>=cutoff:end+=1
        timings[animation['name']]=[round(max(0.001,(start+0.1)/30/duration),6),round((peak+0.5)/30/duration,6),round(min(0.999,(end+0.9)/30/duration),6)]
    print(animation['name'], 'duration',round(duration,3),'tip speed peak',round((peak+0.5)/30,3),'speed',round(speed[peak],2),'endpoint delta',round(math.dist(points[0],points[-1]),3))
timing_path.write_text(json.dumps(timings,indent=2)+'\n')
print('Contact timings:',timings)
