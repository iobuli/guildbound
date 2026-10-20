"""One pose in isolation, checked: Blender renders it (side / front / three-quarter), Kev's body map measures it
against the reference chart, and the side view is onion-skinned over the chart panel at the same body height.

    python3 -I tools/blender/posecheck.py Mae_Geri.chamber [--out staging/poses] [--ref tools/blender/refs/karate.json]

Writes `<out>/<pose>_check.png` (chart panel | onion skin | front | three-quarter) and prints Kev's report
(keypoint errors in heights, segment angle errors, facings, balance).
"""
import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CHART = os.environ.get("GB_REF_PICTURE", "/tmp/claude-0/-home-user/a063a533-b4bc-50c2-b57e-c63d6ea1e379/images/7.png")
BODYMAP = os.path.join(REPO, "staging/kev/bodymap/ual2.json")
ZOOM = 4


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw).stdout


def main():
    a = sys.argv[1:]
    name = a[0]
    out = a[a.index("--out") + 1] if "--out" in a else os.path.join(REPO, "staging/poses")
    refp = a[a.index("--ref") + 1] if "--ref" in a else os.path.join(REPO, "tools/blender/refs/karate.json")
    os.makedirs(out, exist_ok=True)
    tag = name.replace(".", "_")
    if not os.path.exists(BODYMAP):
        run(["python3", "-I", os.path.join(REPO, "tools/kev/bodymap.py"), "build", os.path.join(REPO, "assets/anim/ual2.glb")], cwd=REPO)
    run(["blender", "-b", "--factory-startup", "--python", os.path.join(REPO, "tools/blender/karate.py"), "--", "--pose", name, "--views", out])
    set_name, _, phase = name.partition(".")
    kev = [os.path.join(REPO, "tools/kev/bodymap.py"), "query", BODYMAP, f"{out}/{tag}_pose.json", "--ref", refp, "--set", set_name, "--phase", phase]
    report = run(["python3", "-I", *kev])
    data = json.loads(run(["python3", "-I", *kev, "--json"]))
    with open(refp) as f:
        ref = json.load(f)
    with open(BODYMAP) as f:
        H = json.load(f)["body"]["height"]
    with open(f"{out}/{tag}_view.json") as f:
        view = json.load(f)
    k = ref["sets"][set_name]["phases"][phase]
    x0, y0, w, h = k["crop"]
    anchor = ref["sets"][set_name]["anchor"]
    lm = ref["sets"][set_name]["landmarks"][anchor]
    ax, ay = k["points"][anchor]
    g = data["landmarks"][lm]
    px = view["centre"][0] + (-g[2]) * view["ppm"]
    py = view["centre"][1] - (g[1] - view["view_z"]) * view["ppm"]
    hpx = k.get("height_px", ref["sets"][set_name].get("height_px", k["floor"] - k["top"]))
    s = hpx / (H * view["ppm"]) * ZOOM
    ox, oy = (ax - x0) * ZOOM - px * s, (ay - y0) * ZOOM - py * s
    W, Hh = view_size = (int(700 * s), int(820 * s))
    sheet = f"{out}/{tag}_check.png"
    fc = (f"[0]crop={w}:{h}:{x0}:{y0},scale={w * ZOOM}:{h * ZOOM}:flags=lanczos,split[c1][c2];"
          f"[1]scale={W}:{Hh},colorchannelmixer=rr=0.2:gg=0.45:bb=1.0:aa=0.55[o];"
          f"[c2][o]overlay=x={ox:.1f}:y={oy:.1f}[on];"
          f"[2]scale=-1:{h * ZOOM}[fr];[3]scale=-1:{h * ZOOM}[tq];[c1][on][fr][tq]hstack=inputs=4")
    subprocess.run(["ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-i", CHART, "-i", f"{out}/{tag}_side.png", "-i", f"{out}/{tag}_front.png",
                    "-i", f"{out}/{tag}_34.png", "-filter_complex", fc, sheet], check=True)
    print(report)
    print("sheet:", sheet)


main()
