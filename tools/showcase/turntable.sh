#!/usr/bin/env bash
# Spin each model in the viewer's turntable and record the screen to an MP4 and a GIF.
#
#   tools/showcase/turntable.sh <name> <secs_per_model> <WxH> <glb> [<glb> ...]   (glb paths under assets/)
#
# The viewer shows each model scaled to fit and spinning (GB_VIEWER_TURNTABLE); ffmpeg grabs the X display
# from the moment the first model appears. Output: staging/showcase/<name>.mp4 and <name>.gif.
set -euo pipefail
name=${1:?name}; secs=${2:?secs}; res=${3:?WxH}; shift 3
repo=$(cd "$(dirname "$0")/../.." && pwd)
out="$repo/staging/showcase"; mkdir -p "$out"
list=$(IFS=,; echo "$*")
total=$(python3 -c "print(int($# * ($secs + 1.5) + 2))")
w=${res%x*}; h=${res#*x}
cat > "$out/.turn_$name.sh" <<INNER
cd "$repo"
GB_RES=$res GB_VIEWER_TURNTABLE="$list" GB_VIEWER_SPIN_SECS=$secs VK_ICD_FILENAMES=\${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/lvp_icd.json} ./target/debug/gb_viewer > "$out/$name.log" 2>&1 &
P=\$!
until grep -q "\[turntable\] fitted" "$out/$name.log" 2>/dev/null || [ \$SECONDS -gt 120 ]; do sleep 0.5; done
ffmpeg -nostdin -hide_banner -loglevel error -y -f x11grab -draw_mouse 0 -framerate 15 -video_size ${w}x${h} -i \$DISPLAY -t $total -c:v libx264 -pix_fmt yuv420p -crf 20 "$out/$name.mp4"
kill \$P; wait \$P 2>/dev/null || true
INNER
xvfb-run -a -s "-screen 0 ${w}x${h}x24" bash "$out/.turn_$name.sh"
rm -f "$out/.turn_$name.sh"
ffmpeg -nostdin -hide_banner -loglevel error -y -i "$out/$name.mp4" -vf "fps=10,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a" "$out/$name.gif"
ls -la "$out/$name.mp4" "$out/$name.gif"
