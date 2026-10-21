# Grid poses

Read poses off a reference image with a fine grid, get an animation.

1. `python3 -I grid.py overlay IMG OUT.png --cell 10 --box x0 y0 x1 y1 --zoom 4` - uniform grid over one figure row (labels every 5 cells).
2. Read each joint as a fractional `[col, row]` into a sheet (`sheets/cross.json`) with a `sequence` of poses and times.
3. `GRID_IMAGES=<dir> python3 -I posesheet.py verify SHEET OUT.png` - draws the read joints back over the image to check the reading.
4. `python3 -I posesheet.py bake SHEET MODEL.glb NAME` - lifts each pose to 3D landmarks (side view: image gives forward + height,
   fixed body half-widths give left/right), solves them with Kev Studio's controller, eases between poses, writes
   `staging/kev/baked/MODEL__NAME.json`.
5. In the Studio: `model MODEL.glb`, `ref baked NAME`.

Limits: one side view per sheet, so depth comes from the half-width table, not the image; mean landmark error is about 10 cm
because the drawn figure's proportions differ from the rig's.
