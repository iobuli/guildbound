// kev explorer: read-only server for the kev-lab pages, packed as a Node single executable (built-ins only).
// dev: node kev-lab/explorer/server.cjs [--port N] [--no-open]   build: bash kev-lab/build-exe.sh
'use strict';
const http = require('node:http'), fs = require('node:fs'), path = require('node:path'), { spawn } = require('node:child_process');

let sea = null;
try { sea = require('node:sea'); if (!sea.isSea()) sea = null; } catch { sea = null; }
const ROOT = path.resolve(__dirname, '..', '..');  // dev mode: repo root
const DISK = { 'results.json': 'out/lab/results.json', 'trace.json': 'out/lab/trace.json', 'SOLUSDT-m1.bin.gz': 'out/nucleus/SOLUSDT-m1.bin.gz' };

// asset by name -> Buffer or null; packed assets in the exe, files on disk otherwise
function asset(name) {
  try {
    if (sea) return Buffer.from(sea.getAsset(name));
    const p = name.startsWith('web/') ? path.join(ROOT, 'kev-lab', name) : path.join(ROOT, DISK[name] || '\0');
    return fs.readFileSync(p);
  } catch { return null; }
}

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.ico': 'image/x-icon' };
const STATUS = { readonly: true, live: false, busy: false, last_check: null, last_added: 0, message: 'read-only explorer' };

function send(res, code, body, head = {}) {
  res.writeHead(code, { 'Content-Length': body.length, 'Cache-Control': 'no-cache', ...head });
  res.end(body);
}
const json = (res, obj, code = 200) => send(res, code, Buffer.from(JSON.stringify(obj)), { 'Content-Type': 'application/json' });

function handle(req, res) {
  const url = decodeURIComponent((req.url || '/').split('?')[0]);
  if (req.method !== 'GET' && req.method !== 'HEAD') return json(res, { error: 'read-only explorer' }, 403);
  if (url === '/api/status') return json(res, STATUS);
  if (url === '/api/results' || url === '/api/trace') {
    const b = asset(url === '/api/results' ? 'results.json' : 'trace.json');
    return b ? send(res, 200, b, { 'Content-Type': 'application/json' }) : json(res, { error: `no ${url.slice(5)} in this build` }, 404);
  }
  if (url === '/data/SOLUSDT-m1.bin') {  // pre-gzipped minute bars, the browser inflates them
    const b = asset('SOLUSDT-m1.bin.gz');
    return b ? send(res, 200, b, { 'Content-Type': 'application/octet-stream', 'Content-Encoding': 'gzip' }) : json(res, { error: 'no data in this build' }, 404);
  }
  if (url.startsWith('/api/')) return json(res, { error: 'not available in the read-only explorer' }, 404);
  const file = url === '/' ? 'index.html' : url.slice(1);
  if (file.includes('..') || file.includes('/') || file.includes('\\')) return json(res, { error: 'not found' }, 404);  // web/ is flat
  const b = asset('web/' + file);
  if (!b) return json(res, { error: 'not found' }, 404);
  send(res, 200, b, { 'Content-Type': TYPES[path.extname(file)] || 'application/octet-stream' });
}

function openBrowser(url) {
  const [cmd, args] = process.platform === 'win32' ? ['cmd', ['/c', 'start', '', url]] : process.platform === 'darwin' ? ['open', [url]] : ['xdg-open', [url]];
  try { const p = spawn(cmd, args, { stdio: 'ignore', detached: true, windowsHide: true }); p.on('error', () => {}); p.unref(); } catch {}
}

function main() {
  const argv = process.argv.slice(sea ? 1 : 2), i = argv.indexOf('--port');
  const port0 = i >= 0 ? +argv[i + 1] : 8765, noOpen = argv.includes('--no-open');
  const listen = (port, tries) => {
    const srv = http.createServer((req, res) => { try { handle(req, res); } catch (e) { json(res, { error: String(e) }, 500); } });
    srv.once('error', (e) => {
      if (e.code === 'EADDRINUSE' && tries > 0) return listen(port + 1, tries - 1);  // taken: try the next port
      console.error(`kev explorer could not start: ${e.message}`); process.exit(1);
    });
    srv.listen(port, '127.0.0.1', () => {
      const url = `http://localhost:${port}/`;
      console.log(`kev explorer running at ${url}\n(${sea ? 'packed' : 'dev'} mode, read-only)  close this window to stop.`);
      if (!noOpen) openBrowser(url);
    });
  };
  listen(port0, 20);
}

main();
