# Animation audits (Higgsfield session, from docs/HIGGS_REQUEST_ANIMATION.md)

- `posture_audit.*`: section 1, 88 UAL clips (lean, pitch, shoulders, centre of mass, contacts, root). `posture_audit_note.json` lists clips that already lean > 4° and idle/walk clips with shoulder difference > 3 cm. The UAL rest pose itself pitches +6.5° forward. Contacts: `planted` = foot clearance only (the clips are treadmill loops, feet slide in model space). rokoko/sekiro skipped (no UAL bone names).
- `carry_predicted.png`, `carry_targets_from_code.json`: section 3, a Python port of `motion/carry.rs` (`tools/anim/carry_predict.py`) on idle/walk per weapon. Made against the code BEFORE ad68f63, when the lean sat on spine_02/03 only and barely moved the pelvis→chest line (it found that); re-run on ad68f63 for the current numbers.
- Tools: `tools/anim/clip_fk.py` (vectorised glTF clip FK), `posture_audit.py`, `carry_predict.py`. Need `pygltflib`, numpy, matplotlib.
