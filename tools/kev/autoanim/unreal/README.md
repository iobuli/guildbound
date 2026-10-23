# Sparking! ZERO archive library in Kev

Open Kev Studio, use **Sparking! ZERO archive** at the top left, select a character folder,
search or choose a clip, then press **Load DBZ animation**. The existing play/pause and
seek controls work on that clip. Clear the search box to see every clip in the selected folder.

The current installed-game index contains **33,320 sequence candidates across 167 character folders**.
These are indexed paths, not 33,320 verified conversions. Individual clips are decoded and checked on
first use; special/non-humanoid rigs may need additional retarget rules. Character IDs and original
clip names are preserved so the origin is traceable.

The archive is read on demand. One resident CUE4Parse process mounts it once. Selected sequences are
ACL-decoded, reduced to the body/finger bones, and retargeted to the existing UAL mannequin. Animation-only
GLBs share that mannequin instead of duplicating its mesh. First load measured about 2 seconds; subsequent
short uncached attacks about 0.2 seconds; cached requests effectively instant on this machine.

No full batch export is required to browse the library. The original game files remain the source.

## Local data

- `staging/kev/sparking/config.json`: archive directory, property mappings, index, and published archive key.
- `staging/kev/sparking/archive-index.json`: indexed archive paths.
- `staging/kev/sparking/Mappings.usmap`: property mappings.
- `staging/kev/sparking/worker.log`: decoder diagnostics.
- `staging/kev/sparking/access.jsonl`, `failures.jsonl`: access/failure records.
- `assets/anim/sparking_cache/`: completed clip GLBs and source sidecars (git-ignored).

These paths hold machine-local configuration and generated data. Do not redistribute extracted motion
with the game unless you have the necessary rights.

## Run

```powershell
dotnet build tools/kev/autoanim/unreal/ArchiveProbe.csproj
python -I tools/kev/studio/server.py
```

Open `http://127.0.0.1:8765/?dbz=1`.

Requires .NET 10, Python with NumPy, the native CUE4Parse ACL decoder next to `ArchiveProbe.dll`,
and the local config above. `extract_native.py` extracts the decoder from an official FModel single-file
release without executing FModel.

`GET /api/sparking` returns the index. `POST /api/sparking/load {"id":"..."}` returns a playable
animation-only GLB URL, cache status, source path, timing and size. Calls serialize access to the native reader.
Failed loads are reported rather than silently substituted with another animation.

The game skeleton's anatomical basis is +X left, +Y forward, +Z up. The converter maps it to glTF +X left,
+Y up, +Z forward; assuming Unreal's usual +X forward caused the initial foot-orientation error.

## Character controller

The local `sparking` moveset uses the approved `sparking_rush_v2.glb` sequence in the game's
pose bank. Open the courtyard weapon chest with F, select `sparking`, then use primary attack
while standing, sprinting or airborne. Normal WASD, Shift, Space and dodge controls remain available.
Ground attacks consume source horizontal root travel through collision handling; no extra lunge is added.
Air attacks retain the controller's existing ballistic movement.
Every retargeted sample is retained, with neutral timing/exaggeration for these attacks.
Custom unarmed schools now retain their selected moves against armed opponents.

This is the approved rush integrated into three attack contexts, not a complete DBZ fighting style.
The hit window follows the IN/HT/OT boundaries; damage and hitbox radius are initial game tuning,
not values extracted from Sparking. More archive clips can be reviewed in Kev before adding moves.

## Validation

```powershell
python -m unittest discover -s tools/kev/autoanim/unreal -p "test_*.py" -v
```

`viewer_smoke.mjs` verifies a real Chrome session via localhost DevTools port 9224 and saves a screenshot
to `staging/kev/sparking/viewer.png`. Close stale viewer tabs when working with the shared rig.

Sources: [CUE4Parse](https://github.com/FabianFG/CUE4Parse), [FModel releases](https://github.com/4sval/FModel/releases),
[community mappings](https://github.com/TheNaeem/Unreal-Mappings-Archive/tree/main/DRAGON%20BALL%20Sparking%20Zero).
