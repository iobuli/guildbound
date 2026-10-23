"""Brawler-mage animation library from the five sprite sheets (casting, brawler, evasive, berserk, hit reactions).

    blender -b --factory-startup --python tools/blender/brawler.py -- [--keys DIR] [--render DIR] [--export GLB] [--only name,...]

Animations are in place (the controller moves the body): feet step relative to the pelvis. Each is a list of key
poses on the karate.py rig (planted feet on IK, lifted legs and arms in joint angles, free-body poses for jumps
and falls), with a time to reach each key and an ease (`snap` for strikes). `--keys DIR` renders one picture per
key (for checking against the sheets), `--render DIR` the chained motion, `--export GLB` the actions as
`Brawler_<name>`.
"""
import os
import sys
import math

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bpy  # noqa: E402
import karate as K  # noqa: E402

# --- vocabulary -------------------------------------------------------------------------------------------------


def arm(sh=(0, 0, 0), el=0, wr=0, fist=80):
    return {"shoulder": sh, "elbow": el, "wrist": wr, "fist": fist}


def fk(th=(0, 0, 0), kn=0, ft=0, toe=0):
    return {"thigh": th, "knee": kn, "foot": ft, "toe": toe, "plant": 0.0, "planted": False}


def pl(x, y, **k):
    # Stances are written in a narrow scale; fighters stand wider (x 1.3, y 1.5).
    return {"ankle": (x * 1.3, y * 1.5, 0.104), "planted": True, "pole": (0.1 if x > 0 else -0.1, -1, 0), **k}


G_L = arm((60, 12, 0), 130)
G_R = arm((55, 15, 0), 140)
ORTHO = {"yaw": 25, "pel": (0, 0, -0.06), "tilt": (8, 0), "chest": (0, 0, 0), "head": (10, -15, 0),
         "l": pl(0.12, -0.14), "r": pl(-0.14, 0.18), "hl": G_L, "hr": G_R}
MAGE = {"yaw": 20, "pel": (0, 0, -0.16), "tilt": (12, 0), "chest": (0, 0, 0), "head": (8, -10, 0),
        "l": pl(0.24, -0.20), "r": pl(-0.24, 0.22), "hl": arm((70, 20, 0), 70, 0, 10), "hr": arm((70, 20, 0), 70, 0, 10)}
BERSERK = {"yaw": 15, "pel": (0, -0.02, -0.26), "tilt": (28, 0), "chest": (12, 0, 0), "head": (22, -8, 0),
           "l": pl(0.34, -0.22), "r": pl(-0.34, 0.20), "hl": arm((40, 30, 0), 60, 0, 40), "hr": arm((35, 30, 0), 65, 0, 40)}


def P(base=None, **kw):
    d = dict(base or ORTHO)
    d.update(kw)
    return d


def kar(name, phase, guard=True):
    """A karate-sheet pose with the brawler guard instead of the karate hands."""
    d = dict(K.POSES[name][phase])
    if guard:
        d["hl"], d["hr"] = G_L, G_R
    return d


ANIMS = {}  # name -> (group, [(pose, dt, ease, tag)])
GROUPS = ["Brawler", "Mage", "Evasive", "Berserk", "Hit"]


def crouch(p):
    """Fighters stand in a crouch: drop the pelvis 10 cm when both feet are planted, 4 cm with one lifted."""
    both = p["l"].get("planted") and p["r"].get("planted")
    one = p["l"].get("planted") or p["r"].get("planted")
    drop = 0.10 if both else (0.04 if one else 0.0)
    q = dict(p)
    x, y, z = p.get("pel", (0, 0, 0))
    q["pel"] = (x, y, z - drop)
    return q


def A(group, name, poses, dt=0.12, ease="smooth", fast=(), slow=()):
    out = []
    for i, p in enumerate(map(crouch, poses)):
        e = "snap" if i in fast else ease
        d = 0.07 if i in fast else (0.22 if i in slow else dt)
        out.append((p, 0.15 if i == 0 else d, e, f"{name}:{i + 1}"))
    ANIMS[name] = (group, out)


def fl(**kw):
    """Free-body base: both legs FK, pelvis free."""
    d = {"l": fk(), "r": fk(), "hl": arm(), "hr": arm(), "yaw": 25}
    d.update(kw)
    return d


# --- Sheet 2: brawler -------------------------------------------------------------------------------------------
A("Brawler", "Idle", [ORTHO, P(pel=(0, .01, -.08), tilt=(10, 0), hl=arm((62, 12, 0), 132), hr=arm((57, 15, 0), 142)),
                      P(pel=(0, .02, -.095), tilt=(11, 0), hl=arm((64, 12, 0), 135), hr=arm((58, 15, 0), 144)),
                      P(pel=(0, .01, -.08), tilt=(10, 0)), ORTHO, P(pel=(.01, 0, -.05), tilt=(7, 0), chest=(0, 0, 4))], dt=.18)
A("Brawler", "Step_In", [ORTHO, P(l=pl(.12, -.30), pel=(0, -.02, -.10), tilt=(12, 0)),
                         P(l=pl(.12, -.34), r=pl(-.14, .02), pel=(0, -.08, -.10), tilt=(12, 0)), ORTHO], dt=.14)
A("Brawler", "Jab", [ORTHO, P(pel=(0, .03, -.07), tilt=(10, 0), hl=arm((50, 12, 0), 150)),
                     P(pel=(0, -.05, -.08), tilt=(12, 0), chest=(0, 0, -12), hl=arm((90, 3, 0), 3), l=pl(.12, -.22)),
                     P(hl=arm((70, 10, 0), 70)), ORTHO], fast=(2,))
A("Brawler", "Cross", [ORTHO, P(pel=(0, .03, -.08), chest=(0, 0, 8), hr=arm((40, 15, 0), 140)),
                       P(chest=(0, 0, -35), pel=(.01, -.07, -.09), tilt=(12, 0), hr=arm((90, 0, 0), 3)),
                       P(chest=(0, 0, -15), hr=arm((70, 10, 0), 80)), ORTHO], fast=(2,))
A("Brawler", "Hook", [ORTHO, P(chest=(0, 0, -14), hl=arm((70, 60, 0), 120), pel=(0, .02, -.08)),
                      P(chest=(0, 0, 40), pel=(.02, -.03, -.09), hl=arm((82, 75, 0), 95)),
                      P(chest=(0, 0, 15), hl=arm((70, 50, 0), 110)), ORTHO], fast=(2,))
A("Brawler", "Elbow_Strike", [ORTHO, P(chest=(8, 0, 10), hr=arm((60, 50, 0), 150)),
                              P(chest=(10, 0, -38), pel=(0, -.04, -.10), hr=arm((95, 65, 0), 150)),
                              P(chest=(5, 0, -15), hr=arm((80, 40, 0), 130)), ORTHO], fast=(2,))
A("Brawler", "Knee_Strike", [ORTHO, P(pel=(0, .02, -.12), tilt=(15, 0)),
                             P(r=fk((118, 0, 0), 125, -10, -20), pel=(.06, -.10, 0), tilt=(-6, 0),
                               hl=arm((45, 15, 0), 110), hr=arm((45, 15, 0), 110)),
                             P(r=fk((100, 0, 0), 100, -10, -10), pel=(.06, -.06, -.02), tilt=(-2, 0)), ORTHO], fast=(2,))
A("Brawler", "Front_Kick", [ORTHO, kar("Mae_Geri", "chamber"), kar("Mae_Geri", "kick"), kar("Mae_Geri", "chamber"), ORTHO],
  fast=(2,), slow=(1,))
A("Brawler", "Roundhouse_Kick", [ORTHO, P(r=fk((95, 30, -25), 120, -10), pel=(.06, 0, -.02), tilt=(0, 8), chest=(0, 6, 0), l=pl(.12, -.10)),
                                 P(r=fk((88, 62, -30), 12, -10), pel=(.09, -.02, -.02), tilt=(0, 28), chest=(0, 10, -20),
                                   hl=arm((60, 40, 0), 90), hr=arm((40, 50, 0), 100), l=pl(.12, -.10)),
                                 P(r=fk((95, 30, -25), 120, -10), pel=(.06, 0, -.02), tilt=(0, 8), l=pl(.12, -.10)), ORTHO],
  fast=(2,), slow=(1,))
A("Brawler", "Spinning_Kick", [ORTHO, P(yaw=130, chest=(0, 0, 20), head=(10, 60, 0), pel=(0, 0, -.10), r=pl(-.14, .10)),
                               P(yaw=200, pel=(.04, 0, -.06), r=fk((100, 0, 0), 130, -10)),
                               P(yaw=270, pel=(.09, 0, -.03), tilt=(0, 26), chest=(0, 8, 0), r=fk((88, 65, -20), 8, -10),
                                 hl=arm((30, 60, 0), 40), hr=arm((20, 70, 0), 30)),
                               P(yaw=330, pel=(0, 0, -.10), r=pl(-.14, .10)), P(ORTHO, yaw=385)], fast=(3,))
A("Brawler", "Rising_Uppercut", [ORTHO, P(pel=(0, .02, -.30), tilt=(25, 0), hr=arm((-10, 15, 0), 80)),
                                 P(pel=(0, -.02, 0), tilt=(0, 0), hr=arm((150, 15, 0), 40), chest=(-6, 0, -20)),
                                 P(pel=(0, -.03, .06), tilt=(-4, 0), hr=arm((172, 10, 0), 20), chest=(-12, 0, -25),
                                   l=pl(.12, -.14, plant=.35), r=pl(-.14, .18, plant=.35)),
                                 ORTHO], fast=(2,))
JUMP = fl(pel=(0, -.1, .35), l=fk((70, 0, 0), 100), r=fk((70, 0, 0), 100), tilt=(5, 0),
          hl=arm((60, 30, 0), 80), hr=arm((60, 30, 0), 80))
A("Brawler", "Jump_Kick", [ORTHO, P(pel=(0, .04, -.32), tilt=(20, 0)), JUMP,
                           fl(pel=(0, -.35, .55), tilt=(-25, 0), l=fk((45, 0, 0), 95), r=fk((85, 0, 0), 8, -10),
                              hl=arm((30, 60, 0), 30), hr=arm((20, 70, 0), 30)),
                           fl(pel=(0, -.40, .30), tilt=(-10, 0), l=fk((50, 0, 0), 60), r=fk((60, 0, 0), 40),
                              hl=arm((40, 40, 0), 60), hr=arm((40, 40, 0), 60)),
                           P(pel=(0, -.1, -.28), tilt=(22, 0)), ORTHO], fast=(3,))
A("Brawler", "Aerial_Knee", [P(pel=(0, 0, -.30), tilt=(20, 0)), fl(pel=(0, -.1, .40), l=fk((60, 0, 0), 110), r=fk((60, 0, 0), 110)),
                             fl(pel=(0, -.2, .55), tilt=(-6, 0), r=fk((125, 0, 0), 110), l=fk((25, 0, 0), 40),
                                hl=arm((50, 20, 0), 110), hr=arm((40, 30, 0), 120)),
                             fl(pel=(0, -.2, .25), tilt=(10, 0), l=fk((50, 0, 0), 70), r=fk((70, 0, 0), 90)),
                             P(pel=(0, 0, -.30), tilt=(24, 0))], fast=(2,))
A("Brawler", "Aerial_Spin", [P(pel=(0, 0, -.3), tilt=(20, 0)), fl(pel=(0, 0, .40), yaw=60, l=fk((80, 0, 0), 110), r=fk((80, 0, 0), 110)),
                             fl(pel=(0, 0, .55), yaw=140, l=fk((90, 0, 0), 120), r=fk((90, 0, 0), 120), tilt=(10, 0)),
                             fl(pel=(0, 0, .60), yaw=210, l=fk((70, 0, 0), 100), r=fk((90, 0, 0), 30),
                                hl=arm((40, 70, 0), 30), hr=arm((40, 70, 0), 30)),
                             fl(pel=(0, 0, .60), yaw=280, tilt=(-12, 0), r=fk((85, 0, 0), 8, -10), l=fk((50, 0, 0), 80),
                                hl=arm((30, 80, 0), 20), hr=arm((20, 80, 0), 20)),
                             fl(pel=(0, 0, .55), yaw=340, l=fk((90, 0, 0), 110), r=fk((90, 0, 0), 110)),
                             fl(pel=(0, 0, .35), yaw=400, l=fk((50, 0, 0), 70), r=fk((50, 0, 0), 70)),
                             P(pel=(0, 0, -.32), tilt=(22, 0), yaw=385)], dt=.1)
A("Brawler", "Axe_Kick", [P(pel=(0, 0, -.28), tilt=(20, 0)), fl(pel=(0, -.1, .45), l=fk((60, 0, 0), 100), r=fk((60, 0, 0), 100)),
                          fl(pel=(0, -.1, .62), tilt=(-10, 0), r=fk((170, 0, 0), 4), l=fk((45, 0, 0), 90),
                             hl=arm((70, 30, 0), 80), hr=arm((70, 30, 0), 80)),
                          fl(pel=(0, -.15, .38), tilt=(14, 0), r=fk((55, 0, 0), 4), l=fk((50, 0, 0), 80)),
                          P(pel=(0, -.1, -.34), tilt=(26, 0)), P(pel=(0, -.05, -.20), tilt=(18, 0)), ORTHO], fast=(3,))
A("Brawler", "Landing_Recovery", [P(pel=(0, 0, -.38), tilt=(30, 0)), P(pel=(0, 0, -.28), tilt=(24, 0)),
                                  P(pel=(0, 0, -.16), tilt=(16, 0)), ORTHO], dt=.14)

# --- Original choreography: battle-shonen sword vs fist --------------------------------------------------------
#
# These two tracks share the exact same key times and tags.  They are a complete exchange rather than two
# disconnected attacks: every contact has an answer pose on the other actor.  ``stage`` is world-space travel,
# supported by karate.origin_for; it lets the paired preview preserve spacing while the four compact clips below
# remain in-place for ordinary gameplay locomotion/travel.


def timed(group, name, rows):
    """Register explicitly timed choreography rows: (pose, seconds, ease, semantic tag)."""
    ANIMS[name] = (group, [(crouch(p), dt, ease, tag) for p, dt, ease, tag in rows])


def sf(y, **kw):
    """Fist fighter, facing -Y from the positive side of the stage."""
    # Once the opening dash closes the gap, keep the grounded exchange on one contact line.
    # Apparent advance/retreat then comes from hips and torso until the airborne disengage;
    # planted feet no longer skate between nearby stage offsets.
    if .30 <= abs(y) <= .60:
        y = math.copysign(.47, y)
    return P(ORTHO, stage=(.12 if abs(y) > .1 else 0.0, y, 0), **kw)


SWORD_GUARD_L = arm((78, 22, -12), 105, 0, 92)
SWORD_GUARD_R = arm((70, 18, 12), 92, 0, 92)
SWORD = {"yaw": 180, "pel": (0, 0, -.08), "tilt": (10, 0), "chest": (0, 0, 8), "head": (8, -8, 0),
         "l": pl(.16, -.18), "r": pl(-.18, .20), "hl": SWORD_GUARD_L, "hr": SWORD_GUARD_R}


def sw(y, **kw):
    """Sword fighter, facing +Y from the negative side of the stage."""
    if .30 <= abs(y) <= .60:
        y = math.copysign(.47, y)
    return P(SWORD, stage=(-.12 if abs(y) > .1 else 0.0, y, 0), **kw)


# Tags are intentionally duplicated across both tracks.  The paired renderer and verifier use them as the
# contract: attack/block, counter/parry, hit/reaction and the final clash must land on the same frame.
DUEL_DT = (.14, .20, .12, .08, .13, .09, .08, .12, .12, .08, .13, .09, .10, .15, .10, .08, .14, .10, .18, .24)
FIST_DUEL = [
    sf(.96),
    sf(.76, pel=(.02, -.04, -.18), tilt=(20, 4), l=fk((38, 10, -5), 58, -10), r=fk((72, 8, 5), 98, -16), hl=arm((54, 18, 0), 125), hr=arm((50, 18, 0), 138)),
    sf(.47, pel=(.06, -.08, -.16), tilt=(18, 14), chest=(4, 8, -12), hl=arm((112, 55, -25), 118), hr=arm((62, 18, 0), 135)),
    sf(.47, pel=(.08, -.09, -.18), tilt=(20, 17), chest=(6, 12, -16), hl=arm((120, 62, -30), 125), hr=arm((65, 20, 0), 138)),
    sf(.40, pel=(-.02, -.10, -.14), tilt=(14, -8), chest=(-2, -8, 28), hl=arm((85, 20, 0), 75), hr=arm((42, 35, 0), 120)),
    sf(.38, pel=(-.04, -.12, -.15), tilt=(16, -10), chest=(0, -10, 34), hl=arm((92, 12, 0), 18), hr=arm((55, 38, 0), 125)),
    sf(.43, pel=(.03, .02, -.08), tilt=(-4, 8), chest=(-10, 8, -18), head=(-18, -5, 10), hl=arm((45, 55, 0), 80), hr=arm((70, 25, 0), 115)),
    sf(.58, pel=(.07, .08, -.10), tilt=(-12, 12), chest=(-16, 10, -24), head=(-26, 0, 14), l=pl(.20, -.06), r=pl(-.16, .30), hl=arm((35, 65, 0), 62), hr=arm((48, 50, 0), 92)),
    sf(.52, pel=(-.02, -.02, -.20), tilt=(26, -8), chest=(12, -8, 18), r=fk((76, 18, -10), 118, -12), hl=arm((72, 26, 0), 110), hr=arm((62, 24, 0), 120)),
    sf(.50, pel=(-.04, -.04, -.24), tilt=(30, -12), chest=(14, -12, 22), r=fk((104, 25, -16), 132, -18), hl=arm((78, 30, 0), 118), hr=arm((65, 30, 0), 125)),
    sf(.38, pel=(.04, -.11, -.14), tilt=(16, 8), chest=(4, 8, -30), hl=arm((55, 28, 0), 105), hr=arm((98, 68, 12), 128)),
    sf(.37, pel=(.06, -.12, -.14), tilt=(16, 12), chest=(2, 12, -36), hl=arm((58, 30, 0), 108), hr=arm((105, 72, 15), 132)),
    sf(.43, pel=(-.08, .02, -.12), tilt=(8, -18), chest=(-5, -18, 22), head=(2, -20, -12), hl=arm((72, 18, 0), 95), hr=arm((58, 25, 0), 115)),
    sf(.51, pel=(-.14, .10, -.12), tilt=(4, -26), chest=(-8, -22, 30), head=(-4, -25, -18), l=pl(.30, -.08), r=pl(-.10, .34), hl=arm((82, 15, 0), 80), hr=arm((65, 20, 0), 105)),
    sf(.37, pel=(.02, -.12, -.15), tilt=(18, 4), chest=(4, 4, -34), l=pl(.18, -.34), hl=arm((52, 22, 0), 112), hr=arm((92, 4, 0), 8)),
    sf(.36, pel=(.03, -.13, -.16), tilt=(19, 6), chest=(5, 6, -38), l=pl(.18, -.36), hl=arm((55, 24, 0), 115), hr=arm((96, 2, 0), 4)),
    sf(.43, pel=(-.02, -.02, -.22), tilt=(27, -6), chest=(10, -6, 20), hl=arm((102, 48, -12), 120), hr=arm((74, 24, 0), 105)),
    sf(.42, pel=(0, -.05, -.20), tilt=(24, 0), chest=(8, 0, 0), hl=arm((112, 58, -18), 130), hr=arm((108, 52, 16), 125)),
    sf(.66, pel=(0, .02, -.14), tilt=(16, 0), l=fk((46, 8, -5), 72, -12), r=fk((58, 6, 4), 82, -14), hl=G_L, hr=G_R),
    sf(.92),
]
SWORD_DUEL = [
    sw(-.96),
    sw(-.73, pel=(-.02, -.03, -.16), tilt=(18, -4), l=fk((66, 8, -5), 90, -14), r=fk((36, 10, 5), 56, -10), hl=arm((125, 35, -18), 95), hr=arm((135, 28, 22), 72)),
    sw(-.47, pel=(-.04, -.08, -.14), tilt=(16, -10), chest=(4, -8, -42), hl=arm((108, 46, -15), 110), hr=arm((96, 22, 28), 26)),
    sw(-.47, pel=(-.06, -.09, -.16), tilt=(18, -13), chest=(6, -12, -48), hl=arm((114, 50, -18), 116), hr=arm((102, 20, 30), 22)),
    sw(-.40, pel=(.03, -.10, -.13), tilt=(14, 8), chest=(0, 8, 26), hl=arm((92, 20, -10), 88), hr=arm((72, 68, 25), 128)),
    sw(-.38, pel=(.04, -.11, -.14), tilt=(15, 10), chest=(0, 10, 34), l=fk((34, 6, -4), 54, -12), hl=arm((98, 18, -10), 92), hr=arm((80, 75, 28), 136)),
    sw(-.39, pel=(-.02, -.10, -.12), tilt=(14, -5), chest=(0, -6, -12), hl=arm((70, 25, 0), 100), hr=arm((58, 12, 8), 82)),
    sw(-.55, pel=(-.06, .05, -.10), tilt=(-8, -10), chest=(-10, -8, -20), head=(-16, 0, -10), l=pl(.16, -.02), r=pl(-.18, .30), hl=arm((55, 45, 0), 80), hr=arm((45, 25, 0), 72)),
    sw(-.48, pel=(.02, -.05, -.16), tilt=(20, 8), chest=(8, 8, 38), hl=arm((88, 30, -12), 102), hr=arm((108, 18, 24), 32)),
    sw(-.48, pel=(.04, -.07, -.18), tilt=(22, 12), chest=(10, 12, 44), hl=arm((92, 34, -14), 108), hr=arm((116, 16, 28), 25)),
    sw(-.37, pel=(-.03, .02, -.10), tilt=(2, -10), chest=(-12, -8, -22), head=(-16, 0, -8), hl=arm((60, 50, 0), 70), hr=arm((48, 40, 0), 78)),
    sw(-.37, pel=(-.05, .04, -.08), tilt=(-6, -14), chest=(-18, -12, -30), head=(-22, 0, -12), hl=arm((48, 60, 0), 58), hr=arm((42, 50, 0), 65)),
    sw(-.42, pel=(.02, -.02, -.18), tilt=(22, 5), chest=(8, 4, 44), hl=arm((118, 32, -18), 104), hr=arm((145, 20, 28), 52)),
    sw(-.48, pel=(.08, -.04, -.20), tilt=(26, 12), chest=(12, 10, 58), head=(12, 20, 5), l=fk((5, 2, 0), 10, 0), hl=arm((130, 28, -22), 96), hr=arm((164, 12, 32), 30)),
    sw(-.38, pel=(-.02, -.10, -.14), tilt=(14, -6), chest=(0, -6, -30), l=fk((30, 8, -5), 48, -10), hl=arm((100, 34, -14), 105), hr=arm((72, 62, 24), 122)),
    sw(-.36, pel=(-.03, -.11, -.15), tilt=(16, -8), chest=(2, -8, -34), l=fk((14, 4, -2), 25, -5), hl=arm((104, 38, -16), 110), hr=arm((78, 68, 26), 130)),
    sw(-.43, pel=(.04, -.04, -.18), tilt=(22, 8), chest=(12, 8, 36), hl=arm((122, 42, -20), 112), hr=arm((138, 34, 30), 86)),
    sw(-.42, pel=(.06, -.05, -.20), tilt=(24, 10), chest=(14, 10, 44), hl=arm((130, 48, -24), 120), hr=arm((146, 38, 34), 92)),
    sw(-.66, pel=(0, .02, -.14), tilt=(16, 0), l=fk((58, 6, -4), 82, -14), r=fk((46, 8, 5), 72, -12), hl=SWORD_GUARD_L, hr=SWORD_GUARD_R),
    sw(-.92),
]
DUEL_TAGS = ("ready", "dash", "slash_block", "clash_hold", "fist_counter", "sword_parry", "pommel_hit", "fist_recoil",
             "low_cut", "shin_check", "elbow_hit", "sword_recoil", "reversal", "perfect_slip", "body_cross", "sword_hit",
             "final_rush", "mutual_clash", "disengage", "recover")
timed("Duel", "Shonen_Fist_Duel", [(p, dt, "snap" if tag in {"fist_counter", "elbow_hit", "body_cross", "final_rush"} else "smooth", tag)
                                           for p, dt, tag in zip(FIST_DUEL, DUEL_DT, DUEL_TAGS)])
timed("Duel", "Shonen_Sword_Duel", [(p, dt, "snap" if tag in {"slash_block", "pommel_hit", "low_cut", "reversal", "mutual_clash"} else "smooth", tag)
                                            for p, dt, tag in zip(SWORD_DUEL, DUEL_DT, DUEL_TAGS)])

# Compact, in-place gameplay clips cut from the same visual language.  Their short anticipation, six-to-eight-frame
# strike and visible impact hold keep high attack speed readable instead of simply accelerating an ordinary clip.
FLASH_FIST = [sf(0), sf(0, pel=(.02, .03, -.20), tilt=(24, 4), chest=(8, 4, 18), hr=arm((32, 35, 0), 128)),
              sf(0, pel=(.02, -.10, -.13), tilt=(15, 4), chest=(2, 4, -36), l=pl(.18, -.34), hr=arm((96, 2, 0), 4)),
              sf(0, pel=(.02, -.10, -.14), tilt=(16, 4), chest=(3, 4, -38), l=pl(.18, -.34), hr=arm((98, 2, 0), 3)), sf(0)]
FIST_PARRY = [sf(0), sf(0, pel=(.06, 0, -.18), tilt=(22, 15), chest=(8, 12, -12), hl=arm((118, 62, -30), 128)),
              sf(0, pel=(-.04, -.08, -.14), tilt=(14, -10), chest=(-2, -10, 32), hl=arm((92, 12, 0), 18)),
              sf(0, pel=(.02, -.10, -.15), tilt=(18, 5), chest=(4, 5, -32), hr=arm((96, 3, 0), 5)), sf(0)]
FLASH_SWORD = [sw(0), sw(0, pel=(-.02, .02, -.18), tilt=(22, -4), chest=(8, -4, 26), hl=arm((128, 34, -18), 100), hr=arm((150, 22, 28), 58)),
               sw(0, pel=(-.04, -.09, -.14), tilt=(16, -10), chest=(4, -10, -48), hl=arm((114, 50, -18), 116), hr=arm((102, 20, 30), 22)),
               sw(0, pel=(-.04, -.09, -.15), tilt=(17, -10), chest=(5, -10, -50), hl=arm((116, 52, -18), 118), hr=arm((104, 18, 30), 20)), sw(0)]
SWORD_PARRY = [sw(0), sw(0, pel=(.03, -.04, -.17), tilt=(21, 8), chest=(8, 8, 32), hl=arm((88, 26, -10), 94), hr=arm((70, 66, 24), 126)),
               sw(0, pel=(.05, -.06, -.18), tilt=(23, 12), chest=(10, 12, 42), hl=arm((94, 32, -14), 104), hr=arm((78, 74, 28), 136)),
               sw(0, pel=(-.02, -.08, -.14), tilt=(15, -6), chest=(2, -6, -30), hl=arm((102, 38, -16), 108), hr=arm((76, 65, 26), 128)), sw(0)]
for n, poses, impact in (("Shonen_Flash_Cross", FLASH_FIST, 2), ("Shonen_Perfect_Counter", FIST_PARRY, 2),
                         ("Shonen_Flash_Cut", FLASH_SWORD, 2), ("Shonen_Perfect_Parry", SWORD_PARRY, 2)):
    A("Duel", n, poses, dt=.11, fast=(impact,), slow=(1,))

# Self-authored bow sequence used in place of the generic archery mocap. The bow hand stays
# locked downrange while the string hand nocks, pulls to the cheek, releases in two frames and
# follows through. Timings are exposed directly in bow.json (aim .30, draw .30, release .75).
BOW_AIM = P(ORTHO, yaw=0, pel=(0, 0, -.10), tilt=(10, 0), chest=(0, 0, 0), head=(2, 0, 0),
            l=pl(.16, -.18), r=pl(-.18, .20), hl=arm((92, 5, -4), 8, 0, 10), hr=arm((78, 12, 6), 28, 0, 10))
BOW_DRAW = P(BOW_AIM, chest=(0, 0, -8), head=(1, 4, 0),
             hl=arm((96, 3, -5), 5, 0, 10), hr=arm((56, 18, 18), 124, 0, 10))
BOW_RELEASE = P(BOW_AIM, chest=(0, 0, -12), head=(0, 6, 0),
                hl=arm((98, 2, -5), 4, 0, 10), hr=arm((50, 12, 24), 122, 0, 5))
timed("Duel", "Shonen_Bow_Shot", [
    (P(BOW_AIM, hl=arm((82, 8, -2), 22, 0, 10), hr=arm((70, 16, 5), 45, 0, 10)), .15, "smooth", "ready"),
    (BOW_AIM, .15, "smooth", "aim"),
    (P(BOW_AIM, hr=arm((90, 4, 2), 12, 0, 10)), .12, "smooth", "nock"),
    (BOW_DRAW, .28, "smooth", "full_draw"),
    (BOW_RELEASE, .05, "snap", "release"),
    (P(BOW_RELEASE, hr=arm((45, 8, 28), 128, 0, 5)), .12, "out", "follow_through"),
    (BOW_AIM, .18, "smooth", "recover"),
])

# --- Sheet 1: mage casting --------------------------------------------------------------------------------------
TH = lambda **k: P(MAGE, **k)  # noqa: E731
A("Mage", "Focus_Idle", [MAGE, TH(pel=(.01, 0, -.18), hl=arm((75, 15, 0), 60, 0, 10), hr=arm((62, 30, 0), 85, 0, 10)),
                         TH(pel=(0, .02, -.17), chest=(0, 0, 6), hl=arm((80, 10, 0), 55, 0, 10), hr=arm((70, 25, 0), 70, 0, 10)),
                         TH(pel=(-.01, .03, -.16), hl=arm((65, 25, 0), 80, 0, 10), hr=arm((78, 15, 0), 60, 0, 10)),
                         TH(pel=(0, .01, -.18), chest=(0, 0, -5)), MAGE], dt=.2)
A("Mage", "One_Hand_Cast", [MAGE, TH(hr=arm((20, 40, 0), 100, -40, 10), chest=(0, 0, 15), pel=(0, .04, -.17)),
                            TH(hr=arm((88, 10, 0), 5, -40, 0), chest=(0, 0, -25), pel=(0, -.08, -.20), tilt=(18, 0)),
                            TH(hr=arm((92, 12, 0), 3, -50, 0), chest=(0, 0, -28), pel=(0, -.09, -.20), tilt=(18, 0)),
                            TH(hr=arm((60, 20, 0), 70, 0, 10)), MAGE], fast=(2,))
A("Mage", "Two_Hand_Forward_Cast", [MAGE, TH(hl=arm((45, 15, 0), 100, 0, 10), hr=arm((45, 15, 0), 100, 0, 10), chest=(6, 0, 0)),
                                    TH(hl=arm((40, 10, 0), 110, -20, 10), hr=arm((40, 10, 0), 110, -20, 10), pel=(0, .05, -.18)),
                                    TH(hl=arm((70, 12, 0), 50, -30, 5), hr=arm((70, 12, 0), 50, -30, 5), pel=(0, -.04, -.19)),
                                    TH(hl=arm((90, 12, 0), 5, -50, 0), hr=arm((90, 12, 0), 5, -50, 0), pel=(0, -.10, -.20), tilt=(18, 0)),
                                    TH(hl=arm((92, 12, 0), 3, -55, 0), hr=arm((92, 12, 0), 3, -55, 0), pel=(0, -.10, -.20), tilt=(18, 0)),
                                    TH(hl=arm((60, 15, 0), 70, 0, 10), hr=arm((60, 15, 0), 70, 0, 10)), MAGE], fast=(3,))
A("Mage", "Overhead_Cast", [TH(hl=arm((160, 15, 0), 70, 0, 10), hr=arm((160, 15, 0), 70, 0, 10), pel=(0, 0, -.18)),
                            TH(hl=arm((168, 10, 0), 50, 0, 10), hr=arm((168, 10, 0), 50, 0, 10), chest=(-8, 0, 0)),
                            TH(hl=arm((175, 12, 0), 10, -20, 0), hr=arm((175, 12, 0), 10, -20, 0), chest=(-12, 0, 0), pel=(0, 0, -.06),
                               tilt=(0, 0)),
                            TH(hl=arm((115, 25, 0), 20, -40, 0), hr=arm((115, 25, 0), 20, -40, 0), chest=(20, 0, 0), pel=(0, -.04, -.24), tilt=(20, 0)),
                            TH(hl=arm((60, 15, 0), 70, 0, 10), hr=arm((88, 10, 0), 8, -40, 0), pel=(0, -.06, -.20)), MAGE], fast=(3,))
A("Mage", "Low_Cast", [TH(pel=(0, 0, -.50), tilt=(32, 0), hl=arm((35, 15, 0), 40, 0, 10), hr=arm((35, 15, 0), 40, 0, 10), chest=(10, 0, 0)),
                       TH(pel=(0, -.08, -.48), tilt=(34, 0), chest=(25, 0, -20), hl=arm((35, 25, 0), 40, 0, 10), hr=arm((55, 5, 0), 10, -30, 0)),
                       TH(pel=(0, -.04, -.30), tilt=(22, 0), hr=arm((70, 10, 0), 30, -30, 0)),
                       TH(pel=(0, 0, -.52), tilt=(34, 0), hl=arm((30, 20, 0), 20, 0, 10), hr=arm((30, 20, 0), 20, 0, 10), chest=(15, 0, 0)),
                       TH(pel=(0, 0, -.32), tilt=(22, 0)), MAGE], dt=.16)
A("Mage", "Palm_Thrust_Cast", [MAGE, TH(hr=arm((35, 30, 0), 110, -80, 0), pel=(0, .03, -.19), chest=(0, 0, 16)),
                               TH(hr=arm((90, 5, 0), 5, -75, 0), chest=(0, 0, -28), pel=(0, -.14, -.22), tilt=(18, 0), l=pl(.24, -.34),
                                  r=pl(-.24, .08)),
                               TH(hr=arm((92, 5, 0), 3, -80, 0), chest=(0, 0, -30), pel=(0, -.14, -.22), tilt=(18, 0), l=pl(.24, -.34),
                                  r=pl(-.24, .08)), MAGE], fast=(2,))
A("Mage", "Sigil_Hand_Motion", [MAGE, TH(hr=arm((80, 10, 0), 60, 0, 10)), TH(hr=arm((100, 40, 0), 80, 0, 10)),
                                TH(hr=arm((125, 20, 0), 100, 0, 10), chest=(-6, 0, 0)), TH(hr=arm((95, -10, 0), 90, 0, 10)),
                                TH(hr=arm((70, -20, 0), 70, 0, 10), hl=arm((80, 25, 0), 70, 0, 10)),
                                TH(hr=arm((60, 30, 0), 40, 0, 10), hl=arm((90, 30, 0), 60, 0, 10))], dt=.16)
A("Mage", "Step_In_Cast", [MAGE, TH(l=pl(.24, -.42), pel=(0, -.06, -.18), hr=arm((30, 30, 0), 100, 0, 10)),
                           TH(l=pl(.24, -.46), r=pl(-.24, .02), pel=(0, -.14, -.20), tilt=(16, 0), hr=arm((60, 20, 0), 60, 0, 10)),
                           TH(l=pl(.24, -.46), r=pl(-.24, .02), pel=(0, -.16, -.20), tilt=(18, 0), hr=arm((92, 8, 0), 4, -50, 0),
                              chest=(0, 0, -22)),
                           TH(l=pl(.24, -.40), r=pl(-.24, .08), hr=arm((70, 15, 0), 50, 0, 10)), MAGE], fast=(3,))
A("Mage", "Backstep_Cast", [MAGE, TH(l=pl(.24, -.10), r=pl(-.24, .30), pel=(0, .06, -.14), hr=arm((40, 30, 0), 90, 0, 10)),
                            TH(l=pl(.24, .05), r=pl(-.24, .40), pel=(0, .10, -.14), tilt=(6, 0), plant=.7),
                            TH(l=pl(.24, .02), r=pl(-.24, .42), pel=(0, .10, -.18), chest=(0, 0, -20), hr=arm((90, 10, 0), 6, -45, 0)),
                            TH(l=pl(.24, .0), r=pl(-.24, .40), pel=(0, .08, -.18), hr=arm((92, 12, 0), 4, -50, 0), chest=(0, 0, -24)),
                            MAGE], fast=(3,))
A("Mage", "Air_Cast", [TH(pel=(0, 0, -.38), tilt=(26, 0)),
                       fl(pel=(0, -.05, .30), l=fk((80, 0, 0), 110), r=fk((80, 0, 0), 110), hl=arm((60, 25, 0), 70, 0, 10), hr=arm((60, 25, 0), 70, 0, 10)),
                       fl(pel=(0, -.1, .45), l=fk((70, 10, 0), 100), r=fk((90, -10, 0), 80), hl=arm((60, 25, 0), 70, 0, 10),
                          hr=arm((90, 15, 0), 5, -45, 0), chest=(0, 0, -15)),
                       fl(pel=(0, -.1, .35), l=fk((60, 0, 0), 80), r=fk((60, 0, 0), 80), hr=arm((88, 10, 0), 8, -50, 0)),
                       TH(pel=(0, -.05, -.36), tilt=(24, 0)), MAGE], dt=.14)
A("Mage", "Cast_Cancel_Recovery", [TH(hr=arm((92, 12, 0), 4, -50, 0), pel=(0, -.1, -.20), chest=(0, 0, -25), tilt=(18, 0)),
                                   TH(hr=arm((60, 25, 0), 90, 0, 40), pel=(0, 0, -.22)),
                                   TH(pel=(0, .02, -.34), tilt=(26, 0), hl=arm((55, 15, 0), 120, 0, 60), hr=arm((55, 15, 0), 125, 0, 60)),
                                   TH(pel=(0, 0, -.24)), MAGE], dt=.11)
A("Mage", "Return_To_Stance", [MAGE, P(MAGE, pel=(0, 0, -.13), hl=arm((64, 15, 0), 90, 0, 40), hr=arm((62, 15, 0), 100, 0, 40),
                                       l=pl(.20, -.18), r=pl(-.20, .21)),
                               P(ORTHO, pel=(0, 0, -.09), l=pl(.16, -.16), r=pl(-.18, .20)), ORTHO], dt=.16)

# --- Sheet 3: evasive movement ----------------------------------------------------------------------------------
RD = P(ORTHO, pel=(0, 0, -.10), tilt=(14, 0))
A("Evasive", "Ready_Stance", [RD, P(RD, pel=(0, .01, -.12), tilt=(16, 0)), P(RD, pel=(.01, .02, -.13), tilt=(17, 0), chest=(0, 0, 4)),
                              P(RD, pel=(0, .01, -.11), tilt=(15, 0)), P(RD, pel=(-.01, 0, -.09), chest=(0, 0, -4)), RD], dt=.16)


def weave(side, depth=1.0):
    s = 1 if side == "L" else -1
    return [P(RD, pel=(s * .05, 0, -.13), tilt=(16, s * 6), chest=(0, s * 6, 0), head=(12, -15, -s * 8)),
            P(RD, pel=(s * .12 * depth, 0, -.18 * depth - .06), tilt=(22, s * 15 * depth), chest=(4, s * 10, 0), head=(14, -15, -s * 15)),
            P(RD, pel=(s * .06, 0, -.12), tilt=(16, s * 6), head=(10, -15, -s * 6))]


A("Evasive", "Head_Weave_Left", [RD] + weave("L") + [RD], dt=.13)
A("Evasive", "Head_Weave_Right", [RD] + weave("R") + [RD], dt=.13)
A("Evasive", "Duck_And_Rise", [RD, P(RD, pel=(0, .02, -.28), tilt=(26, 0)),
                               P(RD, pel=(0, .04, -.44), tilt=(38, 0), head=(18, -15, 0), hl=arm((75, 15, 0), 120), hr=arm((75, 15, 0), 130)),
                               P(RD, pel=(0, .02, -.40), tilt=(34, 0)), P(RD, pel=(0, 0, -.20), tilt=(20, 0)), RD], dt=.12)
A("Evasive", "Slip_Back", [RD, P(RD, pel=(0, .10, -.08), tilt=(-12, 0), head=(-6, -15, 0), hl=arm((55, 15, 0), 140), hr=arm((55, 15, 0), 140)),
                           P(RD, pel=(0, .18, -.04), tilt=(-22, 0), head=(-12, -15, 0), chest=(-8, 0, 0), l=pl(.12, -.02), r=pl(-.14, .32)),
                           P(RD, pel=(0, .08, -.10), tilt=(8, 0)), RD], dt=.12)
A("Evasive", "Bob_And_Weave_Combo", [RD] + weave("L")[:2] + weave("R")[:2] + [P(RD, pel=(0, 0, -.30), tilt=(30, 0)) ] + weave("L")[1:2] + [RD], dt=.1)
A("Evasive", "Pivot_Evade", [RD, P(RD, yaw=45, pel=(.04, 0, -.14), tilt=(18, 0), r=pl(-.14, .10)),
                             P(RD, yaw=80, pel=(.06, 0, -.16), tilt=(20, 0), chest=(0, 0, -15), r=pl(-.30, .02)),
                             P(RD, yaw=110, pel=(.06, .02, -.14), tilt=(18, 6), chest=(0, 0, -25), head=(12, 30, 0), r=pl(-.34, -.06)),
                             P(RD, yaw=60, pel=(.04, 0, -.12), tilt=(16, 0)), RD], dt=.12)
A("Evasive", "Crouch_Feint", [RD, P(RD, pel=(0, .02, -.26), tilt=(26, 0), hl=arm((70, 15, 0), 125), hr=arm((65, 15, 0), 135)),
                              P(RD, pel=(0, .04, -.42), tilt=(36, 0), head=(18, -15, 0)),
                              P(RD, pel=(0, -.02, -.20), tilt=(18, 0), hl=arm((80, 12, 0), 100)), RD], dt=.1)
A("Evasive", "Advance_Weave", [RD, P(RD, l=pl(.12, -.30), pel=(.06, -.04, -.16), tilt=(20, 8), head=(14, -15, -8)),
                               P(RD, l=pl(.12, -.30), r=pl(-.14, .04), pel=(.10, -.10, -.20), tilt=(22, 14)),
                               P(RD, l=pl(.12, -.30), r=pl(-.14, .04), pel=(-.02, -.10, -.16), tilt=(18, -4)),
                               P(RD, l=pl(.12, -.30), r=pl(-.14, .04), pel=(-.10, -.10, -.20), tilt=(22, -14)),
                               P(RD, l=pl(.12, -.20), r=pl(-.14, .10), pel=(-.04, -.06, -.14)), RD], dt=.11)
A("Evasive", "Retreat_Weave", [RD, P(RD, l=pl(.12, -.02), r=pl(-.14, .34), pel=(.06, .08, -.16), tilt=(14, 8)),
                               P(RD, l=pl(.12, 0), r=pl(-.14, .36), pel=(.10, .12, -.20), tilt=(16, 14)),
                               P(RD, l=pl(.12, 0), r=pl(-.14, .36), pel=(-.02, .12, -.16), tilt=(14, -4)),
                               P(RD, l=pl(.12, 0), r=pl(-.14, .36), pel=(-.10, .12, -.20), tilt=(16, -14)),
                               P(RD, l=pl(.12, -.08), r=pl(-.14, .26), pel=(-.04, .06, -.14)), RD], dt=.11)
LOW = P(RD, pel=(.06, .06, -.46), tilt=(40, 12), chest=(10, 8, 0), head=(20, -15, -10), hl=arm((40, 25, 0), 70), hr=arm((40, 25, 0), 70))
A("Evasive", "Low_Sway_Recovery", [LOW, P(LOW, pel=(-.06, .06, -.48), tilt=(42, -12), chest=(12, -8, 0), head=(20, -15, 10)),
                                   P(LOW, pel=(0, .04, -.42), tilt=(36, 0)), P(RD, pel=(0, 0, -.24), tilt=(22, 0)), RD], dt=.16)
A("Evasive", "Reset_To_Guard", [P(RD, pel=(0, 0, -.22), tilt=(20, 0)), P(RD, pel=(0, 0, -.14), tilt=(16, 0)), P(RD, pel=(0, 0, -.11)), ORTHO], dt=.14)

# --- Sheet 4: berserk activation -------------------------------------------------------------------------------
CALM = P(ORTHO, pel=(0, 0, -.08), tilt=(10, 0))
A("Berserk", "Calm_Combat_Stance", [CALM, P(CALM, pel=(0, .01, -.09), chest=(0, 0, 3)), P(CALM, pel=(0, .01, -.10), tilt=(11, 0)),
                                    P(CALM, pel=(0, 0, -.08)), CALM], dt=.2)
CR = P(BERSERK, pel=(0, .02, -.46), tilt=(44, 0), chest=(18, 0, 0), head=(26, 0, 0), hl=arm((30, 20, 0), 100, 0, 90), hr=arm((30, 20, 0), 100, 0, 90))
A("Berserk", "Berserk_Activation", [CR, P(CR, pel=(0, .02, -.48), chest=(22, 0, 4)),
                                    P(BERSERK, pel=(0, 0, -.38), tilt=(38, 0), chest=(16, 0, 0), hl=arm((-10, 30, 0), 80, 0, 90), hr=arm((-10, 30, 0), 80, 0, 90)),
                                    P(BERSERK, pel=(0, 0, -.30), tilt=(24, 0), chest=(6, 0, 0), head=(10, 0, 0), hl=arm((-5, 45, 0), 60, 0, 100), hr=arm((-5, 45, 0), 60, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.26), tilt=(10, 0), chest=(-4, 0, 0), head=(-6, 0, 0), hl=arm((10, 60, 0), 50, 0, 100), hr=arm((10, 60, 0), 50, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.22), tilt=(2, 0), chest=(-10, 0, 0), head=(-22, 0, 0), hl=arm((15, 70, 0), 45, 0, 100), hr=arm((15, 70, 0), 45, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.20), tilt=(-4, 0), chest=(-16, 0, 0), head=(-32, 0, 0), l=pl(.40, -.18), r=pl(-.40, .16),
                                      hl=arm((20, 80, 0), 40, 0, 100), hr=arm((20, 80, 0), 40, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.18), tilt=(-6, 0), chest=(-20, 0, 0), head=(-40, 0, 0), l=pl(.44, -.16), r=pl(-.44, .14),
                                      hl=arm((20, 88, 0), 35, 0, 100), hr=arm((20, 88, 0), 35, 0, 100))], dt=.2, slow=(7,))
ROAR = P(BERSERK, pel=(0, 0, -.22), tilt=(0, 0), chest=(-14, 0, 0), head=(-34, 0, 0), l=pl(.42, -.18), r=pl(-.42, .16),
         hl=arm((20, 80, 0), 40, 0, 100), hr=arm((20, 80, 0), 40, 0, 100))
A("Berserk", "Roar_Pose", [P(BERSERK, pel=(0, 0, -.30), tilt=(20, 0), head=(10, 0, 0), hl=arm((20, 50, 0), 60, 0, 100), hr=arm((20, 50, 0), 60, 0, 100)),
                           P(ROAR, pel=(0, 0, -.26), chest=(-6, 0, 0), head=(-16, 0, 0)), ROAR,
                           P(ROAR, chest=(-20, 0, 0), head=(-42, 0, 0), hl=arm((25, 90, 0), 35, 0, 100), hr=arm((25, 90, 0), 35, 0, 100)),
                           P(ROAR, chest=(-18, 0, 0), head=(-38, 0, 0))], dt=.16)
A("Berserk", "Power_Surge_Breath", [P(BERSERK, pel=(0, 0, -.40), tilt=(34, 0), hl=arm((30, 15, 0), 110, 0, 100), hr=arm((30, 15, 0), 110, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.44), tilt=(40, 0), chest=(20, 0, 0), head=(26, 0, 0), hl=arm((35, 15, 0), 120, 0, 100), hr=arm((35, 15, 0), 120, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.30), tilt=(16, 0), chest=(0, 0, 0), hl=arm((0, 30, 0), 80, 0, 100), hr=arm((0, 30, 0), 80, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.24), tilt=(4, 0), chest=(-14, 0, 0), head=(-18, 0, 0), hl=arm((-5, 40, 0), 60, 0, 100), hr=arm((-5, 40, 0), 60, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.28), tilt=(14, 0), chest=(-4, 0, 0), hl=arm((0, 35, 0), 70, 0, 100), hr=arm((0, 35, 0), 70, 0, 100)),
                                    P(BERSERK, pel=(0, 0, -.30), tilt=(22, 0))], dt=.2)
FERAL = P(BERSERK, pel=(0, -.02, -.28), tilt=(34, 0), chest=(16, 0, 0), head=(28, 0, 0),
          hl=arm((45, 30, 0), 60, 0, 50), hr=arm((40, 35, 0), 70, 0, 50))
A("Berserk", "Feral_Guard", [FERAL, P(FERAL, chest=(18, 0, 8), hl=arm((50, 28, 0), 55, 0, 50), pel=(.02, -.03, -.30)),
                             P(FERAL, chest=(14, 0, -6), hr=arm((48, 32, 0), 60, 0, 50)), P(FERAL, pel=(-.02, -.01, -.27)), FERAL], dt=.18)
LUNGE = P(BERSERK, pel=(0, .02, -.44), tilt=(46, 0), chest=(14, 0, 0), head=(22, 0, 0), hl=arm((-30, 20, 0), 60, 0, 90), hr=arm((55, 25, 0), 60, 0, 90))
A("Berserk", "Lunge_Start", [P(BERSERK, pel=(0, 0, -.42), tilt=(40, 0), hl=arm((20, 30, 0), 90, 0, 90), hr=arm((20, 30, 0), 90, 0, 90)),
                             LUNGE, P(LUNGE, pel=(0, -.02, -.40), chest=(20, 0, -14), hl=arm((-45, 25, 0), 40, 0, 90)),
                             P(LUNGE, pel=(0, -.10, -.34), tilt=(42, 0), chest=(20, 0, -26), l=pl(.34, -.42), hl=arm((-60, 25, 0), 30, 0, 90), hr=arm((60, 20, 0), 50, 0, 90)),
                             P(LUNGE, pel=(0, -.16, -.30), tilt=(40, 0), chest=(20, 0, -36), l=pl(.34, -.50), r=pl(-.34, .10), hl=arm((-70, 30, 0), 20, 0, 90), hr=arm((70, 10, 0), 40, 0, 90)),
                             P(FERAL, pel=(0, -.12, -.28), l=pl(.34, -.46), r=pl(-.34, .12))], dt=.12, fast=(3,))
A("Berserk", "Heavy_Step_In", [FERAL, P(FERAL, r=fk((60, 8, 0), 85, -10), pel=(.08, -.02, -.22), tilt=(24, 0)),
                               P(FERAL, r=fk((70, 8, 0), 60, -10), pel=(.08, -.08, -.20), tilt=(24, 0)),
                               P(FERAL, l=pl(.34, -.34), r=pl(-.34, -.05), pel=(0, -.14, -.34), tilt=(38, 0), chest=(20, 0, 0)),
                               P(FERAL, pel=(0, -.10, -.30), l=pl(.34, -.30), r=pl(-.34, 0.0))], dt=.13)
A("Berserk", "Wild_Swing_Windup", [P(FERAL, hr=arm((95, 40, 0), 130, 0, 90), chest=(14, 0, 20)),
                                   P(FERAL, hr=arm((140, 30, 0), 90, 0, 90), chest=(10, 0, 32), tilt=(24, 0)),
                                   P(FERAL, hr=arm((172, 20, 0), 60, 0, 90), chest=(-6, 0, 26), tilt=(8, 0), pel=(0, .02, -.20)),
                                   P(FERAL, hr=arm((120, 60, 0), 30, 0, 90), chest=(-12, 0, 40), tilt=(10, 0)),
                                   P(FERAL, hr=arm((-40, 40, 0), 40, 0, 90), chest=(10, 0, 55), tilt=(22, 0), pel=(0, .04, -.30)),
                                   FERAL], dt=.14)
A("Berserk", "Grounding_Recover", [P(BERSERK, pel=(0, .02, -.34), tilt=(34, 0), hl=arm((40, 20, 0), 60, 0, 90), hr=arm((40, 20, 0), 60, 0, 90)),
                                   P(BERSERK, pel=(0, .04, -.54), tilt=(54, 0), chest=(20, 0, 0), head=(24, 0, 0), hl=arm((75, 15, 0), 10, 0, 40), hr=arm((75, 15, 0), 10, 0, 40)),
                                   P(BERSERK, pel=(0, .02, -.50), tilt=(48, 0), hl=arm((70, 15, 0), 15, 0, 40), hr=arm((70, 15, 0), 15, 0, 40)),
                                   P(BERSERK, pel=(0, 0, -.36), tilt=(32, 0), hl=arm((50, 20, 0), 50, 0, 60), hr=arm((50, 20, 0), 50, 0, 60)),
                                   P(ORTHO, pel=(0, 0, -.12), tilt=(14, 0))], dt=.16)
A("Berserk", "Berserk_Idle_Loop", [FERAL, P(FERAL, chest=(20, 0, 0), pel=(0, 0, -.29), hl=arm((48, 32, 0), 62, 0, 50)),
                                   P(FERAL, chest=(14, 0, 0), pel=(0, -.01, -.31), tilt=(36, 0)),
                                   P(FERAL, hr=arm((45, 38, 0), 68, 0, 50), pel=(0, -.02, -.30)),
                                   P(FERAL, chest=(18, 0, -5), pel=(.01, 0, -.28)), FERAL], dt=.18)

# --- Sheet 5: hit reactions ------------------------------------------------------------------------------------
TIGHT = {"hl": arm((75, 15, 0), 150), "hr": arm((70, 15, 0), 155)}
A("Hit", "Guard_Impact", [ORTHO, P(pel=(0, .06, -.07), tilt=(2, 0), **TIGHT), P(pel=(0, .09, -.06), tilt=(-4, 0), chest=(-4, 0, 0), head=(6, -15, 0), **TIGHT),
                          P(pel=(0, .04, -.07), tilt=(6, 0), **TIGHT), ORTHO], dt=.09)
A("Hit", "Light_Hit_Reaction", [ORTHO, P(chest=(-4, 0, 22), head=(2, 30, 10), pel=(0, .04, -.05), tilt=(6, 6), hl=arm((40, 40, 0), 90), hr=arm((55, 25, 0), 120)),
                                P(chest=(-8, 6, 30), head=(0, 40, 14), pel=(0, .07, -.04), tilt=(0, 10), hl=arm((20, 55, 0), 70), hr=arm((55, 30, 0), 110)),
                                P(chest=(-2, 0, 12), head=(4, 18, 6), pel=(0, .04, -.06)), ORTHO], dt=.1)
BODY = P(pel=(0, .08, -.14), tilt=(34, 0), chest=(26, 0, 0), head=(26, 0, 0), hl=arm((30, 10, 0), 105), hr=arm((30, 10, 0), 105))
A("Hit", "Heavy_Body_Hit", [ORTHO, P(pel=(0, .06, -.08), tilt=(14, 0), chest=(10, 0, 0), hl=arm((40, 10, 0), 110), hr=arm((40, 10, 0), 110)),
                            BODY, P(BODY, pel=(0, .06, -.12), chest=(22, 0, 0)), P(ORTHO, pel=(0, 0, -.10), tilt=(16, 0))], dt=.12)
A("Hit", "Head_Hit_Snapback", [ORTHO, P(head=(-30, 10, 0), chest=(-8, 0, 0), tilt=(-4, 0), pel=(0, .05, -.05), hl=arm((55, 25, 0), 100), hr=arm((50, 30, 0), 110)),
                               P(head=(-48, 20, 0), chest=(-20, 0, 6), tilt=(-14, 0), pel=(0, .10, -.04), hl=arm((40, 55, 0), 70), hr=arm((30, 60, 0), 90)),
                               P(head=(-20, 10, 0), chest=(-10, 0, 0), tilt=(-6, 0), pel=(0, .06, -.06)), ORTHO], dt=.1)
A("Hit", "Stagger_Backward", [ORTHO, P(pel=(0, .08, -.06), tilt=(-10, 4), l=pl(.12, -.04), r=pl(-.14, .28), chest=(-6, 0, 10), hl=arm((40, 50, 0), 70), hr=arm((50, 55, 0), 80)),
                              P(pel=(0, .14, -.04), tilt=(-16, -6), l=pl(.12, .06), r=pl(-.14, .40), chest=(-10, 0, -10), hl=arm((30, 60, 0), 50), hr=arm((40, 65, 0), 60), head=(-10, 0, 0)),
                              P(pel=(0, .16, -.08), tilt=(-12, 8), l=pl(.12, .10), r=pl(-.14, .36), hl=arm((30, 65, 0), 50), hr=arm((20, 60, 0), 50)),
                              P(pel=(0, .08, -.10), tilt=(6, 0)), ORTHO], dt=.14)
A("Hit", "Stagger_Forward", [ORTHO, P(pel=(0, -.08, -.14), tilt=(30, 0), chest=(14, 0, 0), head=(26, 0, 0), l=pl(.12, -.26), hl=arm((40, 15, 0), 90), hr=arm((35, 15, 0), 100)),
                             P(pel=(0, -.14, -.20), tilt=(40, 0), chest=(20, 0, 0), head=(32, 0, 0), l=pl(.12, -.30), r=pl(-.14, .0), hl=arm((20, 20, 0), 60), hr=arm((20, 20, 0), 60)),
                             P(pel=(0, -.12, -.24), tilt=(42, 0), chest=(22, 0, 0), head=(34, 0, 0), l=pl(.12, -.30), r=pl(-.14, .04), hl=arm((10, 30, 0), 50), hr=arm((10, 30, 0), 50)),
                             P(pel=(0, -.04, -.14), tilt=(24, 0)), ORTHO], dt=.14)
LIE = fl(pel=(0, .05, -.82), tilt=(-90, 0), chest=(0, 0, 0), head=(0, 0, 0), l=fk((8, 0, 0), 4), r=fk((12, 0, 0), 4),
         hl=arm((-5, 25, 0), 10, 0, 20), hr=arm((-5, 25, 0), 12, 0, 20))
A("Hit", "Knockback", [ORTHO, fl(pel=(0, .15, -.02), tilt=(-30, 0), l=fk((30, 0, 0), 30), r=fk((20, 0, 0), 20), hl=arm((20, 60, 0), 60), hr=arm((20, 60, 0), 60)),
                       fl(pel=(0, .35, -.18), tilt=(-62, 0), l=fk((40, 0, 0), 30), r=fk((20, 0, 0), 40), hl=arm((10, 70, 0), 40), hr=arm((10, 70, 0), 40)),
                       fl(pel=(0, .50, -.50), tilt=(-80, 0), l=fk((50, 0, 0), 40), r=fk((30, 0, 0), 60), hl=arm((-10, 50, 0), 30), hr=arm((-10, 50, 0), 30)),
                       P(LIE, pel=(0, .60, -.80), l=fk((40, 0, 0), 50), r=fk((25, 0, 0), 30)), P(LIE, pel=(0, .66, -.82))], dt=.14)
A("Hit", "Launch_Reaction", [ORTHO, P(pel=(0, .04, -.30), tilt=(22, 0)),
                             fl(pel=(0, .10, .35), tilt=(-30, 0), l=fk((30, 0, 0), 60), r=fk((50, 0, 0), 80), hl=arm((20, 60, 0), 60), hr=arm((30, 50, 0), 60)),
                             fl(pel=(0, .20, .75), tilt=(-110, 10), l=fk((60, 0, 0), 90), r=fk((90, 0, 0), 70), hl=arm((30, 80, 0), 20), hr=arm((20, 70, 0), 30)),
                             fl(pel=(0, .35, .55), tilt=(-170, 0), l=fk((40, 20, 0), 60), r=fk((70, -10, 0), 40), hl=arm((20, 80, 0), 20), hr=arm((20, 80, 0), 20)),
                             P(LIE, pel=(0, .5, -.82))], dt=.15)
A("Hit", "Airborne_Tumble", [fl(pel=(0, .1, .40), tilt=(-40, 0), l=fk((40, 0, 0), 60), r=fk((50, 0, 0), 80)),
                             fl(pel=(0, .15, .55), tilt=(-110, 0), l=fk((90, 0, 0), 120), r=fk((90, 0, 0), 120), hl=arm((60, 20, 0), 100), hr=arm((60, 20, 0), 100)),
                             fl(pel=(0, .2, .60), tilt=(-190, 10), l=fk((100, 0, 0), 110), r=fk((100, 0, 0), 110), hl=arm((40, 40, 0), 90), hr=arm((40, 40, 0), 90)),
                             fl(pel=(0, .2, .55), tilt=(-260, 0), l=fk((80, 0, 0), 100), r=fk((60, 0, 0), 60)),
                             fl(pel=(0, .25, .55), tilt=(-320, 0), l=fk((60, 0, 0), 80), r=fk((70, 0, 0), 100), hl=arm((70, 30, 0), 40), hr=arm((70, 30, 0), 40)),
                             fl(pel=(0, .25, .45), tilt=(-380, -10), l=fk((50, 0, 0), 70), r=fk((70, 0, 0), 90)),
                             fl(pel=(0, .28, .30), tilt=(-410, 0), l=fk((50, 0, 0), 60), r=fk((40, 0, 0), 60)),
                             P(pel=(0, .1, -.32), tilt=(26, 0), yaw=25),
                             P(pel=(0, 0, -.14), tilt=(14, 0))], dt=.11)
A("Hit", "Falling_Reaction", [fl(pel=(0, .3, .20), tilt=(-55, 0), l=fk((40, 0, 0), 40), r=fk((30, 0, 0), 60), hl=arm((20, 70, 0), 50), hr=arm((10, 70, 0), 50)),
                              fl(pel=(0, .45, -.10), tilt=(-80, 10), l=fk((50, 0, 0), 70), r=fk((20, 0, 0), 50), hl=arm((40, 30, 0), 80), hr=arm((10, 60, 0), 40)),
                              fl(pel=(0, .55, -.45), tilt=(-85, 0), l=fk((60, 0, 0), 80), r=fk((50, 0, 0), 90)),
                              P(LIE, pel=(0, .6, -.80), l=fk((30, 0, 0), 40), r=fk((50, 0, 0), 70)),
                              P(LIE, pel=(0, .62, -.82), hl=arm((-5, 40, 0), 5, 0, 10), hr=arm((-5, 40, 0), 5, 0, 10)), P(LIE, pel=(0, .64, -.83))], dt=.15)
ALL4 = fl(pel=(0, .0, -.52), tilt=(78, 0), chest=(0, 0, 0), head=(-30, 0, 0), l=fk((105, 0, 0), 125), r=fk((105, 0, 0), 125),
          hl=arm((85, 10, 0), 8, 0, 30), hr=arm((85, 10, 0), 8, 0, 30))
A("Hit", "Hard_Landing", [P(LIE, pel=(0, .64, -.83)), fl(pel=(0, .6, -.78), tilt=(-70, 0), yaw=45, l=fk((40, 0, 0), 60), r=fk((10, 0, 0), 20), hl=arm((30, 40, 0), 60), hr=arm((20, 30, 0), 20)),
                          fl(pel=(0, .5, -.7), tilt=(-30, 0), yaw=70, l=fk((60, 0, 0), 100), r=fk((50, 0, 0), 90), hl=arm((70, 30, 0), 30), hr=arm((60, 30, 0), 40)),
                          fl(pel=(0, .2, -.55), tilt=(30, 0), yaw=40, l=fk((90, 10, 0), 120), r=fk((90, -10, 0), 110), hl=arm((90, 15, 0), 5), hr=arm((90, 15, 0), 5)),
                          ALL4, P(ALL4, head=(-25, 0, 0))], dt=.18)
A("Hit", "Knockdown_Crumple", [P(ORTHO, pel=(0, .04, -.20), tilt=(14, 0), head=(12, 0, 0), hl=arm((30, 20, 0), 90), hr=arm((30, 20, 0), 90)),
                               fl(pel=(0, .06, -.40), tilt=(24, 6), l=fk((70, 0, 0), 110), r=fk((60, 0, 0), 100), hl=arm((20, 40, 0), 60), hr=arm((20, 40, 0), 60), head=(20, 0, 0)),
                               fl(pel=(0, .08, -.62), tilt=(10, 50), chest=(0, 20, 0), l=fk((80, 0, 0), 110), r=fk((60, 0, 0), 120), hl=arm((20, 40, 0), 40), hr=arm((30, 20, 0), 60)),
                               fl(pel=(0, .08, -.82), tilt=(0, 85), l=fk((70, 0, 0), 100), r=fk((50, 0, 0), 100), hl=arm((20, 30, 0), 60), hr=arm((20, 30, 0), 70)),
                               fl(pel=(0, .08, -.85), tilt=(0, 90), l=fk((50, 0, 0), 80), r=fk((40, 0, 0), 80), hl=arm((10, 30, 0), 50), hr=arm((20, 20, 0), 60)),
                               fl(pel=(0, .08, -.86), tilt=(0, 90), l=fk((30, 0, 0), 60), r=fk((30, 0, 0), 60))], dt=.15)
A("Hit", "Get_Up_Recovery", [P(LIE, pel=(0, .64, -.83)), P(LIE, pel=(0, .62, -.80), tilt=(-82, 0), hl=arm((40, 40, 0), 30), hr=arm((40, 40, 0), 30)),
                             fl(pel=(0, .3, -.55), tilt=(40, 0), yaw=40, l=fk((100, 15, 0), 125), r=fk((105, -15, 0), 125), hl=arm((90, 15, 0), 5, 0, 30), hr=arm((90, 15, 0), 5, 0, 30)),
                             fl(pel=(0, .1, -.50), tilt=(45, 0), yaw=30, l=fk((85, 10, 0), 100), r=fk((115, -10, 0), 140), hl=arm((70, 20, 0), 20, 0, 30), hr=arm((50, 25, 0), 50, 0, 30)),
                             P(pel=(0, 0, -.52), tilt=(38, 0), l=pl(.12, -.34), r=pl(-.14, .0), hl=arm((60, 20, 0), 50, 0, 40), hr=arm((60, 20, 0), 60, 0, 40)),
                             P(pel=(0, 0, -.34), tilt=(26, 0), l=pl(.12, -.20), r=pl(-.14, .10)),
                             P(pel=(0, 0, -.18), tilt=(18, 0)), ORTHO], dt=.16)
A("Hit", "Dazed_Stunned", [P(pel=(.02, 0, -.12), tilt=(16, 8), chest=(0, 6, 0), head=(14, 8, 14), hl=arm((120, 25, 0), 135, 0, 30), hr=arm((50, 25, 0), 100)),
                           P(pel=(.06, .02, -.14), tilt=(18, 14), chest=(0, 10, 4), head=(16, 14, 20), hl=arm((125, 25, 0), 140, 0, 30), hr=arm((40, 35, 0), 90)),
                           P(pel=(.02, 0, -.12), tilt=(16, 4), head=(14, 0, 8), hl=arm((120, 25, 0), 135, 0, 30), hr=arm((50, 25, 0), 100)),
                           P(pel=(-.04, .02, -.14), tilt=(18, -10), chest=(0, -8, -4), head=(16, -12, -16), hl=arm((110, 30, 0), 130, 0, 30), hr=arm((40, 40, 0), 90)),
                           P(pel=(-.06, .02, -.16), tilt=(20, -14), chest=(0, -10, 0), head=(18, -16, -20), hl=arm((70, 30, 0), 100), hr=arm((35, 40, 0), 80)),
                           P(pel=(0, 0, -.12), tilt=(16, 0), head=(14, 0, 0))], dt=.2)
A("Hit", "Braced_Recovery", [P(pel=(0, .02, -.34), tilt=(28, 0), hl=arm((60, 20, 0), 120), hr=arm((55, 20, 0), 125)),
                             P(pel=(0, .02, -.30), tilt=(26, 0)), P(pel=(0, 0, -.22), tilt=(20, 0)), P(pel=(0, 0, -.14), tilt=(14, 0)), ORTHO], dt=.14)

# --- build / render / export ------------------------------------------------------------------------------------


def camera34(sc, res=(280, 340)):
    cam = bpy.data.cameras.new("c")
    cam.lens = 55
    co = bpy.data.objects.new("c", cam)
    sc.collection.objects.link(co)
    sc.camera = co
    co.location = (2.6, -2.9, 1.35)
    co.rotation_euler = (__import__("mathutils").Vector((0, 0.05, 0.85)) - co.location).to_track_quat("-Z", "Y").to_euler()
    sc.render.engine = "BLENDER_WORKBENCH"
    sc.display.shading.light = "STUDIO"
    sc.display.shading.color_type = "OBJECT"
    sc.view_settings.view_transform = "Standard"
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.world = bpy.data.worlds.new("w")
    sc.world.color = (0.95, 0.85, 0.88)
    sc.display.shading.background_type = "WORLD"
    for o in bpy.data.objects:
        if o.type == "MESH":
            o.color = (0.72, 0.70, 0.66, 1)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opt = {argv[i]: argv[i + 1] for i in range(0, len(argv) - 1, 2)}
    names = opt["--only"].split(",") if "--only" in opt else list(ANIMS)
    rig = K.setup()
    sc = bpy.context.scene
    # K.build keys in K.FPS frames.  Without this, Blender's 24 fps default exports every action
    # 2.5x too long even though the authored frame spacing and easing are otherwise correct.
    sc.render.fps = K.FPS
    if "--keys" in opt:
        out = opt["--keys"].rstrip("/")
        camera34(sc)
        for n in names:
            for i, (pz, _, _, _) in enumerate(ANIMS[n][1]):
                q = K.full(pz)
                K.pose(rig, q, K.origin_for(q))
                sc.render.filepath = f"{out}/{n}_{i + 1:02d}.png"
                bpy.ops.render.render(write_still=True)
        print("KEYS DONE", len(names))
    for n in names:
        K.SEQUENCES[f"Brawler_{n}"] = ANIMS[n][1]
    if "--render" in opt or "--export" in opt:
        acts = [K.build(rig, [f"Brawler_{n}"], f"Brawler_{n}")[0] for n in names]
        for a in acts:
            a.use_fake_user = True
    if "--render" in opt:
        out = opt["--render"].rstrip("/")
        camera34(sc, (640, 760))
        for n, a in zip(names, acts):
            os.makedirs(f"{out}/{n}", exist_ok=True)
            rig.arm.animation_data.action = a
            sc.frame_start, sc.frame_end = 1, int(a.frame_range[1])
            sc.frame_step = 2
            sc.render.filepath = f"{out}/{n}/f_"
            bpy.ops.render.render(animation=True)
    if "--export" in opt:
        rig.arm.animation_data.action = None
        for o in bpy.data.objects:
            o.select_set(o.type in ("ARMATURE", "MESH"))
        bpy.ops.export_scene.gltf(filepath=opt["--export"], export_format="GLB", use_selection=True,
                                  export_animation_mode="ACTIONS", export_force_sampling=True)
        print("EXPORTED", len(names), "actions")


if __name__ == "__main__":
    main()
