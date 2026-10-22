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
peaks, slow drift and keyframe seams flatten; a landmark the filter would roughen is left raw), then foot lock (low, slow
ankles pinned in x/z, the ball keeping its offset). Defaults (min-cut 4 Hz, beta 6) cut jerk while wrist reach stays close
and a planted foot does not slide. The ball is held up by the sole pad, not clamped onto the floor line. Segment lengths are not pulled to a median: stretching a short limb invented motion and
made flips and fast kicks rougher. The solver clamps a reach at the bone length.
