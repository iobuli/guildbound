# gb_lab - headless Guildbound physics lab

Standalone (not part of the Guildbound repo). Bevy 0.19.1 (MinimalPlugins, no window/render), Avian 0.7.0, fixed 1/120 s, deterministic.

## Run (Windows or Linux)
    cargo run --release -- <scene> [--ticks N] [--kp X] [--kd Y] [--k Z] [--leg_floor F] [--unpaired] [--exact] [--out file.jsonl]

Scenes:
- `lie`   - rung 0 gate: motors off, body flat on the floor, 240 ticks; drift = slide.
- `stand` - T-pose capsule ragdoll, single torque writer per joint, 4800 ticks default.
            `--unpaired` applies +tau to child only (no -tau on parent). `--leg_floor` sets a minimum leg clamp (N*m, default 0).
- `sway`  - 3 props x 3 modes (1/3/8 Hz, zeta 0.05), hit at tick 60. Default semi-implicit Euler (phys_modal.rs), `--exact` = exact damped step. Runs twice and checks bit-identical q.
- `clearance` - sparse chunked voxel store (IVec3 keys, 16^3 chunks, 4^3 coarse cells), coarse-then-fine capsule query vs brute force, greedy box merge into Avian static colliders + capsule drop.

A one-line JSON summary goes to stdout; `--out` writes per-tick JSONL.
Defaults: kp 400, kd 40, k 1.2.
