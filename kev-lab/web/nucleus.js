// Nucleus page: replay one UTC hour minute by minute through the 18-line nucleus (nucleus-core.js), plus the trace verdict.
const $ = (s) => document.querySelector(s), css = (v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
const f = (x, d = 2) => (Number.isFinite(x) ? x.toFixed(d) : 'n/a'), H = 3600000;
const sgn = (x) => (x === Nucleus.MISSING ? 'n/a' : String(x));
const CLOUD = { 1: 'above', 0: 'in', '-1': 'below', '-9': '–' };
let D = null, F = null, hourT = Date.UTC(2021, 4, 19, 14), T0 = hourT, T1 = hourT, m = 0, timer = null, shown = false, TR = null;

$('#hour').innerHTML = Array.from({ length: 24 }, (_, h) => `<option value="${h}">${String(h).padStart(2, '0')}:00 UTC</option>`).join('');
const clampT = (t) => Math.min(T1, Math.max(T0, Math.floor(t / H) * H));

function setHour(t) {
  hourT = clampT(t); stop(); shown = false;
  $('#date').value = new Date(hourT).toISOString().slice(0, 10); $('#hour').value = new Date(hourT).getUTCHours();
  if (!D) return;
  const t0 = performance.now();
  F = Nucleus.frames(D, hourT);
  if (!F.length) { $('#msg').hidden = false; $('#msg').textContent = 'No minutes in this hour (data gap).'; return; }
  $('#msg').hidden = true; $('#slider').max = F.length - 1;
  console.log(`nucleus ${new Date(hourT).toISOString()} ${F.length} frames in ${Math.round(performance.now() - t0)} ms`);
  show(0);
}
function show(k) {
  if (!F) return;
  m = Math.max(0, Math.min(F.length - 1, k)); $('#slider').value = m;
  const fr = F[m];
  if (m === F.length - 1) shown = true;
  $('#clock').textContent = new Date(fr.t).toISOString().slice(0, 16).replace('T', ' ') + ` · ${(fr.c / 1e4).toFixed(3)}`;
  dots(fr); bars(fr); price(); rest();
}
function dots(fr) {
  const L = {}; fr.lines.forEach((x) => (L[x.tf + '.' + x.n] = x));
  let h = '<span></span>' + Nucleus.NS.map((n) => `<span class="h">n=${n}</span>`).join('') + '<span class="h" style="text-align:left">order · cloud · bal</span>';
  for (const [tf] of Nucleus.TFS) {
    h += `<span class="tf">${tf}</span>`;
    for (const n of Nucleus.NS) {
      const x = L[tf + '.' + n], p = x.pos, ok = Number.isFinite(p);
      const col = p > 0 ? css('--up') : p < 0 ? css('--dn') : css('--dim');
      const ring = ok && p !== 0 && Math.sign(p) === x.slope;
      h += `<div class="dot${ok ? '' : ' nan'}${ring ? ' ring' : ''}" title="${tf} n=${n} pos ${f(p)} closed ${f(x.cmid)} cpos ${f(x.cpos)} slope ${sgn(x.slope)} shift ${sgn(x.shift)} lag ${sgn(x.lag)}" style="${ok ? `background:${col};opacity:${0.15 + 0.85 * Math.min(1, Math.abs(p))}` : ''}"></div>`;
    }
    const s = fr.tf[tf], o = s.order, cls = o === 0 ? 'bull' : o === 5 ? 'bear' : '';
    const oname = o === Nucleus.TIE ? 'tie' : o >= 0 ? Nucleus.ORDERS[o] : 'n/a';
    h += `<div class="meta"><span class="badge ${cls}">${oname}</span><span class="${s.cloud === 1 ? 'good' : s.cloud === -1 ? 'bad' : ''}">${CLOUD[s.cloud]}</span><span>${s.bal > 0 ? '+' : ''}${f(s.bal)}</span></div>`;
  }
  $('#dots').innerHTML = h;
}
function bar(el, v) {
  const ok = Number.isFinite(v), w = ok ? Math.abs(v) * 50 : 0;
  el.innerHTML = `<i style="left:${v >= 0 ? 50 : 50 - w}%;width:${w}%;background:${v >= 0 ? css('--up') : css('--dn')}"></i>`;
}
function bars(fr) {
  bar($('#balbar'), fr.bal); bar($('#slbar'), fr.slopeBal);
  $('#nums').innerHTML = `<span>bal ${f(fr.bal)}</span><span>slope bal ${f(fr.slopeBal)}</span><span>${fr.valid ? 'valid' : 'warming up'}</span><span>minute ${m}</span>`;
}
function axes(cv, lo, hi, PL = 58) {
  const dpr = devicePixelRatio || 1, W = cv.clientWidth, Ht = cv.clientHeight, x = cv.getContext('2d');
  cv.width = W * dpr; cv.height = Ht * dpr; x.setTransform(dpr, 0, 0, dpr, 0, 0);
  const PR = 10, PT = 10, PB = 20, X = (i) => PL + (i / 59) * (W - PL - PR), Y = (v) => PT + (hi - v) / (hi - lo) * (Ht - PT - PB);
  x.font = '11px ' + css('--mono'); x.fillStyle = css('--dim'); x.strokeStyle = css('--hair'); x.lineWidth = 1;
  return { x, X, Y, W, Ht, PL, PR, PT, PB };
}
function price() {
  const cv = $('#px'), n = shown ? F.length : m + 1, L1 = F.map((fr) => fr.lines.filter((l) => l.tf === '1h'));
  const ys = F.slice(0, n).flatMap((fr, i) => [fr.c, ...L1[i].map((l) => l.mid)]).filter(Number.isFinite);
  let lo = Math.min(...ys), hi = Math.max(...ys); if (hi === lo) hi = lo + 1; const pad = (hi - lo) * 0.06; lo -= pad; hi += pad;
  const { x, X, Y, W } = axes(cv, lo, hi);
  for (let g = 0; g <= 4; g++) { const v = lo + (hi - lo) * g / 4; x.beginPath(); x.moveTo(58, Y(v)); x.lineTo(W - 10, Y(v)); x.stroke(); x.fillText((v / 1e4).toFixed(3), 4, Y(v) + 4); }
  [0, 15, 30, 45, 59].forEach((i) => x.fillText(':' + String(i).padStart(2, '0'), X(i) - 8, cv.clientHeight - 5));
  const path = (vals, col, w, k = n) => { x.strokeStyle = col; x.lineWidth = w; x.beginPath(); let on = false; for (let i = 0; i < k; i++) { const v = vals[i]; if (!Number.isFinite(v)) { on = false; continue; } on ? x.lineTo(X(i), Y(v)) : x.moveTo(X(i), Y(v)); on = true; } x.stroke(); };
  [['--acc', 0], ['--warn', 1], ['--dim', 2]].forEach(([c, j]) => path(L1.map((ls) => ls[j].mid), css(c), 1.3));
  path(F.map((fr) => fr.c), css('--ink'), 1.8, Math.min(n, m + 1));
  if (shown && n > m + 1) { x.globalAlpha = 0.35; path(F.map((fr) => fr.c), css('--ink'), 1.4); x.globalAlpha = 1; }
  x.strokeStyle = css('--acc'); x.setLineDash([3, 3]); x.beginPath(); x.moveTo(X(m), 10); x.lineTo(X(m), cv.clientHeight - 20); x.stroke(); x.setLineDash([]);
}
// (d) after the reveal: the hour's close and, per minute, the move still to come from that minute's close
function rest() {
  const cv = $('#rem');
  if (!shown) { cv.hidden = true; $('#revtxt').textContent = 'Play to minute 59 or press Reveal to see where the hour closed.'; return; }
  const cl = F[F.length - 1].c, mv = F.map((fr) => (cl - fr.c) / fr.c * 1e4), here = mv[m];
  $('#revtxt').innerHTML = `Hour closed at <b>${(cl / 1e4).toFixed(3)}</b> (${f((cl - F[0].c) / F[0].c * 100)}% from minute 0). From minute ${m} the rest of the hour moved <b class="${here >= 0 ? 'good' : 'bad'}">${here >= 0 ? '+' : ''}${f(here, 1)} bp</b>. Bars: remaining move from each minute, in bp.`;
  cv.hidden = false;
  const a = Math.max(1, ...mv.map(Math.abs)), { x, X, Y } = axes(cv, -a, a);
  x.fillText('+' + a.toFixed(0), 4, Y(a) + 10); x.fillText('-' + a.toFixed(0), 4, Y(-a)); x.beginPath(); x.moveTo(58, Y(0)); x.lineTo(cv.clientWidth - 10, Y(0)); x.stroke();
  const bw = Math.max(1, (X(1) - X(0)) * 0.7);
  mv.forEach((v, i) => { x.fillStyle = v >= 0 ? css('--up') : css('--dn'); x.globalAlpha = i === m ? 1 : 0.55; x.fillRect(X(i) - bw / 2, Math.min(Y(0), Y(v)), bw, Math.abs(Y(v) - Y(0))); });
  x.globalAlpha = 1;
}
// (e) trace: rho vs minute-of-hour, discovery and check, with the noise band
function rho() {
  if (!TR) return;
  const ft = $('#feat').value, C = TR[$('#tgt').value] || TR.curves, d = C.discovery?.[ft], k = C.check?.[ft], mins = TR.minutes;
  if (!d) return;
  const band = d.band.map((b, i) => Math.max(b, k?.band?.[i] ?? 0)), all = [...d.rho, ...(k?.rho ?? []), ...band, ...band.map((b) => -b)].filter(Number.isFinite);
  let a = Math.max(0.02, ...all.map(Math.abs)) * 1.1;
  const cv = $('#rho'), { x, X, Y, W } = axes(cv, -a, a, 50), Xm = (i) => X((mins[i] / 58) * 59);
  for (let g = 0; g <= 4; g++) { const v = -a + 2 * a * g / 4; x.beginPath(); x.moveTo(50, Y(v)); x.lineTo(W - 10, Y(v)); x.stroke(); x.fillText(v.toFixed(3), 2, Y(v) + 4); }
  [0, 15, 30, 45, 58].forEach((i) => x.fillText(':' + String(i).padStart(2, '0'), X((i / 58) * 59) - 8, cv.clientHeight - 5));
  x.fillStyle = css('--band'); x.beginPath(); band.forEach((b, i) => x.lineTo(Xm(i), Y(b))); for (let i = band.length - 1; i >= 0; i--) x.lineTo(Xm(i), Y(-band[i])); x.fill();
  const path = (ys, col) => { if (!ys) return; x.strokeStyle = col; x.lineWidth = 1.8; x.beginPath(); ys.forEach((v, i) => (i ? x.lineTo(Xm(i), Y(v)) : x.moveTo(Xm(i), Y(v)))); x.stroke(); };
  path(d.rho, css('--acc')); path(k?.rho, css('--warn'));
}
async function trace() {
  try {
    const r = await fetch('/api/trace');
    if (!r.ok) throw new Error('missing');
    TR = await r.json();
    $('#feat').innerHTML = TR.features.map((x) => `<option>${x}</option>`).join('');
    const p = TR.periods, dd = (t) => new Date(t).toISOString().slice(0, 10);
    $('#verdict').innerHTML = `Discovery ${dd(p.discovery[0])}..${dd(p.discovery[1])} (${TR.n_hours.discovery} hours), check ${dd(p.check[0])}..${dd(p.check[1])} (${TR.n_hours.check} hours). ${TR.verdict}`;
    rho();
  } catch { $('#verdict').innerHTML = 'No trace yet: run <code>python3 kev-lab/trace.py</code>, then reload.'; $('#rho').hidden = true; }
}
function stop() { clearInterval(timer); timer = null; $('#play').textContent = '▶ Play'; }
function play() {
  if (timer) return stop();
  if (m >= F.length - 1) { shown = false; show(0); }
  $('#play').textContent = '❚❚ Pause';
  timer = setInterval(() => { if (m >= F.length - 1) return stop(); show(m + 1); }, +$('#speed').value);
}
$('#date').onchange = $('#hour').onchange = () => { const t = Date.parse($('#date').value + 'T00:00:00Z'); if (Number.isFinite(t)) setHour(t + $('#hour').value * H); };
$('#prev').onclick = () => setHour(hourT - H); $('#next').onclick = () => setHour(hourT + H);
$('#rand').onclick = () => setHour(T0 + Math.floor(Math.random() * ((T1 - T0) / H + 1)) * H);
$('#play').onclick = () => F && play();
$('#speed').onchange = () => { if (timer) { stop(); play(); } };
$('#slider').oninput = () => show(+$('#slider').value);
$('#reveal').onclick = () => { shown = true; show(m); };
$('#feat').onchange = $('#tgt').onchange = rho;
addEventListener('resize', () => { if (F) show(m); rho(); });
(async () => {
  try {
    const r = await fetch('/data/SOLUSDT-m1.bin');
    if (!r.ok) throw new Error('HTTP ' + r.status);
    $('#msg').textContent = 'Decoding 1.25M minutes…';
    D = Nucleus.decode(await r.arrayBuffer());
    const b = Nucleus.hourBounds(D);
    T0 = b.t0; T1 = b.t1;
    $('#date').min = new Date(T0).toISOString().slice(0, 10);
    $('#date').max = new Date(T1).toISOString().slice(0, 10);
    setHour(hourT);
  } catch (e) { $('#msg').textContent = 'Could not load /data/SOLUSDT-m1.bin: ' + e.message; }
})();
trace();
