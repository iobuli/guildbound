"""Workroom: the shared desk of Kev Studio, so several agents (and the human) work on one focused task together.

- Who: every request names itself (`X-Client` header, `?as=`, or `from` / `who` in the JSON). The human's page is `user`.
  `presence()` lists who spoke recently, so nobody drives blind.
- Board: a focus line and tasks, kept as plain files in `board/` so they travel through git with the code:
  `board/focus.json`, one file per task in `board/tasks/<id>.json` (so two agents rarely edit the same file), and an
  append-only `board/notes.jsonl` (git `merge=union`). Claim a task, add notes, mark it `doing` / `review` / `done`.
- Lease: one hand on the rig at a time. A write to the rig (set, reach, ref play, ...) from an agent needs the lease
  (`take`, 120 s, renewed by each write); the human always may and can take it back. Reads are never blocked.
- Token: with `--token T` every /api call needs `X-Token: T` (or `?token=T`); needed before the server is reachable from anywhere
  but this machine (a tunnel).
"""
import json
import os
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
BOARD = os.path.join(HERE, "board")
HUMANS = {"user", "ui", "console"}
LEASE_SECS = 120
STATUSES = ("todo", "doing", "review", "done", "blocked")
# Console verbs that only read: never gated by the lease.
READ_VERBS = {"help", "models", "clips", "bones", "get", "landmarks", "facings", "balance", "rig", "state"}
READ_SUBS = {"ref": {"status", "list"}, "cat": {"list", "info"}}

LOCK = threading.Lock()
SEEN = {}  # who -> {"at": t, "did": last thing}
LEASE = {"holder": None, "until": 0.0}


def _now():
    return round(time.time(), 1)


def who_of(handler, body=None):
    import urllib.parse
    q = urllib.parse.parse_qs(urllib.parse.urlparse(handler.path).query)
    name = handler.headers.get("X-Client") or (q.get("as") or [None])[0] or (body or {}).get("who") or (body or {}).get("from") or "anon"
    return str(name).lower()[:24]


def touch(who, did):
    with LOCK:
        SEEN[who] = {"at": _now(), "did": did[:80]}


def presence():
    t = _now()
    with LOCK:
        return {w: {"idle_s": round(t - v["at"], 1), "did": v["did"]} for w, v in SEEN.items() if t - v["at"] < 600}


def lease_info():
    with LOCK:
        if LEASE["holder"] and LEASE["until"] < _now():
            LEASE["holder"] = None
        return {"holder": LEASE["holder"], "left_s": max(0.0, round(LEASE["until"] - _now(), 1)) if LEASE["holder"] else 0.0}


def gate(who, line=None):
    """(ok, message) for a write to the rig by `who`; `line` is the console line when there is one."""
    if who in HUMANS:
        return True, ""
    if line is not None:
        t = line.strip().split()
        verb = t[0].lower() if t else ""
        if verb in READ_VERBS or (verb in READ_SUBS and len(t) > 1 and t[1].lower() in READ_SUBS[verb]) or (verb in READ_SUBS and len(t) == 1):
            return True, ""
    with LOCK:
        now = _now()
        h = LEASE["holder"] if LEASE["until"] >= now else None
        if h in (None, who):
            LEASE.update({"holder": who, "until": now + LEASE_SECS})  # a write by a free-lease agent takes and renews it
            return True, ""
        return False, f"the rig is held by {h} for {round(LEASE['until'] - now)} s more: wait, or ask them to `release` (POST /api/board {{action:'release_lease'}})"


def _task_path(tid):
    return os.path.join(BOARD, "tasks", f"{tid}.json")


def tasks():
    os.makedirs(os.path.join(BOARD, "tasks"), exist_ok=True)
    out = []
    for f in sorted(os.listdir(os.path.join(BOARD, "tasks"))):
        if f.endswith(".json"):
            with open(os.path.join(BOARD, "tasks", f)) as fh:
                out.append(json.load(fh))
    return out


def save_task(t):
    t["updated"] = _now()
    tmp = _task_path(t["id"]) + ".tmp"
    with open(tmp, "w") as f:
        json.dump(t, f, indent=1, sort_keys=True)
        f.write("\n")
    os.replace(tmp, _task_path(t["id"]))


def note(who, text, task=None):
    os.makedirs(BOARD, exist_ok=True)
    with LOCK, open(os.path.join(BOARD, "notes.jsonl"), "a") as f:
        f.write(json.dumps({"at": _now(), "who": who, "task": task, "text": text[:2000]}) + "\n")


def notes(limit=30):
    p = os.path.join(BOARD, "notes.jsonl")
    if not os.path.exists(p):
        return []
    with open(p) as f:
        rows = [json.loads(line) for line in f if line.strip()]
    return rows[-limit:]


def focus():
    p = os.path.join(BOARD, "focus.json")
    if os.path.exists(p):
        with open(p) as f:
            return json.load(f)
    return {"focus": "", "rules": []}


def board():
    return {"focus": focus(), "tasks": tasks(), "notes": notes(), "presence": presence(), "lease": lease_info()}


def act(who, body):
    """One board action. Returns (code, payload)."""
    a = body.get("action")
    touch(who, f"board:{a}")
    if a == "add":
        ids = [int(t["id"][1:]) for t in tasks() if t["id"][1:].isdigit()] or [0]
        t = {"id": f"T{max(ids) + 1:03d}", "title": body.get("title", "")[:160], "detail": body.get("detail", ""), "owner": body.get("owner"),
             "status": "todo", "by": who, "created": _now(), "done_when": body.get("done_when", "")}
        save_task(t)
        note(who, f"added {t['id']}: {t['title']}", t["id"])
        return 200, t
    if a in ("claim", "status", "assign", "result"):
        p = _task_path(body.get("id", ""))
        if not os.path.exists(p):
            return 404, {"error": "no such task"}
        with open(p) as f:
            t = json.load(f)
        if a == "claim":
            if t.get("owner") not in (None, who) and t.get("status") == "doing":
                return 409, {"error": f"{t['id']} is being done by {t['owner']}"}
            t.update({"owner": who, "status": "doing"})
        elif a == "assign":
            t["owner"] = body.get("owner")
        elif a == "status":
            if body.get("status") not in STATUSES:
                return 400, {"error": f"status is one of {STATUSES}"}
            t["status"] = body["status"]
        elif a == "result":
            t["result"] = body.get("result", "")[:4000]
        save_task(t)
        note(who, f"{t['id']} {a}: {body.get('status') or body.get('owner') or ''}".strip(), t["id"])
        return 200, t
    if a == "note":
        note(who, body.get("text", ""), body.get("task"))
        return 200, {"ok": True}
    if a == "focus":
        if who not in HUMANS:
            return 403, {"error": "only the human sets the focus"}
        os.makedirs(BOARD, exist_ok=True)
        f = focus()
        f["focus"] = body.get("text", "")
        with open(os.path.join(BOARD, "focus.json"), "w") as fh:
            json.dump(f, fh, indent=1)
        return 200, f
    if a == "take_lease":
        with LOCK:
            now = _now()
            h = LEASE["holder"] if LEASE["until"] >= now else None
            if h not in (None, who) and who not in HUMANS:
                return 409, {"error": f"held by {h}"}
            LEASE.update({"holder": who, "until": now + LEASE_SECS})
        return 200, lease_info()
    if a == "release_lease":
        with LOCK:
            if LEASE["holder"] in (who, None) or who in HUMANS:
                LEASE.update({"holder": None, "until": 0.0})
        return 200, lease_info()
    return 400, {"error": "action: add | claim | assign | status | result | note | focus | take_lease | release_lease"}
