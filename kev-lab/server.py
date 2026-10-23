"""kev-lab server: python3 kev-lab/server.py [--port 8765] [--live]

Serves the web page and a small JSON API over what kev computes. Standard library only (plus numpy and pandas-free run.py).
--live: every 10 minutes, append newly closed SOL hours from Binance, re-run kev's signals and the paper book.
"""
from __future__ import annotations

import argparse
import json
import sys
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parent))
import data  # noqa: E402
import run  # noqa: E402

WEB = Path(__file__).resolve().parent / "web"
RESULTS = data.ROOT / "out" / "lab" / "results.json"
TRACE = data.ROOT / "out" / "lab" / "trace.json"
M1BIN = data.ROOT / "out" / "nucleus" / "SOLUSDT-m1.bin.gz"
STATE = {"live": False, "last_check": None, "last_added": 0, "message": "idle", "busy": False}
LOCK = threading.Lock()


def refresh(live: bool) -> str:
    with LOCK:
        STATE["busy"] = True
        try:
            added = 0
            if live:
                for sym in data.LIVE:
                    added += data.append_live(sym)
                    if added:
                        data.kev("signals", sym)
            if added or not RESULTS.exists() or not live:
                run.main()
            STATE.update(last_check=int(time.time() * 1000), last_added=added, message=f"{added} new hour(s) added" if live else "recomputed")
            return STATE["message"]
        except Exception as e:  # report, keep serving
            STATE["message"] = f"error: {e}"
            return STATE["message"]
        finally:
            STATE["busy"] = False


def live_loop():
    while True:
        refresh(live=True)
        time.sleep(600)


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=str(WEB), **k)

    def _json(self, obj, code=200):
        body = json.dumps(obj, default=float).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        url = urlsplit(self.path)
        if url.path == "/api/gold/nucleus":
            from gold_observatory import gold_report, parse_asof
            try:
                cutoff = parse_asof(parse_qs(url.query).get('asof', [None])[0])
                return self._json(gold_report(cutoff))
            except ValueError as e:
                return self._json({'error': str(e)}, 400)
        if self.path == "/api/results":
            return self._json(json.loads(RESULTS.read_text()) if RESULTS.exists() else {"error": "no results yet; POST /api/recompute"})
        if self.path == "/api/status":
            return self._json(STATE)
        if self.path == "/api/trace":
            return self._json(json.loads(TRACE.read_text())) if TRACE.exists() else self._json({"error": "no trace yet; run python3 kev-lab/trace.py"}, 404)
        if self.path == "/data/SOLUSDT-m1.bin":  # pre-gzipped minute bars for the Nucleus page
            if not M1BIN.exists():
                return self._json({"error": "no data; run kev-lab/nucleus.py export"}, 404)
            body = M1BIN.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Encoding", "gzip")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            return self.wfile.write(body)
        if self.path.startswith("/api/candles/"):
            sym = self.path.rsplit("/", 1)[-1]
            c = data.h1(sym)
            step = max(1, len(c["t"]) // 4000)  # downsample for the chart
            return self._json({k: c[k][::step].tolist() for k in "tohlc"})
        return super().do_GET()

    def do_POST(self):
        if self.path == "/api/recompute":
            return self._json({"message": refresh(live=False)})
        if self.path == "/api/live":
            return self._json({"message": refresh(live=True)})
        self._json({"error": "unknown"}, 404)

    def log_message(self, *a):
        pass


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--live", action="store_true")
    a = ap.parse_args()
    if not RESULTS.exists():
        refresh(live=False)
    if a.live:
        STATE["live"] = True
        threading.Thread(target=live_loop, daemon=True).start()
    print(f"kev-lab on http://localhost:{a.port}  (live paper trading: {'on' if a.live else 'off'})")
    ThreadingHTTPServer(("127.0.0.1", a.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
