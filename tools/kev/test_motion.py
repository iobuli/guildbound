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


if __name__ == "__main__":
    unittest.main()
