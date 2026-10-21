# Kimodo for the brawler-mage animation sheets

NVIDIA Kimodo ([github.com/nv-tlabs/kimodo](https://github.com/nv-tlabs/kimodo)) is a text-to-motion diffusion model
(282 M parameters, 700 h of mocap, SOMA skeleton). `prompts.json` holds a prompt and duration for each of the 65
animations on the five sprite sheets (names match `tools/blender/brawler.py`, the hand-authored library).

## Run (GPU machine)
1. `pip install git+https://github.com/nv-tlabs/kimodo.git`
2. Request access to `meta-llama/Meta-Llama-3-8B-Instruct` on Hugging Face (Kimodo's text encoder), then `hf auth login`.
3. `python3 tools/kimodo/generate.py --samples 3` (about 65 x 3 clips; `--only Jab,Cross` for a few). With less than
   17 GB of VRAM: `TEXT_ENCODER_DEVICE=cpu python3 tools/kimodo/generate.py ...`.
4. `tools/kimodo/import_all.sh` retargets every BVH to the UAL skeleton (Blender 4.2) and writes `assets/anim/kimodo.glb`.

## How the import works (`bvh_to_ual.py`, tested)
Kimodo writes SOMA BVH (77 joints, cm, Y-up, +Z forward, Z-Y-X rotations) with `--bvh --bvh_standard_tpose`, whose rest
pose is a T-pose like ours. Each joint's world rotation is taken by forward kinematics and applied as a delta to the
matching UAL bone's own rest (so bone roll differences cancel); hips height is scaled to ours and horizontal travel
is dropped (clips are in place). Checked with a synthetic BVH on the real SOMA hierarchy: left arm up, left knee
back, thigh forward, head turn all land on the right side and direction.

## Not possible in the cloud sandbox this was built in
No GPU, 15 GB RAM, and the Llama-3-8B encoder needs ~16 GB, so nothing was generated there.
