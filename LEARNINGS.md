# Learnings

- `Pose.pelvis` is in the pelvis's parent (root bone) frame, which is rotated: lift along model up via the parent rotation, not `.y` (CMU floor placement bug).
- `AlphaMode::Add` in Bevy is premultiplied: an additive shader must return alpha 0 or it paints black (rank aura capsule, fixed by agent-3).
- Ragdoll joints need local frames from the spawn pose; world anchors launch twisted bodies (agent-3, E7).
- A fresh AI `Brain` starts with a 1 s cooldown: tests must reset it.
- A full disk crashes the linker with "Bus error": delete recording frames after encoding.
- Bevy query conflicts only show at runtime: do a short headless launch after adding systems.
- Measure instead of eyeballing: fit checker (`GB_CHECK=1`), floor report, joint monitor, `[ragdoll] down` lines.
