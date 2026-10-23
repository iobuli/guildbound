"""Small stdlib client for live Kev Studio control."""
import json
import urllib.error
import urllib.request


class KevClient:
    def __init__(self, base_url="http://127.0.0.1:8765", token="", client="autoanim", timeout=2.0):
        self.base = base_url.rstrip("/")
        self.token = token
        self.client = client
        self.timeout = timeout

    def _request(self, method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        headers = {"X-Client": self.client}
        if self.token:
            headers["X-Token"] = self.token
        if data is not None:
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(self.base + path, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                return json.loads(response.read().decode())
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode(errors="replace")
            raise RuntimeError(f"Kev Studio HTTP {exc.code}: {detail}") from exc

    def state(self):
        return self._request("GET", "/api/state")

    def send(self, frame, inplace=False):
        return self._request("POST", "/api/control",
                             {"landmarks": frame["landmarks"], "inplace": bool(inplace)})

    def command(self, command):
        """Broadcast a viewer command (including the AutoAnim onion overlay)."""
        return self._request("POST", "/api/cmd", command)
