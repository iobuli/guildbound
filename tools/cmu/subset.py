"""Pick the CMU mocap clips the game uses and write them, downsampled, to assets/anim/cmu/.

Source: the cgspeed "Motionbuilder-friendly" BVH conversion of the CMU Graphics Lab Motion
Capture Database (http://mocap.cs.cmu.edu/; mirrored at github.com/Shriinivas/cmubvh). CMU places
no restrictions on the data and the conversion adds none (see LICENSES.md). Frame one of every
take is a T-pose, which `motion::bvh` uses as the bind pose.

Usage: python -I tools/cmu/subset.py <dir with NN_MM.bvh files> [--fps 30]
Writes assets/anim/cmu/<id>_<name>.bvh (frame one kept, then every step-th frame, 2 decimals).
"""
import os
import re
import sys

CLIPS = {
    # Karate kata and drills (subject 135).
    "135_01": "bassai", "135_02": "empi", "135_03": "empi_b", "135_04": "front_kick",
    "135_05": "gedanbarai", "135_06": "heiansyodan", "135_09": "oiduki",
    # Martial arts session (subject 144): kicks, blocks, punches, spinning reaches.
    "144_05": "front_kicking", "144_06": "front_kicking_b", "144_07": "left_blocks",
    "144_08": "left_blocks_b", "144_09": "left_front_kicking", "144_10": "left_front_kicking_b",
    "144_13": "left_punch_sequence", "144_14": "left_punch_sequence_b", "144_15": "left_spin_reach",
    "144_16": "left_spin_reach_b", "144_20": "punch_sequence", "144_21": "punch_sequence_b",
    "144_26": "right_blocks", "144_27": "right_blocks_b", "144_28": "right_spin_reach",
    "144_29": "right_spin_reach_b",
    # Boxing and strikes.
    "13_17": "boxing", "13_18": "boxing_b", "14_01": "boxing_c", "14_02": "boxing_d", "14_03": "boxing_e",
    "02_05": "punch_strike", "111_19": "punch_kick", "113_13": "punch_and_kick", "141_14": "punch_and_kick_b",
    "143_23": "punching", "143_24": "kicking",
    # Swordplay.
    "02_07": "swordplay", "02_08": "swordplay_b", "02_09": "swordplay_c",
    # Footwork: cuts, spins, defensive shuffles (subject 78, cleaned).
    "78_01": "left_tight_turn", "78_04": "super_tight_left", "78_05": "super_tight_right",
    "78_10": "running_tighter_right", "78_13": "offensive_spin_left", "78_14": "offensive_spin_left_b",
    "78_15": "offensive_go_right", "78_16": "offensive_go_left", "78_17": "offensive_spin_right",
    "78_24": "defensive_straight", "78_25": "defensive_straight_stop", "78_26": "defensive_right_stop",
    "78_27": "defensive_left_stop", "78_28": "defensive_zigzag", "78_29": "defensive_side_to_side",
    "78_30": "defensive_side_to_side_b", "78_35": "right_tight_turn", "139_09": "ducking",
    # Getting up.
    "111_06": "get_up_floor", "111_07": "get_up_floor_b", "111_08": "get_up_floor_c",
    "139_16": "get_up_ground", "139_17": "get_up_ground_b", "139_18": "get_up_back",
    "140_01": "get_up_face_down", "140_02": "get_up_face_down_b", "140_03": "get_up_side",
    "140_04": "get_up_side_b", "140_08": "get_up_back_b", "140_09": "get_up_back_c",
}


def downsample(text, fps):
    head, motion = text.split("MOTION", 1)
    lines = [l for l in motion.strip().splitlines() if l.strip()]
    frames = int(lines[0].split()[1])
    frame_time = float(lines[1].split(":")[1])
    rows = lines[2:2 + frames]
    step = max(1, round(1.0 / (frame_time * fps)))
    picked = rows[::step]
    fmt = lambda row: " ".join(f"{float(v):.2f}" for v in row.split())
    out = [head.rstrip(), "MOTION", f"Frames: {len(picked)}", f"Frame Time: {frame_time * step:.7f}"]
    out += [fmt(r) for r in picked]
    return "\n".join(out) + "\n"


def main():
    src = sys.argv[1]
    fps = float(sys.argv[sys.argv.index("--fps") + 1]) if "--fps" in sys.argv else 30.0
    dst = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "anim", "cmu")
    os.makedirs(dst, exist_ok=True)
    total = 0
    for cid, name in CLIPS.items():
        path = os.path.join(src, f"{cid}.bvh")
        if not os.path.exists(path):
            print("missing", cid)
            continue
        if not re.fullmatch(r"[a-z0-9_]+", name):
            raise SystemExit(f"bad clip name {name}")
        out = downsample(open(path).read(), fps)
        target = os.path.join(dst, f"{cid}_{name}.bvh")
        open(target, "w").write(out)
        total += len(out)
    print(f"{len(CLIPS)} clips, {total / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
