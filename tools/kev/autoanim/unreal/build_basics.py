"""Build the local Sparking starter library from exact archive sequences."""
import json
from pathlib import Path
import urllib.request

import numpy as np
import retarget_glb as R
from sparking_library import ROOT, DATA


def build():
    catalog = json.load(urllib.request.urlopen('http://127.0.0.1:8765/api/sparking'))
    by_name = {c['name']: c for c in catalog['clips'] if c['character'] in ('0000', '9998_standard')}
    def decoded(name):
        item = by_name[name]
        request = urllib.request.Request('http://127.0.0.1:8765/api/sparking/load',
            data=json.dumps({'id': item['id']}).encode(),
            headers={'Content-Type': 'application/json', 'X-Client': 'user'})
        json.load(urllib.request.urlopen(request, timeout=120))
        return json.loads((DATA / (item['id'] + '.json')).read_text())

    inputs, report = [], []
    for number in (1, 2, 3):
        sections = [decoded(f'A_0000_C_ATC_a00{number}_R1_{number}_{part}') for part in ('IN', 'HT', 'OT')]
        merged = dict(sections[0], name=f'DBZ_Rush_0{number}', frames=[])
        merged['source'] = ' + '.join(s['source'] for s in sections)
        offset = 0.0
        for section in sections:
            if section['bones'] != merged['bones']:
                raise ValueError('Rush section skeleton mismatch')
            for frame in section['frames']:
                t = offset + frame['t']
                if not merged['frames'] or t > merged['frames'][-1]['t'] + 1e-7:
                    merged['frames'].append(dict(frame, t=t))
            offset += section['duration']
        merged['duration'] = offset
        # Measure source effector motion during the contact section, relative to pelvis.
        contact = sections[1]
        names = {b['name']: i for i, b in enumerate(contact['bones'])}
        parents = [b['parent'] for b in contact['bones']]
        positions = np.array([[m[:3, 3] for m in R.world([R.matrix(p) for p in f['pose']], parents)]
                              for f in contact['frames']])
        speeds = {}
        for limb, bone in [('hand_l', 'WRIST_L'), ('hand_r', 'WRIST_R'), ('foot_l', 'TOE1_L'), ('foot_r', 'TOE1_R')]:
            relative = positions[:, names[bone]] - positions[:, names['WAIST']]
            speeds[limb] = round(float(np.linalg.norm(np.diff(relative, axis=0), axis=1).max() * 0.6), 3)
        report.append(dict(clip=merged['name'], duration=offset,
            windup=sections[0]['duration']/offset,
            active=(sections[0]['duration']+sections[1]['duration'])/offset,
            effector_peak_mps=speeds))
        path = DATA / (merged['name'] + '_basics.json')
        path.write_text(json.dumps(merged), encoding='utf-8')
        inputs.append(path)
    for name, alias in [('A_0000_A_STD_b002_STD_G_LP', 'DBZ_Ground_Stance'),
                        ('A_0000_A_STD_d001_GRD_A_LP', 'DBZ_Guard'),
                        ('A_0000_A_STD_f001_CHGE_2LP', 'DBZ_Ki_Charge_Loop'),
                        ('A_0000_S_BST_a001_ry1_3HT1', 'DBZ_Ki_Release'),
                        ('A_9998_D_KDN_a100_RSLR_F_2FR', 'DBZ_Ki_Blast'),
                        ('A_9998_E_DMG_a005_DMM1_F_IN', 'DBZ_Hit_Body'),
                        ('A_9998_E_DMG_a006_DMM2_F_IN', 'DBZ_Hit_Head'),
                        ('A_9998_E_DMG_a012_DML1_R_IN', 'DBZ_Hit_Side'),
                        ('A_9998_A_STD_d011_GRD_BRKE', 'DBZ_Guard_Break'),
                        ('A_9998_B_MVE_a001_WAKG_F', 'DBZ_Walk_Loop'),
                        ('A_0000_B_MVE_c006_SP_GL_IN', 'DBZ_Step_Left'),
                        ('A_0000_B_MVE_c007_SP_GR_IN', 'DBZ_Step_Right'),
                        ('A_0000_B_MVE_c005_SP_GB_IN', 'DBZ_Step_Back')]:
        data = decoded(name)
        data['name'] = alias
        path = DATA / (alias + '_basics.json')
        path.write_text(json.dumps(data), encoding='utf-8')
        inputs.append(path)
    output = ROOT / 'assets/anim/sparking_combat.glb'
    R.convert(ROOT / 'assets/anim/ual2.glb', output, inputs)
    (DATA / 'basics-report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    build()
