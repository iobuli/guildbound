"""On-demand archive reader for Kev Studio. Configuration/cache stay in staging/.

One resident CUE4Parse worker mounts the game once. Requests are serialized and
converted clips are cached as animation-only GLBs sharing Kev's UAL mannequin.
"""
import atexit
import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

import retarget_glb

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / "staging/kev/sparking"
CONFIG = DATA / "config.json"
CLIPS = ROOT / "assets/anim/sparking_cache"
DLL = Path(__file__).parent / "bin/Debug/net10.0/ArchiveProbe.dll"
_catalog = None
_config = None
_lock = threading.Lock()
_worker = None
_lines = None
_log = None
_reader = None


def catalog():
    global _catalog, _config
    if _catalog is not None:
        return _catalog
    if not CONFIG.exists():
        raise ValueError("Sparking archive library has not been configured")
    _config = json.loads(CONFIG.read_text(encoding="utf-8-sig"))
    index = json.loads(Path(_config["index"]).read_text(encoding="utf-8-sig"))
    clips = []
    for path in index["files"]:
        name = path.rsplit("/", 1)[-1]
        if not name.startswith("A_") or not name.endswith(".uasset"):
            continue
        parts = path.split("/")
        character = parts[parts.index("Characters") + 1] if "Characters" in parts else "shared"
        clip_id = hashlib.sha256(path.encode()).hexdigest()[:24]
        clips.append({"id": clip_id, "name": name[:-7], "character": character, "source": path})
    _catalog = {"clips": clips, "characters": sorted({c["character"] for c in clips}),
                "total": len(clips), "mode": "on-demand"}
    return _catalog


def _stop():
    global _worker, _log, _reader
    if _worker is not None and _worker.poll() is None:
        _worker.terminate()
        try:
            _worker.wait(timeout=5)
        except subprocess.TimeoutExpired:
            _worker.kill()
            _worker.wait()
    if _reader is not None:
        _reader.join(timeout=5)
        _reader = None
    if _worker is not None:
        _worker.stdin.close()
        _worker.stdout.close()
    _worker = None
    if _log is not None:
        _log.close()
        _log = None


atexit.register(_stop)


def _start():
    global _worker, _lines, _log, _reader
    if _worker is not None and _worker.poll() is None:
        return
    _stop()
    DATA.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update(DOTNET_gcServer="0", KEV_UNREAL_AES_KEY=_config["key"],
                       KEV_UNREAL_MAPPINGS=_config["mappings"], DOTNET_PROCESSOR_COUNT="2")
    _log = (DATA / "worker.log").open("a", encoding="utf-8")
    _worker = subprocess.Popen(["dotnet", str(DLL), _config["archives"], "unused", "--worker"],
                               cwd=str(ROOT), env=environment, stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=_log, text=True, encoding="utf-8",
                               creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    _lines = queue.Queue()
    process, lines, log = _worker, _lines, _log
    def read():
        for line in process.stdout:
            if line.startswith("KEV_"):
                lines.put(line.strip())
            else:
                log.write(line)
                log.flush()
        lines.put("KEV_EXIT")
    _reader = threading.Thread(target=read, daemon=True)
    _reader.start()
    try:
        ready = _lines.get(timeout=60)
    except queue.Empty:
        _stop()
        raise RuntimeError("archive reader startup timed out")
    if ready != "KEV_READY":
        _stop()
        raise RuntimeError("archive reader failed; inspect staging/kev/sparking/worker.log")


def export(clip_id):
    clips = catalog()["clips"]
    item = next((c for c in clips if c["id"] == clip_id), None)
    if item is None:
        raise ValueError("unknown archive clip")
    started = time.perf_counter()
    with _lock:
        CLIPS.mkdir(parents=True, exist_ok=True)
        output = CLIPS / ("v3_" + clip_id + ".glb")
        cached = output.exists()
        if not cached:
            decoded = DATA / (clip_id + ".json")
            try:
                _start()
                _worker.stdin.write(json.dumps({"source": item["source"], "output": str(decoded)}) + "\n")
                _worker.stdin.flush()
                response = _lines.get(timeout=120)
                if not response.startswith("KEV_RESULT "):
                    raise RuntimeError("archive reader stopped during decode")
                result = json.loads(response[len("KEV_RESULT "):])
                if not result.get("ok"):
                    raise RuntimeError(result.get("error", "decode failed"))
                partial = output.with_suffix(".partial")
                if partial.exists():
                    partial.unlink()  # incomplete generated cache from an interrupted conversion
                reports = retarget_glb.convert(ROOT / "assets/anim/ual2.glb", partial, [decoded], animation_only=True)
                partial.replace(output)
                # Only completed GLBs are visible to the client. Keep a provenance sidecar.
                output.with_suffix(".json").write_text(json.dumps({**item, "reports": reports}), encoding="utf-8")
            except Exception as error:
                with (DATA / "failures.jsonl").open("a", encoding="utf-8") as log:
                    log.write(json.dumps({**item, "error": str(error), "time": time.time()}) + "\n")
                _stop()
                raise
        result = {**item, "url": "/assets/anim/sparking_cache/" + output.name,
                  "base": "ual2.glb", "cached": cached,
                  "seconds": round(time.perf_counter() - started, 3), "bytes": output.stat().st_size}
        with (DATA / "access.jsonl").open("a", encoding="utf-8") as log:
            log.write(json.dumps(result) + "\n")
        return result
