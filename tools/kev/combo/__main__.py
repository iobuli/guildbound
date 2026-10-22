"""python3 -I tools/kev/combo/__main__.py bake [name|all] | list | check

`python3 -I` does not put the working directory on the path, so this file
inserts tools/kev itself when it is launched as a script.
"""
import os
import sys

if __package__ in (None, ""):
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    __package__ = "combo"

from .bake import bake_all, bake_combo, studio
from .compile import list_combos


def main(argv):
    cmd = argv[1] if len(argv) > 1 else "bake"
    if cmd == "list":
        for n in list_combos():
            print(n)
        return 0
    srv = studio()
    if cmd == "check":
        rows = bake_all(srv, render=True)
        return 0 if all(r["ok"] for r in rows) else 1
    if cmd == "bake":
        which = argv[2] if len(argv) > 2 else "all"
        if which == "all":
            rows = bake_all(srv, render=True)
            return 0 if all(r["ok"] for r in rows) else 1
        row = bake_combo(srv, which, render=True)
        print(row["report"])
        return 0 if row["report"]["ok"] else 1
    print("combo list | bake [name|all] | check")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
