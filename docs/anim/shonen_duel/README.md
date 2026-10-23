# Shonen sword-vs-fist choreography

Original 60 fps animation set authored on the UAL2 mannequin. The full exchange is two
synchronized 2.6-second tracks:

- `Brawler_Shonen_Fist_Duel`
- `Brawler_Shonen_Sword_Duel`

The shared beats cover an opening dash, slash guard, fist counter, sword parry, pommel hit,
low-cut shin check, elbow response, reversal, perfect slip, body cross, sword counter, final
mutual clash, airborne disengage, and recovery. `contacts.json` is the frame/time contract.
Each beat also has a `*_side.png` and `*_front.png` proof render.

Four 0.82-second gameplay cuts and one self-authored bow sequence are included in
`assets/anim/shonen_duel.glb`. The combat cuts are wired into the unarmed/katana movesets;
the bow sequence replaces the former generic mocap dependency:

- `Brawler_Shonen_Flash_Cross`
- `Brawler_Shonen_Perfect_Counter`
- `Brawler_Shonen_Flash_Cut`
- `Brawler_Shonen_Perfect_Parry`
- `Brawler_Shonen_Bow_Shot`

Regenerate the proof with:

```powershell
$env:GB_POSE_MODEL='D:\gb_overhaul\assets\anim\ual2.glb'
& 'D:\blender.exe' -b --factory-startup --python tools/blender/duel_preview.py -- docs/anim/shonen_duel
```
