# No-glitch pass

Every smoothed catalogue clip and every solved frame goes through the same checks. A failure is a frame
range, not a single number, so a pop at the strike can be told from a slide during the plant.

    python3 -I tools/kev/quality/__main__.py summary
    python3 -I tools/kev/quality/__main__.py check NAME
    python3 -I tools/kev/quality/__main__.py check NAME --raw
    python3 -I tools/kev/quality/__main__.py contact NAME out.png

`summary` prints before/after frame counts from the index. `check` reads the smoothed reference in
`staging/kev/refs` (written by `catalog.py rebuild`). `contact` writes a side-view sheet of the feet and
the striking hand, raw in red and cleaned in green. It needs Pillow.

Studio console: `check NAME` (same report).

`catalog.py smooth` runs this after the spatial filter and stores the counts on the index:

- `quality.before` / `quality.after` - flagged frames per kind, after the spatial filter and after this pass
- `quality.raw` - the grabbed sample, before either pass
- `motion.state` - `ok` (the sample moves), `static` (the source clip really does not), or `frozen`
  (the source rotates and the sample stays put). A frozen entry is a sampling bug. `kev_melee` clips
  used to land here: those nodes are stored as matrices and the tracks were ignored.

What it will not pass:

- limb segments drifting off their rest length, or popping between frames
- knee bent backward, elbow popped forward, knee or elbow folded past its limit
- wrist, ankle, or toe past its range; toe bent backward; toe-off with the toe still straight up
- a foot through the floor
- a planted foot sliding (the ankle while it is down, the ball only on toe-off; a low sweep is left alone)
- a one-frame position pop, or a quaternion that flips and comes back
- a hand inside the torso or the head
- a finger joint bent away from the palm
- an extended hand whose palm faces backward

Fingers on a clip that does not already animate them blend between relaxed, blade (extended, slow), and
fist (extended, fast). A clip whose fingers already curl is only clamped, not restyled.

Solved frames (`/api/control`, `ref bake`) clean their targets before IK, re-aim a hinge that still
lands on the wrong side, and when the reference has no hand pose they twist the palm off the forearm
and curl the fingers with the arm's extension.
