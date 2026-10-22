#!/usr/bin/env bash
# Build the read-only kev explorer as a Node single executable:
#   dist/kev-explorer.exe (Windows x64) and dist/kev-explorer-linux.
# Run from anywhere: bash kev-lab/build-exe.sh
#
# The SEA blob must be built by the same Node version that runs it. That pair
# is pinned to NODE_V (the fuse below is the sentinel postject looks for).
# The host node on PATH does not have to be NODE_V: it only writes the config
# and runs postject. When it is not NODE_V, the pinned binary for this OS is
# downloaded into dist/cache/ and used to build the blob.
set -euo pipefail
cd "$(dirname "$0")/.."
NODE_V=v22.22.0
FUSE=NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2
POSTJECT=postject@1.0.0-alpha.6

command -v node >/dev/null 2>&1 || { echo "need a host node to run postject (Node 18+ is enough). The packed runtime stays $NODE_V." >&2; exit 1; }
for f in out/lab/results.json out/nucleus/SOLUSDT-m1.bin.gz; do [ -f "$f" ] || { echo "missing $f (run kev-lab first)" >&2; exit 1; }; done
mkdir -p dist/cache

# stdout is the binary path; progress goes to stderr so $(...) stays clean
pinned_node() {
  if [ "$(node -v 2>/dev/null || true)" = "$NODE_V" ]; then
    command -v node
    return
  fi
  local os cpu ext dir dest tmp
  case "$(uname -s)" in
    Linux) os=linux; ext=tar.xz ;;
    Darwin) os=darwin; ext=tar.gz ;;
    *) echo "host node is $(node -v), not $NODE_V, and $(uname -s) cannot run a downloaded Node binary. Install node $NODE_V or build on Linux/macOS." >&2; exit 1 ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64) cpu=x64 ;;
    aarch64|arm64) cpu=arm64 ;;
    *) echo "unsupported arch $(uname -m)" >&2; exit 1 ;;
  esac
  dir="node-${NODE_V}-${os}-${cpu}"
  dest="dist/cache/${dir}-node"
  if [ ! -x "$dest" ]; then
    echo "host node is $(node -v); downloading $NODE_V to build the SEA blob" >&2
    tmp=$(mktemp -d)
    curl -fL --retry 3 -o "$tmp/node.archive" "https://nodejs.org/dist/${NODE_V}/${dir}.${ext}"
    if [ "$ext" = tar.xz ]; then tar -xJf "$tmp/node.archive" -C "$tmp"; else tar -xzf "$tmp/node.archive" -C "$tmp"; fi
    mv "$tmp/$dir/bin/node" "$dest"
    chmod +x "$dest"
    rm -rf "$tmp"
  fi
  echo "$dest"
}

PINNED="$(pinned_node)"
grep -q "$FUSE" "$PINNED" || { echo "pinned node $PINNED ($("$PINNED" -v)) has no SEA fuse $FUSE; refusing to build a blob the runtime cannot load" >&2; exit 1; }

# sea-config: every web/* file at build time, results, data, trace if present (paths relative to the repo root)
node -e '
const fs = require("fs"), a = {};
for (const f of fs.readdirSync("kev-lab/web").sort()) if (fs.statSync("kev-lab/web/" + f).isFile()) a["web/" + f] = "kev-lab/web/" + f;
a["results.json"] = "out/lab/results.json"; a["SOLUSDT-m1.bin.gz"] = "out/nucleus/SOLUSDT-m1.bin.gz";
if (fs.existsSync("out/lab/trace.json")) a["trace.json"] = "out/lab/trace.json";
fs.writeFileSync("dist/sea-config.json", JSON.stringify({ main: "kev-lab/explorer/server.cjs", output: "dist/sea-prep.blob",
  disableExperimentalSEAWarning: true, useSnapshot: false, useCodeCache: false, assets: a }, null, 2));
console.log("assets: " + Object.keys(a).join(" "));'
"$PINNED" --experimental-sea-config dist/sea-config.json

# Windows: official node.exe of NODE_V, injected (postject skips signature; the exe is unsigned)
if [ ! -f dist/cache/node.exe ]; then
  curl -fL --retry 3 -o dist/cache/node.exe.part "https://nodejs.org/dist/$NODE_V/win-x64/node.exe" && mv dist/cache/node.exe.part dist/cache/node.exe
fi
inject() { cp "$1" "$2"; chmod u+w "$2"; npx --yes "$POSTJECT" "$2" NODE_SEA_BLOB dist/sea-prep.blob --sentinel-fuse "$FUSE" "${@:3}"; }
inject dist/cache/node.exe dist/kev-explorer.exe

# Linux artifact is always the pinned linux-x64 binary, even when the blob was built by another OS's NODE_V.
if [ ! -x dist/cache/node-linux ]; then
  curl -fL --retry 3 "https://nodejs.org/dist/$NODE_V/node-$NODE_V-linux-x64.tar.xz" | tar -xJ -O "node-$NODE_V-linux-x64/bin/node" > dist/cache/node-linux
  chmod +x dist/cache/node-linux
fi
grep -q "$FUSE" dist/cache/node-linux || { echo "downloaded linux $NODE_V has no SEA fuse" >&2; exit 1; }
inject dist/cache/node-linux dist/kev-explorer-linux
chmod +x dist/kev-explorer-linux
ls -la dist/kev-explorer.exe dist/kev-explorer-linux dist/sea-prep.blob
echo "done: copy dist/kev-explorer.exe to Windows and double-click it (unsigned: SmartScreen may ask 'Run anyway')"
echo "runtime pinned to $NODE_V; host node was $(node -v)"
