"""Kev motion: bones are found by role, so the same features run on every rig family.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import unittest

import motion

LIMBS = ["upperarm", "lowerarm", "hand", "thigh", "calf", "foot"]


def rig(names, hips):
    """Node names plus a parent map where every thigh hangs off `hips`."""
    parent = {i: names.index(hips) for i, n in enumerate(names) if "thigh" in n.lower() or "upleg" in n.lower()}
    return names, parent


class BoneMapTests(unittest.TestCase):
    def check(self, names, hips):
        names, parent = rig(names, hips)
        m = motion.bone_map(names, parent)
        self.assertEqual(m["pelvis"], names.index(hips))
        self.assertEqual(set(m), set(motion.CANON) | {"pelvis"})
        return names, m

    def test_unreal(self):
        names = ["root", "pelvis"] + [f"{b}_{s}" for s in "lr" for b in LIMBS]
        self.check(names, "pelvis")

    def test_rigify_takes_the_first_segment(self):
        names = ["DEF-spine"]
        for s in "LR":
            names += [f"DEF-upper_arm.{s}", f"DEF-upper_arm.{s}.001", f"DEF-forearm.{s}", f"DEF-forearm.{s}.001", f"DEF-hand.{s}",
                      f"DEF-thigh.{s}", f"DEF-thigh.{s}.001", f"DEF-shin.{s}", f"DEF-shin.{s}.001", f"DEF-foot.{s}"]
        names, m = self.check(names, "DEF-spine")
        self.assertEqual(names[m["upperarm_l"]], "DEF-upper_arm.L")

    def test_auto_rig_pro(self):
        names = ["c_traj", "root.x", "spine_01.x"]
        for s in "lr":
            names += [f"shoulder.{s}", f"arm_stretch.{s}", f"arm_twist.{s}", f"Shoulder_S_Attachment.{s.upper()}",
                      f"forearm_stretch.{s}", f"hand.{s}", f"thigh_stretch.{s}", f"leg_stretch.{s}", f"foot.{s}"]
        names, m = self.check(names, "root.x")
        self.assertEqual(names[m["upperarm_r"]], "arm_stretch.r")

    def test_a_rig_without_legs_is_refused(self):
        with self.assertRaises(KeyError):
            motion.bone_map(["pelvis", "upperarm_l", "lowerarm_l", "hand_l"], {})



def row(hand_speed, extend, level, foot_lift, travel, returns, reach=1.0, gait=0.0, burst=1.0):
    return {"hand_speed": hand_speed, "extend": extend, "level": level, "foot_lift": foot_lift,
            "travel": travel, "returns": returns, "reach": reach, "gait": gait, "burst": burst}


class LabelTests(unittest.TestCase):
    """Feature rows measured on two rigs (UAL ActionForge clips and the Mech_00 Auto-Rig Pro clips)."""

    def test_thrusts_and_swings_are_strikes(self):
        self.assertEqual(motion.name_cluster(row(5.96, 0.534, 0.0, 0.001, 0.234, 1)), "strike")    # punch_jab
        self.assertEqual(motion.name_cluster(row(43.96, 0.532, -0.244, 0.14, 1.704, 0)), "strike")  # melee_hook
        self.assertEqual(motion.name_cluster(row(29.62, 0.492, 0.0, 0.159, 0.693, 1)), "strike")    # Mech_Attack2
        self.assertEqual(motion.name_cluster(row(47.34, 0.311, -0.361, 0.066, 2.396, 0)), "strike")  # lunge

    def test_deaths_landings_and_runs_are_not(self):
        self.assertEqual(motion.name_cluster(row(10.84, 0.437, -0.982, 0.423, 0.585, 0)), "fall-or-rise")  # death01
        self.assertEqual(motion.name_cluster(row(6.34, 0.501, 1.0, 0.137, 0.164, 0)), "fall-or-rise")      # laytoidle
        self.assertEqual(motion.name_cluster(row(15.55, 0.279, -0.666, 0.36, 5.452, 0)), "fall-or-rise")   # Mech_Die
        self.assertEqual(motion.name_cluster(row(4.58, 0.0, -0.06, 0.075, 0.173, 0)), "transition")        # jump_land
        self.assertEqual(motion.name_cluster(row(17.5, 0.407, 0.055, 0.908, 0.711, 1, gait=0.779)), "jump-or-kick")  # Mech_Run (bounds)
        self.assertEqual(motion.name_cluster(row(8.28, 0.453, 0.0, 0.002, 0.212, 1)), "gesture")           # Mech_Shoot

    def test_walks_in_place_and_push_backs(self):
        self.assertEqual(motion.name_cluster(row(1.75, 0.174, 0.002, 0.223, 0.144, 1, gait=-0.772)), "locomotion")  # Mech_Walk
        self.assertEqual(motion.name_cluster(row(9.0, 0.3, 0.0, 0.9, 9.9, 1, gait=-0.815)), "locomotion")          # sprint_loop
        self.assertEqual(motion.name_cluster(row(12.18, 0.237, 0.0, 0.222, 2.305, 1, burst=1.785)), "transition")  # Mech_GetHit
        self.assertEqual(motion.name_cluster(row(6.32, 0.404, 0.328, 0.121, 0.67, 0)), "transition")             # melee_hook_rec
        self.assertEqual(motion.name_cluster(row(2.29, 0.205, -0.405, 0.081, 0.238, 0)), "fall-or-rise")         # sitting_enter
        self.assertEqual(motion.name_cluster(row(0.3, 0.05, 0.0, 0.001, 0.009, 1)), "idle")                      # idle_loop



class OverrideTests(unittest.TestCase):
    def test_overrides_name_real_labels_and_are_flagged(self):
        for clip, o in motion.OVERRIDES.items():
            self.assertIn(o["label"], motion.LABELS, clip)
            self.assertTrue(o["reason"])
        rows = [{"clip": "Mech_Run", **row(17.5, 0.407, 0.055, 0.908, 0.711, 1, gait=0.779)},
                {"clip": "punch_jab", **row(5.96, 0.534, 0.0, 0.001, 0.234, 1)}]
        for r in rows:
            for f in motion.FEATURES:
                r.setdefault(f, 0.0)
        motion.cluster(rows, 2)
        self.assertEqual((rows[0]["category"], rows[0]["rule_category"], rows[0].get("overridden")), ("locomotion", "jump-or-kick", True))
        self.assertEqual(rows[1]["category"], "strike")
        self.assertNotIn("overridden", rows[1])


if __name__ == "__main__":
    unittest.main()
