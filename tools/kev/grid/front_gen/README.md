# Front-view keyframe sheets for the nine generated moves (for T003)

Generated 22 Oct 2026 by Grok. Nothing was committed, and nothing in the repo or staging/ was touched.

## Files
- `<move>_front.png`: one sheet per move. 1280x720, a 2 rows x 4 columns grid, panels labelled f1..f8.
  Frame order is f1-f4 on the top row and f5-f8 on the bottom row, the same order as `tools/kev/grid/sheets/<move>.json`.
  Each panel is about 320x360 px, at panel (col, row) = ((i-1)%4, (i-1)//4).
- `all_front.png`: contact sheet of all nine.
- `spinning_back_kick_front_alt_backview.png`: an alternative where the body really turns its back to the camera during the kick (see notes).
- `ref/<move>_side_ref.png`: the side-view references used, re-drawn from the joint data in sheets/*.json.
  Orange is the "_l" (leading) joints and blue is the "_r" (trailing) joints. `render_side.py` re-draws them.

## Important caveats
- The original images (gen_sheet1/2/3.webp, gen_front.webp, staging/poses/ref) were NOT found on the box or on the
  user's PC (C:\ and D:\ were searched). The references were therefore re-drawn from the sheet JSON joint readings,
  and the new sheets do not reuse the source images' pixel layout or `cell=10` grid. A new sheet JSON for these needs
  its own `image` and per-pose coordinates, read with `grid.py overlay`. Use 2 rows x 4 panels, not the source's packed rows.
- These are AI-generated drawings, not measured renders. They are frontal and consistently two-tone, but their
  proportions and lateral spread are illustrative. Treat them as roughly as accurate as the side sheets.
- Colour convention: the mannequin is split down its midline. ORANGE is the half on the VIEWER'S LEFT (the character's
  own right), and BLUE is the half on the viewer's right. The leading/kicking side ("_l" in the sheets) was asked to be
  orange, except in the spinning back kick, where the kicking leg is the trailing one and is blue. The generator did not
  always follow this (see per-move notes).
- Camera: fixed, directly in front of the character's starting facing. The side sheets face right (+x), so +x is toward the camera.

## Per move (frontal = figure squarely faces camera; match = pose agrees with that side-sheet frame)

| move | frontal | pose match | notes |
|---|---|---|---|
| flying_side_kick | 8/8 | 5/8 | f5 and f6 (peaks) are correct: the orange kick goes straight at the camera, so the sole is a big oval. But the figure stands on the blue leg instead of being airborne, so take the height from the side sheet. f3 push-off is weak. f4 is airborne with the knee tucked (good). |
| jumping_knee | 8/8 | 7/8 | The raised knee in f3-f6 is drawn on the BLUE side (the generator ignored the rule), consistently across all four, so read it as knee_l with swapped colour. The airborne gap is small. f1 stance, f2 dip, f7 land and f8 crouch are good. |
| spinning_back_kick | 8/8 | 4/8 | The main file keeps the torso facing the camera throughout, so f3-f4 show no spin. f5 and f6 have the blue foot sole toward the camera, which is geometrically wrong for a back kick (the back should face the camera). The alt file shows f4-f6 back-on (orange on viewer's right, which is correct for a back view) and an f3 mid-turn, but its f1 and f8 have swapped colours. Use the alt for the f4-f6 lateral spread, and the main for f1, f2, f7 and f8. |
| roundhouse_kick | 8/8 | 7/8 | Good chamber (f3), extension at head height (f5, f6) and re-chamber (f7). The colour of the raised leg is muddled in f4 and f7, where the thigh shades blue. |
| backflip_evade | 8/8 | 7/8 | f1 stance, f2 crouch, f3-f4 arms up, f5 inverted end-on, f6 upright landing, f7-f8 crouch. f4 tucks less than the side sheet. f6 stands straighter than the side sheet. |
| palm_thrust_cast | 8/8 | 8/8 | Horse stance in f2-f4. f5 and f6 have the orange palm toward the camera (peak). f3 is nearly a repeat of f2, and the side sheet is similar. |
| axe_kick | 8/8 | 8/8 | f3 rising, f4 and f5 with the orange leg vertical above the head (peak), f6 the chop into a wide stance. One panel-edit pass fixed f5 (it first showed a raised arm). |
| aerial_dive_attack | 8/8 | 7/8 | The best result. f6 and f7 are the head-first dive at the camera, strongly foreshortened. f4 is inverted. f5 has the arms overhead. f8 is a crouch. f1 crouches more deeply than the side sheet. |
| crash_landing | 8/8 | 8/8 | f1 falling spread-eagle, f2 knees up, f3-f5 impact and compression with knee and fist down, f6-f8 rising to stance. |

All 72 panels are frontal, except where a move requires turning away: in the spinning back kick alt, f3-f6 are side- or back-on.
Pose mismatches are listed above. Those that matter most for T003 peaks are the flying side kick f5/f6 (not airborne) and the
spinning back kick f4-f6 (choose main or alt).
