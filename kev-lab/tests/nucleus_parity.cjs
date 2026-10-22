// node tests/nucleus_parity.cjs <bin.gz> <hourMs,...>: the JS nucleus frames for those hours, as JSON (for test_nucleus.py).
const fs = require('fs'), zlib = require('zlib'), path = require('path');
require(path.join(__dirname, '..', 'web', 'nucleus-core.js'));
const N = globalThis.Nucleus;
const d = N.decode(zlib.gunzipSync(fs.readFileSync(process.argv[2])));
const out = {};
for (const h of process.argv[3].split(',').map(Number)) {
  out[h] = N.frames(d, h).map((f) => ({ t: f.t, bal: f.bal, slopeBal: f.slopeBal, valid: f.valid,
    tf: Object.fromEntries(Object.entries(f.tf).map(([k, v]) => [k, { order: v.order, cloud: v.cloud, bal: v.bal }])),
    lines: f.lines.map((x) => [x.tf, x.n, x.pos, x.slope, x.shift, x.lag, x.cmid, x.cpos]) }));
}
process.stdout.write(JSON.stringify(out, (k, v) => (typeof v === 'number' && !Number.isFinite(v) ? null : v)));
