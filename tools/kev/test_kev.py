"""Tests for Kev: run with `python3 -m unittest discover -s tools/kev`."""
import json
import os
import struct
import tempfile
import unittest

import numpy as np

import kev


def write_glb(path, joints=0, clips=0, vertices=0):
    """A minimal GLB with `joints` on one skin, `clips` animations and one mesh of `vertices` positions."""
    accessors = [{"count": vertices, "type": "VEC3", "componentType": 5126}] if vertices else []
    js = {
        "asset": {"version": "2.0"},
        "nodes": [{"name": f"n{i}"} for i in range(joints)],
        "skins": [{"joints": list(range(joints))}] if joints else [],
        "animations": [{"name": f"clip{i}", "channels": [], "samplers": []} for i in range(clips)],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}] if vertices else [],
        "accessors": accessors,
    }
    text = json.dumps(js, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    out = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(text))
    out += struct.pack("<II", len(text), 0x4E4F534A) + text
    with open(path, "wb") as f:
        f.write(out)


class ReadTests(unittest.TestCase):
    def test_counts_come_from_the_header(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "rig.glb")
            write_glb(p, joints=52, clips=3, vertices=900)
            info = kev.describe(p, "rig.glb")
            self.assertEqual((info["bones"], info["clips"], info["meshes"], info["vertices"]), (52, 3, 1, 900))
            self.assertEqual(info["kind"], "model")

    def test_unrigged_file_has_zero_bones(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "prop.glb")
            write_glb(p, vertices=40)
            self.assertEqual(kev.describe(p, "prop.glb")["bones"], 0)

    def test_archives_are_listed_not_read(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "pack.zip")
            with open(p, "wb") as f:
                f.write(b"PK\x03\x04junk")
            info = kev.describe(p, "pack.zip")
            self.assertEqual(info["kind"], "archive")
            self.assertIsNone(info["bones"])


class ClusterTests(unittest.TestCase):
    def test_kmeans_separates_two_blobs(self):
        rng = np.random.default_rng(0)
        x = np.vstack([rng.normal(0, 0.1, (20, 2)), rng.normal(5, 0.1, (20, 2))])
        labels, _ = kev.kmeans(x, 2, seed=1)
        self.assertEqual(len(set(labels[:20])), 1)
        self.assertEqual(len(set(labels[20:])), 1)
        self.assertNotEqual(labels[0], labels[20])

    def test_silhouette_picks_the_true_number_of_groups(self):
        rng = np.random.default_rng(0)
        x = np.vstack([rng.normal(c, 0.1, (15, 2)) for c in (0, 5, 10)])
        labels, k = kev.choose_clusters(x, kmax=6, seed=1)
        self.assertEqual(k, 3)
        self.assertEqual(len(set(labels)), 3)

    def test_few_assets_make_one_cluster(self):
        labels, k = kev.choose_clusters(np.zeros((2, 5)), kmax=8, seed=1)
        self.assertEqual(k, 1)
        self.assertEqual(list(labels), [0, 0])

    def test_bone_bands(self):
        self.assertEqual(kev.bone_band(0), "unrigged")
        self.assertEqual(kev.bone_band(20), "rigged-small")
        self.assertEqual(kev.bone_band(65), "rigged-medium")
        self.assertEqual(kev.bone_band(120), "rigged-large")


class EndToEndTests(unittest.TestCase):
    def test_run_writes_index_notes_and_symlinks(self):
        with tempfile.TemporaryDirectory() as root, tempfile.TemporaryDirectory() as out:
            os.makedirs(os.path.join(root, "hair"))
            os.makedirs(os.path.join(root, "weapons"))
            for i in range(4):
                write_glb(os.path.join(root, "hair", f"hair_buzzed{i}.glb"), joints=60, vertices=2000 + i)
                write_glb(os.path.join(root, "weapons", f"sword{i}.glb"), vertices=300 + i)
            index = kev.run(root, out, kmax=4, seed=1)
            self.assertEqual(len(index["assets"]), 8)
            self.assertTrue(os.path.exists(os.path.join(out, "kev_index.json")))
            notes = os.listdir(os.path.join(out, "notes"))
            self.assertEqual(len(notes), 8)
            # Each category folder links back to the original, which is still in place.
            for cat in os.listdir(os.path.join(out, "categories")):
                for link in os.listdir(os.path.join(out, "categories", cat)):
                    target = os.path.realpath(os.path.join(out, "categories", cat, link))
                    self.assertTrue(os.path.exists(target))
                    self.assertTrue(target.startswith(os.path.realpath(root)))
            self.assertEqual(len(set(c["name"] for c in index["clusters"])), len(index["clusters"]))


if __name__ == "__main__":
    unittest.main()
