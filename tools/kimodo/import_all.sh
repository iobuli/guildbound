#!/bin/bash
# Import every generated BVH onto the UAL skeleton and write one GLB the game loads like the other animation libraries.
#   tools/kimodo/import_all.sh [GENERATED_DIR=staging/kimodo] [OUT=assets/anim/kimodo.glb]
# Action names: Kimodo_<Animation>_<sample> (e.g. Kimodo_Jab_0). Needs Blender 4.2 (`blender` on the PATH).
set -euo pipefail
cd "$(dirname "$0")/../.."
dir=${1:-staging/kimodo}; out=${2:-assets/anim/kimodo.glb}
files=$(find "$dir" -name '*.bvh' | sort)
[ -n "$files" ] || { echo "no BVH under $dir"; exit 1; }
paths=$(echo "$files" | paste -sd, -)
names=$(echo "$files" | while read -r f; do b=$(basename "$f" .bvh); a=$(basename "$(dirname "$f")"); echo "${a}_${b##*_}"; done | paste -sd, -)
blender -b --factory-startup --python tools/kimodo/bvh_to_ual.py -- --bvh "$paths" --names "$names" --export "$out"
echo "wrote $out"
