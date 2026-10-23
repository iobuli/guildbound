"""Convert selected installed-game architecture/rocks to local reusable GLBs.

Geometry is preserved; neutral prototype materials replace Unreal material graphs.
The source path, normalized bounds and scale are retained in a manifest.
"""
import json
import os
from pathlib import Path
import subprocess
import numpy as np
import retarget_glb as R
from sparking_library import ROOT, DATA, CONFIG


def build():
    config = json.loads(CONFIG.read_text(encoding='utf-8-sig'))
    files = json.loads((DATA / 'full-index.json').read_text())['files']
    env = dict(os.environ, DOTNET_gcServer='0', KEV_UNREAL_AES_KEY=config['key'], KEV_UNREAL_MAPPINGS=config['mappings'])
    output = ROOT / 'assets/environment/sparking'
    output.mkdir(parents=True, exist_ok=True)
    manifest = []
    selections = [('rock', 'SM_BG_Cmn_RockA', 3.5),
                  ('wall', 'SM_CB_BG_000_Wall_House01_A_01', 5.0),
                  ('building', 'SM_BG_000_House04_A_01', 7.0)]
    for alias, name, size in selections:
        source = next(p for p in files if p.endswith('/' + name + '.uasset'))
        decoded = DATA / (alias + '_mesh.json')
        subprocess.run(['dotnet', 'D:/tmp/kev-env-probe/ArchiveProbe.dll', config['archives'], str(decoded), '--mesh', source],
                       cwd=ROOT, env=env, check=True)
        data = json.loads(decoded.read_text())
        positions = np.asarray(data['vertices'], dtype=np.float32)
        indices = np.asarray(data['indices'], dtype=np.uint32).reshape(-1, 3)[:, [0, 2, 1]].copy()
        if not np.isfinite(positions).all() or indices.max() >= len(positions):
            raise ValueError('Invalid decoded mesh')
        low, high = positions.min(axis=0), positions.max(axis=0)
        scale = size / max(float((high-low).max()), 0.001)
        positions = (positions - (low + high) * 0.5) * scale
        normals = np.zeros_like(positions)
        triangles = positions[indices]
        face = np.cross(triangles[:, 1]-triangles[:, 0], triangles[:, 2]-triangles[:, 0])
        for corner in range(3): np.add.at(normals, indices[:, corner], face)
        normals /= np.maximum(np.linalg.norm(normals, axis=1, keepdims=True), 1e-8)
        blob = bytearray()
        js = dict(asset={'version':'2.0','generator':'Kev archive environment converter'}, buffers=[{'byteLength':0}],
                  bufferViews=[], accessors=[], nodes=[{'mesh':0,'name':alias}], scenes=[{'nodes':[0]}], scene=0)
        def accessor(values, kind, component):
            offset = len(blob); raw=values.tobytes(); blob.extend(raw)
            while len(blob)%4: blob.append(0)
            js['bufferViews'].append({'buffer':0,'byteOffset':offset,'byteLength':len(raw)})
            entry={'bufferView':len(js['bufferViews'])-1,'componentType':component,'count':len(values),'type':kind}
            if kind=='VEC3': entry.update(min=values.min(axis=0).tolist(),max=values.max(axis=0).tolist())
            js['accessors'].append(entry)
            return len(js['accessors'])-1
        p=accessor(positions.astype('<f4'),'VEC3',5126)
        n=accessor(normals.astype('<f4'),'VEC3',5126)
        i=accessor(indices.flatten().astype('<u4'),'SCALAR',5125)
        js['materials']=[{'name':'prototype stone','pbrMetallicRoughness':{'baseColorFactor':[0.55,0.57,0.61,1], 'metallicFactor':0,'roughnessFactor':0.85}}]
        js['meshes']=[{'primitives':[{'attributes':{'POSITION':p,'NORMAL':n},'indices':i,'material':0}]}]
        js['buffers'][0]['byteLength']=len(blob)
        dest=output/(alias+'.glb')
        if not dest.exists(): R.statue.write_glb(dest,js,blob)
        manifest.append({'name':alias,'file':'environment/sparking/'+dest.name,'source':source,
                         'half':((high-low)*scale*0.5).tolist(),'scale':scale,'triangles':len(indices),
                         'vertices':positions.tolist(), 'indices':indices.tolist()})
    (output/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    print(json.dumps([{k:v for k,v in m.items() if k not in ('vertices','indices')} for m in manifest],indent=2))

if __name__ == '__main__': build()
