"""Tests for the Kev stage graph: order, cycles, the --only closure and the cache.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import json
import os
import tempfile
import time
import unittest

import graph


class OrderTests(unittest.TestCase):
    def test_dependencies_come_first(self):
        stages = {"c": {"after": ["b"]}, "b": {"after": ["a"]}, "a": {}}
        self.assertEqual(graph.order(stages), ["a", "b", "c"])

    def test_cycles_and_unknown_stages_stop_the_run(self):
        with self.assertRaises(ValueError):
            graph.order({"a": {"after": ["b"]}, "b": {"after": ["a"]}})
        with self.assertRaises(ValueError):
            graph.order({"a": {"after": ["missing"]}})

    def test_only_pulls_in_what_a_stage_needs(self):
        stages = {"a": {}, "b": {"after": ["a"]}, "c": {}}
        self.assertEqual(graph.closure(stages, ["b"]), {"a", "b"})

    def test_the_shipped_pipeline_is_a_valid_graph(self):
        stages = graph.load(os.path.join(os.path.dirname(__file__), "pipeline.json"))["stages"]
        names = graph.order(stages)
        self.assertLess(names.index("strikes"), names.index("h2h"))


class CacheTests(unittest.TestCase):
    def test_unchanged_stages_are_skipped_and_changes_rerun_downstream(self):
        with tempfile.TemporaryDirectory() as root:
            src = os.path.join(root, "in.txt")
            open(src, "w").write("one")
            pipe = {
                "cache": "cache.json",
                "stages": {
                    "a": {"exec": ["a"], "inputs": ["in.txt"], "outputs": ["a.out"]},
                    "b": {"exec": ["b"], "after": ["a"], "outputs": ["b.out"]},
                },
            }
            path = os.path.join(root, "pipeline.json")
            json.dump(pipe, open(path, "w"))
            ran = []

            def runner(cmd):
                ran.append(cmd[0])
                open(os.path.join(root, f"{cmd[0]}.out"), "w").write("x")
                return 0

            self.assertEqual([s for _, s, _ in graph.run(path, root=root, runner=runner)], ["ran", "ran"])
            self.assertEqual([s for _, s, _ in graph.run(path, root=root, runner=runner)], ["cached", "cached"])
            time.sleep(1.1)  # mtime resolution is one second
            open(src, "w").write("two!")
            self.assertEqual([s for _, s, _ in graph.run(path, root=root, runner=runner)], ["ran", "ran"])
            self.assertEqual(ran, ["a", "b", "a", "b"])

    def test_a_failed_stage_stops_the_run(self):
        with tempfile.TemporaryDirectory() as root:
            pipe = {"stages": {"a": {"exec": ["a"]}, "b": {"exec": ["b"], "after": ["a"]}}}
            path = os.path.join(root, "pipeline.json")
            json.dump(pipe, open(path, "w"))
            report = graph.run(path, root=root, runner=lambda cmd: 1)
            self.assertEqual(report[0][1], "failed")
            self.assertEqual(len(report), 1)


if __name__ == "__main__":
    unittest.main()
