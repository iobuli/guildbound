// Kev Studio viewer: draws the rig, plays clips, streams bone rotations to Kev (server.py), shows Kev's readout,
// and executes commands (from the console here, or POSTed to /api/cmd by anything else).
import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

const $ = (id) => document.getElementById(id);
const view = $('view');
const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
view.appendChild(renderer.domElement);
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0f1115);
const camera = new THREE.PerspectiveCamera(40, 1, 0.05, 100);
camera.position.set(2.6, 1.7, 3.2);
const controls = new OrbitControls(camera, renderer.domElement);
controls.target.set(0, 0.9, 0);
scene.add(new THREE.HemisphereLight(0xffffff, 0x303844, 1.6));
const sun = new THREE.DirectionalLight(0xffffff, 1.6); sun.position.set(3, 5, 4); scene.add(sun);
scene.add(new THREE.GridHelper(6, 24, 0x2c3240, 0x1d222c));

const state = { model: null, root: null, mixer: null, clips: [], action: null, bones: {}, rest: {}, overrides: {}, selected: null,
                playing: false, speed: 1, helper: null, clipName: null, overrideQ: {}, overrideP: {}, ghost: null, physDiag: null };
const markers = new THREE.Group(); scene.add(markers);
const sphereGeo = new THREE.SphereGeometry(0.018, 8, 8);
let comMesh = null, lastKev = null;

function log(text) { const l = $('log'); l.textContent += '\n' + text; l.scrollTop = l.scrollHeight; }

async function loadModel(file) {
  if (state.root) { scene.remove(state.root); if (state.helper) scene.remove(state.helper); }
  const gltf = await new GLTFLoader().loadAsync('/assets/anim/' + file);
  state.model = file; state.root = gltf.scene; scene.add(gltf.scene);
  state.bones = {}; state.rest = {}; state.overrides = {}; state.overrideQ = {}; state.overrideP = {}; state.restP = {};
  gltf.scene.traverse((o) => { if (o.isBone) { state.bones[o.name] = o; state.rest[o.name] = o.quaternion.clone(); state.restP[o.name] = o.position.clone(); } });
  state.mixer = new THREE.AnimationMixer(gltf.scene); state.clips = gltf.animations; state.action = null; state.playing = false; state.clipName = null;
  state.helper = new THREE.SkeletonHelper(gltf.scene); state.helper.visible = $('showSkel').checked; scene.add(state.helper);
  fillClips(); fillBones(); applyShow();
  $('model').value = file;
  sendTelemetry(true);
}

function fillClips() {
  const s = $('clip'); s.innerHTML = '';
  for (const c of state.clips) { const o = document.createElement('option'); o.value = o.textContent = c.name; s.appendChild(o); }
  if (!state.clips.length) { const o = document.createElement('option'); o.textContent = '(no clips)'; s.appendChild(o); }
}

function fillBones() {
  const box = $('bones'); box.innerHTML = '';
  const names = Object.keys(state.bones); $('bcount').textContent = `(${names.length})`;
  const f = $('bfilter').value.toLowerCase();
  for (const n of names) {
    if (f && !n.toLowerCase().includes(f)) continue;
    const d = document.createElement('div'); d.textContent = n; d.dataset.n = n; if (n === state.selected) d.className = 'sel';
    d.onclick = () => selectBone(n); box.appendChild(d);
  }
}

function selectBone(n) {
  state.selected = n; fillBones();
  $('bsel').textContent = n; $('sliders').style.display = 'block';
  const e = state.overrides[n] || [0, 0, 0];
  document.querySelectorAll('#sliders input[type=range]').forEach((r, i) => { r.value = e[i]; $('v' + 'xyz'[i]).textContent = e[i]; });
}

function setOverride(name, euler) { state.overrides[name] = euler; }

function play(name) {
  const clip = state.clips.find((c) => c.name === name) || (/^\d+$/.test(String(name)) ? state.clips[+name] : null);
  if (!clip) { log('no such clip: ' + name); return; }
  if (state.action) state.action.stop();
  state.action = state.mixer.clipAction(clip); state.action.reset().play(); state.clipName = clip.name;
  state.mixer.timeScale = state.speed; state.playing = true; $('clip').value = clip.name;
}

function handle(cmd) {
  switch (cmd.cmd) {
    case 'model': state.baked = null; loadModel(cmd.file); break;
    case 'play': state.baked = null; play(cmd.clip ?? $('clip').value); break;
    case 'pause': state.mixer.timeScale = 0; state.playing = false; break;
    case 'resume': state.mixer.timeScale = state.speed; state.playing = !!state.action; break;
    case 'seek': if (state.action) { state.action.time = cmd.value; state.mixer.update(0); } break;
    case 'speed': state.speed = cmd.value; if (state.playing) state.mixer.timeScale = cmd.value; break;
    case 'baked': state.baked = { fps: cmd.fps, frames: cmd.frames, t0: performance.now(), loop: cmd.loop }; state.overrideQ = {}; state.overrideP = {}; if (state.action) state.action.stop(); state.action = null; state.clipName = null; break;
    case 'stopclip': state.baked = null; if (state.action) state.action.stop(); state.action = null; state.playing = false; state.clipName = null; break;
    case 'pose': state.baked = null; state.ghost = cmd.ghost || null; state.physDiag = cmd.phys || null;
      if (cmd.phys && $('phys')) $('phys').textContent = `plants ${cmd.phys.plants}  slip ${cmd.phys.slip_mps} m/s  pen ${cmd.phys.penetration_mm} mm`;
      for (const [n, q] of Object.entries(cmd.quats || {})) state.overrideQ[n] = q; for (const [n, p] of Object.entries(cmd.pos || {})) state.overrideP[n] = p; break;
    case 'phys': if ($('showPhys')) $('showPhys').checked = !!cmd.on; if (!cmd.on && $('phys')) $('phys').textContent = ''; break;
    case 'reset': state.baked = null; state.overrideQ = {}; state.overrideP = {}; for (const [n, b] of Object.entries(state.bones)) b.position.copy(state.restP[n]); state.overrides = {}; if (state.action) state.action.stop(); state.action = null; state.playing = false; state.clipName = null;
      for (const [n, b] of Object.entries(state.bones)) b.quaternion.copy(state.rest[n]); break;
    case 'set': if (state.bones[cmd.bone]) { setOverride(cmd.bone, cmd.euler); if (state.selected === cmd.bone) selectBone(cmd.bone); } break;
  }
}

const tmpE = new THREE.Euler(), tmpQ = new THREE.Quaternion();
function applyBaked() {
  const bk = state.baked; if (!bk) return;
  const n = bk.frames.length; let f = bk.fixed != null ? bk.fixed : Math.floor((performance.now() - bk.t0) / 1000 * bk.fps);
  f = bk.loop ? f % n : Math.min(f, n - 1);
  const fr = bk.frames[f];
  for (const [nm, q] of Object.entries(fr.q)) { const b = state.bones[nm]; if (b) b.quaternion.set(q[0], q[1], q[2], q[3]); }
  const pb = state.bones.pelvis; if (pb) pb.position.set(fr.p[0], fr.p[1], fr.p[2]);
  state.bakedFrame = f;
}
function applyOverrides() {
  applyBaked();
  for (const [n, e] of Object.entries(state.overrides)) {
    const b = state.bones[n]; if (!b) continue;
    tmpE.set(e[0] * Math.PI / 180, e[1] * Math.PI / 180, e[2] * Math.PI / 180, 'XYZ'); tmpQ.setFromEuler(tmpE);
    b.quaternion.copy(state.rest[n]).multiply(tmpQ);
  }
  for (const [n, q] of Object.entries(state.overrideQ)) { const b = state.bones[n]; if (b) b.quaternion.set(q[0], q[1], q[2], q[3]); }
  for (const [n, p] of Object.entries(state.overrideP)) { const b = state.bones[n]; if (b) b.position.set(p[0], p[1], p[2]); }
}

let lastSent = '', lastSendAt = 0;
async function sendTelemetry(force) {
  const now = performance.now();
  if (!force && now - lastSendAt < 80) return;
  const quats = {}; let h = '';
  for (const [n, b] of Object.entries(state.bones)) { const q = b.quaternion; quats[n] = [+q.x.toFixed(5), +q.y.toFixed(5), +q.z.toFixed(5), +q.w.toFixed(5)]; h += quats[n].join(','); }
  if (!force && h === lastSent && now - lastSendAt < 1000) return;
  lastSent = h; lastSendAt = now;
  const dur = state.action ? state.action.getClip().duration : 0;
  const t = state.action ? state.action.time : 0;
  try { await fetch('/api/telemetry', { method: 'POST', body: JSON.stringify({ model: state.model, clip: state.clipName, playing: state.playing, t, duration: dur, speed: state.speed, quats }) }); } catch (e) { /* server gone */ }
}

function renderKev(k) {
  lastKev = k;
  if (k.balance) {
    const b = $('bal'); b.className = k.balance.over_foot ? 'over' : 'off';
    b.textContent = (k.balance.over_foot ? 'centre of mass OVER the ' : 'centre of mass OFF the ') + k.balance.support + ' foot';
    $('balkv').innerHTML = kv('forward of foot centre', k.balance.forward) + kv('left of foot centre', k.balance.left) + kv('off the foot (body heights)', k.balance.off_foot_heights);
  }
  if (k.facings) $('facings').innerHTML = Object.entries(k.facings).map(([n, v]) => kv(n, `${v.toward} (${v.off_deg}°)`)).join('');
  if (k.heights) $('lms').innerHTML = Object.entries(k.heights).map(([n, v]) => kv(n, v)).join('');
  drawMarkers(k);
  const r = k.ref && k.ref.name ? k.ref : null;
  $('ctl').innerHTML = k.capturing ? kv('capturing', 'recording one loop...') : r ? kv('reference', r.name) + kv('time', `${r.tau} / ${r.duration.toFixed(2)} s`) +
    kv('tracking error mean', (r.tracking_error_m.mean * 100).toFixed(1) + ' cm') + kv('max', (r.tracking_error_m.max * 100).toFixed(1) + ' cm') +
    (r.worst || []).map(([n, e]) => kv('worst: ' + n, (e * 100).toFixed(1) + ' cm')).join('') : '<span style="color:var(--dim)">idle: ref capture / ref play</span>';
  if (state.action) { const d = state.action.getClip().duration; $('seek').value = d ? state.action.time / d : 0; $('tinfo').textContent = `t ${state.action.time.toFixed(2)} / ${d.toFixed(2)}`; }
}
const kv = (a, b) => `<div class="kv"><span>${a}</span><span>${b}</span></div>`;

function drawMarkers(k) {
  markers.clear();
  if (!$('showLm').checked || !k.landmarks) return;
  const mat = new THREE.MeshBasicMaterial({ color: 0x7cc4ff, depthTest: false });
  for (const p of Object.values(k.landmarks)) { const m = new THREE.Mesh(sphereGeo, mat); m.position.set(...p); m.renderOrder = 10; markers.add(m); }
  const c = k.balance.com; const ok = k.balance.over_foot;
  const com = new THREE.Mesh(new THREE.SphereGeometry(0.045, 12, 12), new THREE.MeshBasicMaterial({ color: ok ? 0x5fd38d : 0xff6b6b, depthTest: false }));
  com.position.set(...c); com.renderOrder = 11; markers.add(com);
  const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(...c), new THREE.Vector3(c[0], 0, c[2])]), new THREE.LineBasicMaterial({ color: ok ? 0x5fd38d : 0xff6b6b }));
  markers.add(line);
  if ($('showPhys') && $('showPhys').checked && state.ghost) {
    const ghost = new THREE.MeshBasicMaterial({ color: 0xff9a3c, depthTest: false, transparent: true, opacity: 0.85 });
    for (const p of Object.values(state.ghost)) { const m = new THREE.Mesh(sphereGeo, ghost); m.position.set(...p); m.scale.setScalar(0.65); m.renderOrder = 9; markers.add(m); }
  }
}

function applyShow() {
  if (state.helper) state.helper.visible = $('showSkel').checked;
  if (state.root) state.root.traverse((o) => { if (o.isSkinnedMesh) o.visible = $('showMesh').checked; });
  if (lastKev) drawMarkers(lastKev);
}

// ---- UI wiring
async function init() {
  const models = await (await fetch('/api/models')).json();
  for (const m of models) { const o = document.createElement('option'); o.value = o.textContent = m; $('model').appendChild(o); }
  $('model').onchange = () => loadModel($('model').value);
  $('clip').onchange = () => play($('clip').value);
  $('play').onclick = () => { if (state.action) handle({ cmd: 'resume' }); else play($('clip').value); };
  $('pause').onclick = () => handle({ cmd: 'pause' });
  $('seek').oninput = () => { if (state.action) { handle({ cmd: 'pause' }); state.action.time = $('seek').value * state.action.getClip().duration; state.mixer.update(0); } };
  ['showSkel', 'showLm', 'showMesh'].forEach((id) => { $(id).onchange = applyShow; });
  $('bfilter').oninput = fillBones;
  document.querySelectorAll('#sliders input[type=range]').forEach((r, i) => {
    r.oninput = () => { if (!state.selected) return; const e = (state.overrides[state.selected] || [0, 0, 0]).slice(); e[i] = +r.value; setOverride(state.selected, e); $('v' + 'xyz'[i]).textContent = r.value; };
  });
  $('resetBone').onclick = () => { delete state.overrides[state.selected]; selectBone(state.selected); };
  $('resetAll').onclick = () => handle({ cmd: 'reset' });
  if ($('showPhys')) $('showPhys').onchange = () => fetch('/api/exec', { method: 'POST', body: JSON.stringify({ line: $('showPhys').checked ? 'phys on' : 'phys off' }) });
  const hist = []; let hi = 0;
  $('line').onkeydown = async (ev) => {
    if (ev.key === 'ArrowUp') { hi = Math.max(0, hi - 1); $('line').value = hist[hi] || ''; }
    else if (ev.key === 'ArrowDown') { hi = Math.min(hist.length, hi + 1); $('line').value = hist[hi] || ''; }
    else if (ev.key === 'Enter') {
      const line = $('line').value.trim(); if (!line) return; hist.push(line); hi = hist.length; $('line').value = '';
      await fetch('/api/exec', { method: 'POST', body: JSON.stringify({ line, from: 'console' }) });
    }
  };
  const ev = new EventSource('/api/events');
  ev.addEventListener('cmd', (e) => handle(JSON.parse(e.data)));
  ev.addEventListener('state', (e) => renderKev(JSON.parse(e.data)));
  ev.addEventListener('log', (e) => { const d = JSON.parse(e.data); log(`kev> ${d.line}${d.from !== 'console' ? '   [' + d.from + ']' : ''}\n${d.out}`); });
  const first = new URLSearchParams(location.search).get('model') || (models.includes('ual2.glb') ? 'ual2.glb' : models[0]);
  await loadModel(first);
  const q = new URLSearchParams(location.search).get('play'); if (q) play(q);
}

function resize() { const w = view.clientWidth, h = view.clientHeight; renderer.setSize(w, h, false); camera.aspect = w / h; camera.updateProjectionMatrix(); }
addEventListener('resize', resize); resize();
const clock = new THREE.Clock();
renderer.setAnimationLoop(() => {
  const dt = Math.min(clock.getDelta(), 0.1);
  if (state.mixer) state.mixer.update(dt);
  applyOverrides();
  if (state.root) { state.root.updateMatrixWorld(true); sendTelemetry(false); }
  controls.update(); renderer.render(scene, camera);
});
init();

// deterministic stepping for offline recording: ksStep(frame) pins the baked track to a frame
window.ksStep = (f) => { if (state.baked) state.baked.fixed = f; return state.baked ? state.baked.frames.length : 0; };
