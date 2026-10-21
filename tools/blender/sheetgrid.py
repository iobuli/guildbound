"""Assemble per-animation key renders into labelled contact grids, one image per group.

    python3 -I tools/blender/sheetgrid.py KEYDIR OUTPREFIX [--group Brawler|Mage|...]
Row = one animation (frames left to right), the animation's name on the left.
"""
import glob
import os
import re
import sys

from PIL import Image, ImageDraw, ImageFont

ORDER = {
    "Brawler": ["Idle", "Step_In", "Jab", "Cross", "Hook", "Elbow_Strike", "Knee_Strike", "Front_Kick", "Roundhouse_Kick", "Spinning_Kick",
                "Rising_Uppercut", "Jump_Kick", "Aerial_Knee", "Aerial_Spin", "Axe_Kick", "Landing_Recovery"],
    "Mage": ["Focus_Idle", "One_Hand_Cast", "Two_Hand_Forward_Cast", "Overhead_Cast", "Low_Cast", "Palm_Thrust_Cast", "Sigil_Hand_Motion",
             "Step_In_Cast", "Backstep_Cast", "Air_Cast", "Cast_Cancel_Recovery", "Return_To_Stance"],
    "Evasive": ["Ready_Stance", "Head_Weave_Left", "Head_Weave_Right", "Duck_And_Rise", "Slip_Back", "Bob_And_Weave_Combo", "Pivot_Evade",
                "Crouch_Feint", "Advance_Weave", "Retreat_Weave", "Low_Sway_Recovery", "Reset_To_Guard"],
    "Berserk": ["Calm_Combat_Stance", "Berserk_Activation", "Roar_Pose", "Power_Surge_Breath", "Feral_Guard", "Lunge_Start", "Heavy_Step_In",
                "Wild_Swing_Windup", "Grounding_Recover", "Berserk_Idle_Loop"],
    "Hit": ["Guard_Impact", "Light_Hit_Reaction", "Heavy_Body_Hit", "Head_Hit_Snapback", "Stagger_Backward", "Stagger_Forward", "Knockback",
            "Launch_Reaction", "Airborne_Tumble", "Falling_Reaction", "Hard_Landing", "Knockdown_Crumple", "Get_Up_Recovery", "Dazed_Stunned",
            "Braced_Recovery"],
}
keydir, out = sys.argv[1], sys.argv[2]
groups = [sys.argv[sys.argv.index("--group") + 1]] if "--group" in sys.argv else list(ORDER)
font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 15)
W, H, LAB = 150, 185, 130
for g in groups:
    names = ORDER[g]
    per = 6
    for part in range(0, len(names), per):
        chunk = names[part:part + per]
        cols = max(len(glob.glob(f"{keydir}/{n}_[0-9][0-9].png")) for n in chunk)
        im = Image.new("RGB", (LAB + cols * W, len(chunk) * H), (249, 236, 240))
        d = ImageDraw.Draw(im)
        for r, n in enumerate(chunk):
            d.text((6, r * H + 8), n.replace("_", " "), fill=(60, 30, 50), font=font)
            for c, f in enumerate(sorted(glob.glob(f"{keydir}/{n}_[0-9][0-9].png"))):
                t = Image.open(f).convert("RGB").resize((W, int(W * 340 / 280)))
                im.paste(t, (LAB + c * W, r * H - 12))
                d.text((LAB + c * W + 4, r * H + H - 20), str(c + 1), fill=(90, 60, 80), font=font)
        im.save(f"{out}_{g}_{part // per + 1}.png")
        print(f"{out}_{g}_{part // per + 1}.png", im.size)
