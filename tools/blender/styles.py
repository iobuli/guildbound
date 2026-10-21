"""Fighting styles from the stance sheet, each with a signature move animated in "superhero mode".

    blender -b --factory-startup --python tools/blender/styles.py -- [--render DIR] [--export GLB] [--only Style]

Every style has a stance and a few key poses (joint angles on the karate.py rig: planted feet on IK, lifted legs
and arms in angles). Superhero mode turns each key into: an anticipation (20 % the other way), the strike pushed
25 % past the human pose with a further 10 % overshoot that snaps back, a short hold so it reads, then recovery to
the stance. Strikes use the snap ease; tai chi gets slow, even timing instead. Actions: `Style_<name>`, plus
`Styles_All` chaining them for the showcase render.
"""
import sys
import os

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bpy  # noqa: E402
import karate as K  # noqa: E402


def feet(lx, ly, rx, ry, **kw):
    return {"l": {"ankle": (lx, ly, 0.104), "planted": True, "pole": (0.1, -1, 0), **kw.get("l", {})},
            "r": {"ankle": (rx, ry, 0.104), "planted": True, "pole": (-0.1, -1, 0), **kw.get("r", {})}}


def S(feet_, **kw):
    p = dict(feet_)
    p.update(kw)
    return p


def arms(l, r):
    return {"hl": l, "hr": r}


STYLES = {}

# Battle manga: deep crouch, claws low -> explosive dash punch.
st = S(feet(0.30, 0.08, -0.30, -0.02), pel=(0.0, 0.04, -0.24), tilt=(24, 0), chest=(10, 0, 0), head=(-22, 0, 0),
       **arms({"shoulder": (30, 45, 0), "elbow": 70, "fist": 50}, {"shoulder": (30, 45, 0), "elbow": 70, "fist": 50}))
STYLES["Battle_Manga"] = (st, [
    (S(feet(0.25, -0.30, -0.25, 0.45), pel=(0.0, -0.25, -0.20), tilt=(32, 0), chest=(10, 0, -25), head=(-30, 20, 0),
       **arms({"shoulder": (-30, 30, 0), "elbow": 110}, {"shoulder": (92, 8, 0), "elbow": 2})), "strike"),
], "snap")

# Boxing: high guard -> jab, cross, hook.
g = arms({"shoulder": (60, 12, 0), "elbow": 130}, {"shoulder": (55, 15, 0), "elbow": 140})
st = S(feet(0.12, -0.14, -0.14, 0.18), yaw=25, pel=(0.0, 0.0, -0.06), tilt=(8, 0), head=(10, -15, 0), **g)
STYLES["Boxing"] = (st, [
    (S(st, **arms({"shoulder": (88, 5, 0), "elbow": 5}, g["hr"])), "jab"),
    (S(st, chest=(0, 0, -35), pel=(0.0, -0.05, -0.07), **arms(g["hl"], {"shoulder": (88, 0, 0), "elbow": 5})), "cross"),
    (S(st, chest=(0, 0, 35), **arms({"shoulder": (85, 70, 0), "elbow": 90}, g["hr"])), "hook"),
], "snap")

# Muay thai: tall guard -> rising knee, then a downward elbow.
g = arms({"shoulder": (70, 25, 0), "elbow": 140}, {"shoulder": (70, 25, 0), "elbow": 140})
st = S(feet(0.10, -0.10, -0.12, 0.12), pel=(0.0, 0.0, -0.02), head=(12, 0, 0), **g)
STYLES["Muay_Thai"] = (st, [
    (S(st, pel=(0.06, -0.12, 0.0), tilt=(-12, 0), r={"thigh": (115, 0, 0), "knee": 125, "plant": 0.0, "ankle": (-0.12, 0.12, 0.104)},
       **arms({"shoulder": (70, 10, 0), "elbow": 100}, {"shoulder": (70, 10, 0), "elbow": 100})), "knee"),
    (S(st, chest=(15, 0, -40), **arms(g["hl"], {"shoulder": (100, 60, 0), "elbow": 150})), "elbow"),
], "snap")

# Karate: front stance, fist at the hip -> lunge punch with a locked finish.
st = S(feet(0.12, -0.38, -0.12, 0.34), pel=(0.0, -0.02, -0.12), head=(5, 0, 0),
       **arms({"shoulder": (35, 10, 0), "elbow": 10}, {"shoulder": (-30, 10, 0), "elbow": 110}))
STYLES["Karate"] = (st, [
    (S(st, pel=(0.0, -0.10, -0.14), chest=(0, 0, -12), **arms({"shoulder": (-30, 10, 0), "elbow": 110}, {"shoulder": (90, 0, 0), "elbow": 0})), "oi_zuki"),
], "snap")

# Kung fu: horse stance, crane hands -> crane lift into a snap kick.
st = S(feet(0.36, 0.0, -0.36, 0.0), pel=(0.0, 0.0, -0.25), head=(5, 0, 0),
       **arms({"shoulder": (80, 40, 0), "elbow": 40, "wrist": 60, "fist": 40}, {"shoulder": (20, 70, 0), "elbow": 30, "fist": 20}))
STYLES["Kung_Fu"] = (st, [
    (S(st, pel=(0.12, 0.0, -0.02), r={"thigh": (120, 10, 0), "knee": 140, "plant": 0.0, "ankle": (-0.36, 0.0, 0.104)},
       **arms({"shoulder": (40, 100, 0), "elbow": 10, "wrist": 60, "fist": 40}, {"shoulder": (40, 100, 0), "elbow": 10, "wrist": 60, "fist": 40})), "crane"),
    (S(st, pel=(0.12, 0.0, -0.02), tilt=(-8, 0), r={"thigh": (95, 0, 0), "knee": 5, "foot": 40, "plant": 0.0, "ankle": (-0.36, 0.0, 0.104)},
       **arms({"shoulder": (40, 100, 0), "elbow": 10, "wrist": 60, "fist": 40}, {"shoulder": (40, 100, 0), "elbow": 10, "wrist": 60, "fist": 40})), "kick"),
], "snap")

# Judo: gripping -> turn in, drop, throw.
grip = arms({"shoulder": (70, 10, 0), "elbow": 60, "fist": 70}, {"shoulder": (70, 10, 0), "elbow": 60, "fist": 70})
st = S(feet(0.15, -0.05, -0.15, 0.05), pel=(0.0, 0.0, -0.06), tilt=(15, 0), **grip)
STYLES["Judo"] = (st, [
    (S(feet(0.15, -0.05, -0.15, 0.05), yaw=170, pel=(0.0, 0.0, -0.26), tilt=(35, 0),
       **arms({"shoulder": (60, -10, 0), "elbow": 100, "fist": 70}, {"shoulder": (100, 30, 0), "elbow": 100, "fist": 70})), "load"),
    (S(feet(0.15, -0.05, -0.15, 0.05), yaw=170, pel=(0.0, 0.0, -0.04), tilt=(65, 0), head=(-20, 0, 0),
       **arms({"shoulder": (20, -10, 0), "elbow": 80, "fist": 70}, {"shoulder": (40, 30, 0), "elbow": 60, "fist": 70})), "throw"),
], "snap")

# Sumo: shiko squat -> leg raise, stamp, palm push.
st = S(feet(0.42, 0.0, -0.42, 0.0), pel=(0.0, 0.0, -0.36), tilt=(12, 0),
       **arms({"shoulder": (40, 30, 0), "elbow": 10, "fist": 10}, {"shoulder": (40, 30, 0), "elbow": 10, "fist": 10}))
STYLES["Sumo"] = (st, [
    (S(st, pel=(0.28, 0.0, -0.10), tilt=(5, 28), r={"thigh": (25, 85, 0), "knee": 10, "plant": 0.0, "ankle": (-0.42, 0.0, 0.104)},
       **arms({"shoulder": (20, 60, 0), "elbow": 10, "fist": 10}, {"shoulder": (20, 80, 0), "elbow": 10, "fist": 10})), "raise"),
    (S(st, pel=(0.0, 0.0, -0.42), tilt=(18, 0)), "stamp"),
    (S(feet(0.42, -0.25, -0.42, 0.20), pel=(0.0, -0.22, -0.30), tilt=(20, 0),
       **arms({"shoulder": (85, 10, 0), "elbow": 10, "wrist": -60, "fist": 0}, {"shoulder": (85, 10, 0), "elbow": 10, "wrist": -60, "fist": 0})), "push"),
], "snap")

# Tai chi: soft bow stance -> slow double-palm push.
soft = arms({"shoulder": (50, 20, 0), "elbow": 70, "wrist": -30, "fist": 0}, {"shoulder": (30, 20, 0), "elbow": 80, "wrist": -20, "fist": 0})
st = S(feet(0.15, -0.30, -0.15, 0.25), pel=(0.0, 0.05, -0.15), **soft)
STYLES["Tai_Chi"] = (st, [
    (S(st, pel=(0.0, -0.10, -0.17), **arms({"shoulder": (75, 15, 0), "elbow": 25, "wrist": -60, "fist": 0}, {"shoulder": (75, 15, 0), "elbow": 25, "wrist": -60, "fist": 0})), "push"),
    (S(st, pel=(0.0, 0.10, -0.15), chest=(0, 0, 20), **arms({"shoulder": (40, 50, 0), "elbow": 60, "wrist": -20, "fist": 0}, {"shoulder": (60, 10, 0), "elbow": 70, "wrist": -20, "fist": 0})), "circle"),
], "smooth")

# Drunken boxing: off-balance lean, cup hand -> sway, then a sudden low strike.
st = S(feet(0.06, -0.10, -0.02, 0.14), pel=(0.05, 0.05, -0.08), tilt=(-10, 15), chest=(0, 15, 10), head=(5, 0, -15),
       **arms({"shoulder": (70, 10, 0), "elbow": 140, "fist": 50}, {"shoulder": (10, 20, 0), "elbow": 40, "fist": 30}))
STYLES["Drunken_Boxing"] = (st, [
    (S(st, tilt=(-5, -20), chest=(0, -15, -15), head=(5, 0, 15)), "sway"),
    (S(feet(0.06, -0.40, -0.02, 0.20), pel=(0.0, -0.20, -0.25), tilt=(30, 0), chest=(0, 0, -20),
       **arms({"shoulder": (70, 10, 0), "elbow": 140, "fist": 50}, {"shoulder": (75, 0, 0), "elbow": 5, "fist": 60})), "strike"),
], "snap")

# Aikido: hanmi, open hands -> turning redirect.
st = S(feet(0.10, -0.25, -0.10, 0.20), pel=(0.0, 0.0, -0.06),
       **arms({"shoulder": (50, 5, 0), "elbow": 30, "fist": 0}, {"shoulder": (30, 5, 0), "elbow": 50, "fist": 0}))
STYLES["Aikido"] = (st, [
    (S(st, yaw=170, pel=(0.0, 0.0, -0.12), **arms({"shoulder": (60, 60, 0), "elbow": 20, "fist": 0}, {"shoulder": (70, 30, 0), "elbow": 20, "fist": 0})), "tenkan"),
    (S(st, yaw=170, pel=(0.0, 0.0, -0.20), tilt=(20, 0), **arms({"shoulder": (20, 30, 0), "elbow": 10, "fist": 0}, {"shoulder": (30, 20, 0), "elbow": 20, "fist": 0})), "descend"),
], "smooth")


def hero(style):
    """Superhero mode: anticipation, exaggeration with overshoot, a hold, recovery."""
    st, keys, kind = STYLES[style]
    stf = K.full(st)
    slow = kind == "smooth"
    seq = [(st, 0.30, "smooth", f"{style}:stance")]
    for pz, tag in keys:
        k = K.full(pz)
        antic = K.mix(stf, k, -0.20)
        big = K.mix(stf, k, 1.25)
        over = K.mix(stf, k, 1.35)
        if slow:
            seq += [(antic, 0.45, "smooth", f"{style}:antic"), (big, 0.9, "smooth", f"{style}:{tag}"), (big, 0.25, "linear", "hold")]
        else:
            seq += [(antic, 0.16, "smooth", f"{style}:antic"), (over, 0.11, "snap", f"{style}:{tag}"), (big, 0.06, "smooth", "settle"),
                    (big, 0.12, "linear", "hold")]
    seq.append((st, 0.40 if not slow else 0.8, "smooth", f"{style}:stance"))
    return seq


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opt = {argv[i]: argv[i + 1] for i in range(0, len(argv) - 1, 2)}
    names = [opt["--only"]] if "--only" in opt else list(STYLES)
    rig = K.setup()
    for n in names:
        K.SEQUENCES[f"Style_{n}"] = hero(n)
    acts = [K.build(rig, [f"Style_{n}"], f"Style_{n}")[0] for n in names]
    allact, keys = K.build(rig, [f"Style_{n}" for n in names], "Styles_All")
    for a in acts + [allact]:
        a.use_fake_user = True
    starts = []
    for n in names:
        f = next(fr for (_, tag, fr) in keys if tag.startswith(f"{n}:"))
        starts.append((n, f))
    print("STYLE STARTS", starts, "END", int(allact.frame_range[1]))
    sc = bpy.context.scene
    if "--render" in opt:
        out = opt["--render"].rstrip("/")
        K.camera(sc)
        sc.render.resolution_x, sc.render.resolution_y = 900, 900
        co = sc.camera
        co.data.type = "PERSP"
        co.data.lens = 45
        co.location = (3.6, -3.6, 1.5)
        K.look_at(co, (0, 0, 0.8))
        rig.arm.animation_data.action = allact
        sc.frame_start, sc.frame_end = 1, int(allact.frame_range[1])
        sc.frame_step = 2
        sc.render.filepath = f"{out}/s_"
        bpy.ops.render.render(animation=True)
    if "--export" in opt:
        rig.arm.animation_data.action = None
        for o in bpy.data.objects:
            o.select_set(o.type in ("ARMATURE", "MESH"))
        bpy.ops.export_scene.gltf(filepath=opt["--export"], export_format="GLB", use_selection=True, export_animation_mode="ACTIONS", export_force_sampling=True)


main()
