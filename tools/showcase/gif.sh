#!/usr/bin/env bash
# Record a stretch of the game headless and turn it into a GIF (and an MP4) at full resolution.
#
#   tools/showcase/gif.sh <name> <from_s> <to_s> [WxH] [KEY=VALUE ...]
#
# Game time [from_s, to_s] is recorded at 30 fps by the game's own recorder (GB_RECORD: every frame advances
# game time by exactly 1/30 s, so the result plays at true speed however slowly the software renderer runs),
# then encoded with a palette made from the clip (palettegen / paletteuse, sierra dithering) at 15 fps.
# Extra KEY=VALUE pairs are passed to the game (GB_SPAWN, GB_FIGHT, GB_CLASH, GB_RIVAL, GB_ZOOM, ...).
# Output: staging/showcase/<name>.gif and <name>.mp4; the frames are deleted afterwards.
set -euo pipefail
name=${1:?name}; from=${2:?from}; to=${3:?to}; res=${4:-1920x1080}
shift 4 || shift $#
repo=$(cd "$(dirname "$0")/../.." && pwd)
out="$repo/staging/showcase"; frames="$out/frames_$name"
mkdir -p "$frames"
w=${res%x*}; h=${res#*x}
( cd "$repo" && env GB_RES="$res" GB_RECORD="$frames" GB_RECORD_FROM="$from" GB_RECORD_TO="$to" GB_NEW=1 "$@" \
    VK_ICD_FILENAMES=${VK_ICD_FILENAMES:-/usr/share/vulkan/icd.d/lvp_icd.json} \
    xvfb-run -a -s "-screen 0 ${w}x${h}x24" ./target/debug/guildbound > "$out/$name.log" 2>&1 )
ffmpeg -nostdin -hide_banner -loglevel error -y -framerate 30 -i "$frames/f_%05d.png" \
    -vf "fps=15,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a" "$out/$name.gif"
ffmpeg -nostdin -hide_banner -loglevel error -y -framerate 30 -i "$frames/f_%05d.png" -c:v libx264 -pix_fmt yuv420p -crf 20 "$out/$name.mp4"
find "$frames" -name 'f_*.png' -delete && rmdir "$frames"
ls -la "$out/$name.gif" "$out/$name.mp4"
