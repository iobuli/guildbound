"""Local remote client for the running Guildbound beta (Bevy Remote Protocol, http://127.0.0.1:15702).

python -I Tools/gb.py state                      sim state as JSON
python -I Tools/gb.py feed [SINCE]               feed entries (boss messages) from index SINCE
python -I Tools/gb.py say TEXT...                post to the in-game feed as claude
python -I Tools/gb.py pause | resume | reset
python -I Tools/gb.py step [N]                   step N ticks (pauses first)
python -I Tools/gb.py speed X
python -I Tools/gb.py opponent idle|chase|spar
python -I Tools/gb.py input FIGHTER X Z [TICKS] [run]   hold a move intent for TICKS sim ticks
python -I Tools/gb.py tuning KEY=VAL ...         e.g. walk_speed=2.4 model=drag (or defaults=1)
python -I Tools/gb.py clips                      animation clip names
python -I Tools/gb.py call gb.presentation       active clips, seek times and pose hashes
python -I Tools/gb.py call gb.physics            ragdoll bodies, joints, stability and pose hashes
python -I Tools/gb.py anim CLIP [FIGHTER]        play a clip once on fighter (default 0)
python -I Tools/gb.py library [NAME|INDEX]       list packs, or switch every fighter's model + clips
python -I Tools/gb.py shot [PATH]                screenshot, default Documentation/checks/live/latest.png
python -I Tools/gb.py call METHOD [JSON]         any BRP method, e.g. world.query
python -I Tools/gb.py call gb.possess [JSON]     Possession: {"set":{"B_L_UpperArm":[0,0,20]}, "aim":{bone:[x,y,z]}, "step":1,
                                                 "frame":12, "fps":30, "grip":[1,1], "look":"camera", "lean":true, "breath":true,
                                                 "save":"correction", "pose":"name", "clear":true, "bones":true}

gb.control also accepts state=play|menu, dev_panel=true|false and lock=true|false.
gb.input also accepts jump, roll, light, heavy, heavy_held, guard, step, cast=0..3,
cast_held and ki_charge. Press edges fire once on the script's first tick.
python -I Tools/check_beta.py runs the live controller verification.
python -I Tools/check_combat.py checks live combos, smash and dash animations.
python -I Tools/check_spells.py checks mage fireball/lightning and launch timing.
"""
import json
import sys
import time
import urllib.error
import urllib.request

URL = "http://127.0.0.1:15702"


def call(method, params=None):
    body = {"jsonrpc": "2.0", "id": 1, "method": method}
    if params is not None:
        body["params"] = params
    req = urllib.request.Request(URL, json.dumps(body).encode(), {"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            reply = json.load(r)
    except urllib.error.URLError as e:
        sys.exit(f"game not reachable at {URL} ({e.reason}); start it with: cargo run -p guildbound")
    if "error" in reply:
        sys.exit(f"error: {json.dumps(reply['error'])}")
    return reply.get("result")


def main(argv):
    if not argv:
        sys.exit(__doc__)
    cmd, args = argv[0], argv[1:]
    if cmd == "state":
        out = call("gb.state")
    elif cmd == "feed":
        out = call("gb.feed", {"since": int(args[0]) if args else 0})
    elif cmd == "say":
        out = call("gb.say", {"text": " ".join(args)})
    elif cmd in ("pause", "resume"):
        out = call("gb.control", {"pause": cmd == "pause"})
    elif cmd == "reset":
        out = call("gb.control", {"reset": True})
    elif cmd == "step":
        call("gb.control", {"pause": True})
        out = call("gb.control", {"step": int(args[0]) if args else 1})
    elif cmd == "speed":
        out = call("gb.control", {"speed": float(args[0])})
    elif cmd == "opponent":
        out = call("gb.control", {"opponent": args[0]})
    elif cmd == "input":
        fighter, x, z = int(args[0]), float(args[1]), float(args[2])
        ticks = int(args[3]) if len(args) > 3 and args[3].isdigit() else 60
        out = call("gb.input", {"fighter": fighter, "move": [x, z], "ticks": ticks, "run": "run" in args})
    elif cmd == "tuning":
        pairs = dict(a.split("=", 1) for a in args)
        def num(v):
            try:
                return float(v)
            except ValueError:
                return v
        params = {k: num(v) for k, v in pairs.items() if k != "defaults"}
        if "defaults" in pairs:
            params["defaults"] = pairs["defaults"] not in ("0", "false")
        out = call("gb.tuning", params)
    elif cmd == "clips":
        out = call("gb.clips")
    elif cmd == "library":
        out = call("gb.library", {"select": int(args[0]) if args[0].isdigit() else args[0]} if args else {})
    elif cmd == "anim":
        out = call("gb.anim", {"clip": args[0], "fighter": int(args[1]) if len(args) > 1 else 0})
    elif cmd == "shot":
        out = call("gb.screenshot", {"path": args[0]} if args else {})
        time.sleep(0.5)
    elif cmd == "call":
        out = call(args[0], json.loads(args[1]) if len(args) > 1 else None)
    else:
        sys.exit(__doc__)
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main(sys.argv[1:])
