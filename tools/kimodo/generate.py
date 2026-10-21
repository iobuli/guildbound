"""Generate every brawler-mage animation with NVIDIA Kimodo (run this on a GPU machine with Kimodo installed).

    python3 tools/kimodo/generate.py [--only Jab,Cross] [--samples 3] [--model Kimodo-SOMA-RP-v1.2] [--out staging/kimodo] [--seed 42]

One `kimodo_gen` call per entry of prompts.json: the prompt, its duration, N samples, SOMA BVH with the standard
T-pose rest (what bvh_to_ual.py expects). Files land in `<out>/<Name>/`. Re-running skips animations whose BVH exist.
Needs: `pip install git+https://github.com/nv-tlabs/kimodo.git`, a Hugging Face token whose account has access to the
gated meta-llama/Meta-Llama-3-8B-Instruct (the text encoder), and ~17 GB of GPU memory, or set TEXT_ENCODER_DEVICE=cpu
to keep the encoder on the CPU (then <3 GB of VRAM).
"""
import argparse
import glob
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", default="")
    ap.add_argument("--samples", type=int, default=3)
    ap.add_argument("--model", default="Kimodo-SOMA-RP-v1.2")
    ap.add_argument("--out", default="staging/kimodo")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--steps", type=int, default=100)
    a = ap.parse_args()
    with open(os.path.join(HERE, "prompts.json")) as f:
        prompts = json.load(f)
    names = [n for n in a.only.split(",") if n] or list(prompts)
    for n in names:
        d = os.path.join(a.out, n)
        if glob.glob(os.path.join(d, "*.bvh")):
            print(f"skip {n} (already generated)")
            continue
        os.makedirs(d, exist_ok=True)
        cmd = ["kimodo_gen", prompts[n]["prompt"], "--model", a.model, "--duration", str(prompts[n]["duration"]),
               "--num_samples", str(a.samples), "--seed", str(a.seed), "--diffusion_steps", str(a.steps),
               "--bvh", "--bvh_standard_tpose", "--output", os.path.join(d, n)]
        print(" ".join(repr(c) if " " in c else c for c in cmd), flush=True)
        r = subprocess.run(cmd)
        if r.returncode:
            print(f"FAILED {n} ({r.returncode})", file=sys.stderr)
    print("done:", sum(len(glob.glob(os.path.join(a.out, n, '*.bvh'))) for n in names), "BVH files")


main()
