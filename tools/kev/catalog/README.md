# Animation catalogue + spatial smoothing

`catalog.py` keeps an index (`index.json`, committed) of every animation with its source (a clip in a GLB, or a gridded pose
sheet), tags and before/after metrics. The reference data - Kev landmark tracks at 60 fps, raw (`NAME__raw.json`) and
smoothed (`NAME.json`) - lives in `staging/kev/refs` (git-ignored); `catalog.py rebuild` regenerates it from the sources.

    python3 -I catalog.py add-clip  jab ual1.glb Punch_Jab --tags punch,fist
    python3 -I catalog.py add-sheet sheet_cross ../grid/sheets/cross.json --tags punch,sheet
    python3 -I catalog.py smooth all [--min-cut 4 --beta 6]
    python3 -I catalog.py list | info NAME | rebuild

Studio: `cat list`, `cat info NAME`, `cat play NAME` (bakes the smoothed reference onto the loaded rig and plays it).

`spatial.py` smooths in space, not just time: an adaptive zero-phase One-Euro filter per landmark (fast strikes keep their
peaks, slow drift and keyframe seams flatten), foot lock (low, slow feet pinned in x/z), limb-length relaxation (each segment
pulled toward its median length). Defaults (min-cut 4 Hz, beta 6) cut jerk to ~25-40 % while wrist reach stays within ~2 % and
peak wrist speed within ~25 %.
