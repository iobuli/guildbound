"""Zoomed crop of a reference picture with a labelled pixel grid, for reading keypoints off it.

    python3 -I tools/blender/refgrid.py <picture> <x0> <y0> <w> <h> <out.png> [--zoom 4] [--step 10]

Grid lines every `step` source pixels (labelled every 50, in SOURCE pixel coordinates), so a point read off the
zoomed image is written down directly in the picture's own pixels.
"""
import subprocess
import sys

FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"


def main():
    a = sys.argv[1:]
    pic, x0, y0, w, h, out = a[0], *map(int, a[1:5]), a[5]
    zoom = int(a[a.index("--zoom") + 1]) if "--zoom" in a else 4
    step = int(a[a.index("--step") + 1]) if "--step" in a else 10
    f = [f"crop={w}:{h}:{x0}:{y0}", f"scale={w * zoom}:{h * zoom}:flags=lanczos"]
    for x in range(x0 - x0 % step, x0 + w + 1, step):
        major = x % 50 == 0
        f.append(f"drawbox=x={(x - x0) * zoom}:y=0:w=1:h={h * zoom}:color={'red@0.6' if major else 'gray@0.3'}:t=fill")
        if major:
            f.append(f"drawtext=fontfile={FONT}:text={x}:x={(x - x0) * zoom + 2}:y=2:fontsize=14:fontcolor=red")
    for y in range(y0 - y0 % step, y0 + h + 1, step):
        major = y % 50 == 0
        f.append(f"drawbox=x=0:y={(y - y0) * zoom}:w={w * zoom}:h=1:color={'red@0.6' if major else 'gray@0.3'}:t=fill")
        if major:
            f.append(f"drawtext=fontfile={FONT}:text={y}:x=2:y={(y - y0) * zoom + 2}:fontsize=14:fontcolor=red")
    subprocess.run(["ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-i", pic, "-vf", ",".join(f), out], check=True)


main()
