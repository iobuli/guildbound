#!/usr/bin/env bash
# Build the read-only kev explorer as a Node single executable: dist/kev-explorer.exe (Windows x64) + dist/kev-explorer-linux (local test).
# Run from the repo root: bash kev-lab/build-exe.sh   (idempotent; node.exe is downloaded once into dist/cache/)
set -euo pipefail
cd "$(dirname "$0")/.."
NODE_V=v22.22.0 FUSE=NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2 POSTJECT=postject@1.0.0-alpha.6
[ "$(node -v)" = "$NODE_V" ] || { echo "need node $NODE_V (blob format must match the runtime), have $(node -v)" >&2; exit 1; }
for f in out/lab/results.json out/nucleus/SOLUSDT-m1.bin.gz; do [ -f "$f" ] || { echo "missing $f (run kev-lab first)" >&2; exit 1; }; done
mkdir -p dist/cache

# sea-config: every web/* file at build time, results, data, trace if present (paths relative to the repo root)
node -e '
const fs = require("fs"), a = {};
for (const f of fs.readdirSync("kev-lab/web").sort()) if (fs.statSync("kev-lab/web/" + f).isFile()) a["web/" + f] = "kev-lab/web/" + f;
a["results.json"] = "out/lab/results.json"; a["SOLUSDT-m1.bin.gz"] = "out/nucleus/SOLUSDT-m1.bin.gz";
if (fs.existsSync("out/lab/trace.json")) a["trace.json"] = "out/lab/trace.json";
fs.writeFileSync("dist/sea-config.json", JSON.stringify({ main: "kev-lab/explorer/server.cjs", output: "dist/sea-prep.blob",
  disableExperimentalSEAWarning: true, useSnapshot: false, useCodeCache: false, assets: a }, null, 2));
console.log("assets: " + Object.keys(a).join(" "));'
node --experimental-sea-config dist/sea-config.json

# Windows: official node.exe of the same version, injected (postject skips signature; the exe is unsigned)
if [ ! -f dist/cache/node.exe ]; then
  curl -fL --retry 3 -o dist/cache/node.exe.part "https://nodejs.org/dist/$NODE_V/win-x64/node.exe" && mv dist/cache/node.exe.part dist/cache/node.exe
fi
inject() { cp "$1" "$2"; chmod u+w "$2"; npx --yes "$POSTJECT" "$2" NODE_SEA_BLOB dist/sea-prep.blob --sentinel-fuse "$FUSE" "${@:3}"; }
inject dist/cache/node.exe dist/kev-explorer.exe

# Linux test build: the local node if it carries the fuse, else the official linux-x64 binary
LNODE="$(command -v node)"
if ! grep -q "$FUSE" "$LNODE"; then
  [ -f dist/cache/node-linux ] || { curl -fL --retry 3 "https://nodejs.org/dist/$NODE_V/node-$NODE_V-linux-x64.tar.xz" | tar -xJ -O "node-$NODE_V-linux-x64/bin/node" > dist/cache/node-linux; chmod +x dist/cache/node-linux; }
  LNODE=dist/cache/node-linux
fi
inject "$LNODE" dist/kev-explorer-linux
chmod +x dist/kev-explorer-linux
ls -la dist/kev-explorer.exe dist/kev-explorer-linux dist/sea-prep.blob
echo "done: copy dist/kev-explorer.exe to Windows and double-click it (unsigned: SmartScreen may ask 'Run anyway')"
