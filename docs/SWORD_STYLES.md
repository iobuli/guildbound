# Trunks and Tapion sword schools

Z switches fists and the selected sword school. Shift+Z changes between Trunks and Tapion
while free. Rook can also equip either. `GB_STYLE=trunks_sword|tapion_sword|katana|sparking`
selects the starting style in the focused build. X draws and sheathes.

Both schools use their own locally extracted ground stance, guard and five complete attacks.
Three attacks form a light chain; the others supply the advancing cut and charged/falling
finisher. The legacy katana keeps its UAL chain. The previous mixed Trunks/Tapion/UAL chain
and parry-pose heavy attack have been removed from the legacy katana.

The exporter preserves the canonical skin joint list, every source sample, and the original
rotation timing. It removes accumulated horizontal arena offsets from the sword clips: later
Trunks stages started metres forward of the skeleton reference pose. Keeping those offsets
shifted the visible body away from the collider and into the camera. Controller steps now
supply the movement, with nearby-target distance limiting the extra lunge.

Contact windows are calculated from the source intro/contact/recovery durations. The generated
report is `assets/anim/sword_styles.report.json`. Clips use continuous sampling with no pose
exaggeration. Trimmed attacks retain their source recovery section for a clean return to stance.
Early follow-up taps remain buffered until the current light attack can chain.
Weapon afterimages use shared meshes/materials and the common accent cap; sword arms do not
receive the unarmed follow-through spring impulse.

The blade meshes are normalized versions of the existing Warrior Pack longsword and knight
sword. Their old FBX node offsets and scale have been removed; the blade points along the
same +Y axis used by the melee hitbox. Steel, guard and grip vertex colours add contrast without
new textures. Their skeleton-independent geometry is small and shared between wielders.

Regenerate the clips, school JSON, report and normalized blades with
`python tools/kev/autoanim/unreal/build_sword_styles.py` using the existing local archive config.
Kev Studio displays clip-only libraries on the canonical mannequin and attaches the matching
blade when either sword library is selected.

Focused rendered runs completed Cut 01 -> 02 -> 03 and the Cut 05 charged finisher for both
schools, then exited with code 0. Each library contains eight clips and 65 skin joints. The
export audit measured horizontal hip displacement below 0.000001 metres relative to the
canonical rest pose. Captures are `staging/kev/magic-polish/final-trunks_sword.png` and
`final-tapion_sword.png`. This checks source selection and visible/collider alignment; the
deferred gameplay pass still needs to assess all weapon contacts and environment clipping.
