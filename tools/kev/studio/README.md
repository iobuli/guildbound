# Kev Studio

Live rig viewer + Kev read/write console. Run locally (needs the repo's assets/anim):

    python3 -I tools/kev/studio/server.py      # then open http://localhost:8765

- Console (`help`): play/pause/seek/model, `get`/`set` bones, `landmarks`, `facings`, `balance`.
- `check NAME` reports no-glitch issues (limb stretch, hinges, floor, foot slide, palms, fingers) with frame ranges. The viewer posts bone positions with the rotations, so `/api/state` tracks pelvis translation the same way `/api/control` does.
- Hands: `hand l|r open|fist|point|relaxed|0..1`; `ref capture` also records every finger joint and `ref play` re-poses palm + fingers.
- Controller: `reach arm_r x y z` (IK to a world point), `aim <bone> x y z`.
- Kev references: play a clip, `ref capture <name>` records one loop of Kev's landmarks to staging/kev/refs;
  `ref play <name> [rate]` replays them onto whichever model is loaded (height-scaled), with tracking error live in the panel.
- Claude as controller: any client can `GET /api/state` (quats, landmarks, balance) and `POST /api/control`
  `{"landmarks": {"wrist_r": [x,y,z], ...}}` or `POST /api/exec {"line": "..."}`; the viewer mirrors every command.
