const $ = (s) => document.querySelector(s), css = (v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
const usd = (x) => (x < 0 ? '-$' : '$') + Math.abs(x).toLocaleString('en-US', { maximumFractionDigits: 0 });
const pct = (x, d = 1) => (Number.isFinite(x) ? (100 * x).toFixed(d) + '%' : 'n/a'), f = (x, d = 2) => (Number.isFinite(x) ? x.toFixed(d) : 'n/a');
const day = (t) => new Date(t).toISOString().slice(0, 10);
let R = null;

async function load() {
  R = await (await fetch('/api/results')).json();
  if (R.error) { $('#verdict').textContent = R.error; return; }
  const fw = R.forward, pl = R.placebo, mc = R.montecarlo;
  $('#verdict').innerHTML = `Chosen on 2020-2022: <b>${R.chosen}</b>. On 2023 onward, $100,000 became <b class="${fw.final >= 1e5 ? 'good' : 'bad'}">${usd(fw.final)}</b> (${fw.trades} trades, mean ${f(fw.meanR)}R). The same rule on random orders: ${usd(pl.final)}. Bootstrap probability of ending below $100k: ${pct(mc.p_loss, 0)}. ${fw.final < 1e5 ? 'kev has no tradable edge after costs, as every earlier test said.' : 'Check the deflated Sharpe before trusting this.'}`;
  const tiles = [['Final equity', usd(fw.final)], ['CAGR', pct(fw.cagr)], ['Sharpe', f(fw.sharpe)], ['Sortino', f(fw.sortino)], ['Max drawdown', pct(fw.max_dd)], ['Calmar', f(fw.calmar)], ['Win rate', pct(fw.win, 0)], ['PSR (Sharpe > 0)', f(fw.psr)], ['Ulcer index', f(fw.ulcer, 1)], ['Skipped (busy/cap)', `${fw.skipped['market busy']}/${fw.skipped['risk cap']}`]];
  $('#tiles').innerHTML = tiles.map(([k, v]) => `<div class="tile"><b>${v}</b><span>${k}</span></div>`).join('');
  $('#cands').innerHTML = `<table><tr><th>candidate</th><th>trades</th><th>mean R</th><th>final</th><th>Sharpe</th><th>max DD</th><th>DSR</th><th>Kelly</th></tr>${Object.entries(R.candidates).map(([k, m]) => `<tr class="${k === R.chosen ? 'chosen' : ''}"><td>${k}</td><td>${m.trades}</td><td>${f(m.meanR)}</td><td>${usd(m.final)}</td><td>${f(m.sharpe)}</td><td>${pct(m.max_dd)}</td><td>${f(m.dsr)}</td><td>${f(m.kelly, 3)}</td></tr>`).join('')}</table>`;
  $('#mkts').innerHTML = `<table><tr><th>market</th><th>trades</th><th>mean R</th><th>P&amp;L</th></tr>${Object.entries(R.per_market).map(([k, v]) => `<tr><td>${k}</td><td>${v.trades}</td><td>${f(v.meanR)}</td><td class="${v.pnl >= 0 ? 'good' : 'bad'}">${usd(v.pnl)}</td></tr>`).join('')}</table>`;
  const years = [...new Set(Object.values(R.regime).flatMap((m) => Object.keys(m)))].sort();
  const mk = Object.keys(R.regime);
  $('#regime').innerHTML = `<table><tr><th>year</th>${mk.map((m) => `<th>${m}</th>`).join('')}</tr>${years.slice().reverse().map((y) => `<tr><td>${y}</td>${mk.map((m) => { const v = R.regime[m][y]; return v ? `<td title="VR z ${f(v.z, 1)}">H ${f(v.hurst)} · VR ${f(v.vr24)}</td>` : '<td>–</td>'; }).join('')}</tr>`).join('')}</table>`;
  if (R.patterns) {
    const Pt = R.patterns, ms = Pt.markets;
    $('#patsum').innerHTML = `${Pt.n_events.toLocaleString()} events. Entry at the event bar's close, signed in the trade's direction. Lift = event minus all hours of the same month and direction. Bold = CI excludes 0. Passed the pre-registered bar (q &lt; 0.05, 2 of 3 markets, n ≥ 100): <b>${Pt.chosen.length ? Pt.chosen.join(', ') : 'none'}</b>.`;
    $('#pat').innerHTML = `<table><tr><th>pattern</th><th>n</th><th>lift 24h [95% CI]</th><th>q</th><th>1h</th><th>4h</th><th>24h</th><th>72h</th><th>MFE/MAE 24h</th><th>best stop/target (R, gross)</th>${ms.map((m) => `<th>${m}</th>`).join('')}</tr>${Pt.table.map((x) => `<tr><td>${x.group}</td><td>${x.n}</td><td class="${x.lo > 0 ? 'good' : x.hi < 0 ? 'bad' : ''}" style="${x.lo > 0 || x.hi < 0 ? 'font-weight:600' : ''}">${f(x.lift24)} [${f(x.lo)}, ${f(x.hi)}]</td><td>${f(x.q)}</td><td>${f(x.r1)}</td><td>${f(x.r4)}</td><td>${f(x.r24)}</td><td>${f(x.r72)}</td><td>${f(x.mfe24)} / ${f(x.mae24)}</td><td>${x.best} ${f(x.pairs[x.best])}</td>${ms.map((m) => `<td>${x.per_market[m] == null ? '–' : f(x.per_market[m])}</td>`).join('')}</tr>`).join('')}</table>`;
    const qs = [...new Set(Object.values(Pt.quarters).flatMap((v) => Object.keys(v)))].sort();
    const cell = (v) => { if (!v) return '<td>–</td>'; const a = Math.min(1, Math.abs(v.lift24) / 0.25); return `<td title="n ${v.n}" style="background:${v.lift24 >= 0 ? `rgba(31,122,90,${a * 0.45})` : `rgba(192,57,43,${a * 0.45})`}">${f(v.lift24)}</td>`; };
    $('#patq').innerHTML = `<table><tr><th>pattern</th>${qs.map((q) => `<th>${q}</th>`).join('')}</tr>${Object.entries(Pt.quarters).map(([k, v]) => `<tr><td>${k}</td>${qs.map((q) => cell(v[q])).join('')}</tr>`).join('')}</table>`;
  }
  if (R.walk) {
    const W = R.walk, P = W['0.9'], S = W['0.5'];
    const line = (w, th) => `DSR ≥ ${th}: ${w.real.trades} trades, $100k → <b>${usd(w.real.final)}</b>${w.real.trades ? ` (mean ${f(w.real.meanR)}R; random orders ${usd(w.placebo.final)})` : ' (sat out everything)'}`;
    $('#walksum').innerHTML = `${line(P, '0.90 (pre-registered)')} · ${line(S, '0.50 (sensitivity)')}`;
    const mk = [...new Set(S.plan.map((p) => p.mkt))], ys = [...new Set(S.plan.map((p) => p.year))];
    $('#walk').innerHTML = `<table><tr><th>year</th>${mk.map((m) => `<th>${m}</th>`).join('')}</tr>${ys.map((y) => `<tr><td>${y}</td>${mk.map((m) => { const p = S.plan.find((q) => q.mkt === m && q.year === y); return `<td class="${p.dsr >= 0.9 ? 'good' : p.dsr >= 0.5 ? '' : 'sub'}" title="train n ${p.n}, ${p.trials} configs">${p.filter ?? '–'} / ${p.exit ?? ''} · train ${f(p.meanR)}R · DSR ${f(p.dsr)}</td>`; }).join('')}</tr>`).join('')}</table>`;
    $('#live').innerHTML = 'Live choice from all data to date: ' + Object.entries(R.live_choice).map(([m, c]) => `${m} ${c.trade ? `<b>trade ${c.filter} / ${c.exit}</b>` : `sit out (best ${c.filter} / ${c.exit}, DSR ${f(c.dsr)})`}`).join(' · ');
  }
  $('#trades').innerHTML = `<table><tr><th>filled</th><th>market</th><th>side</th><th>entry</th><th>exit</th><th>R</th><th>risked</th><th>P&amp;L</th><th>equity</th></tr>${R.ledger.slice().reverse().map((x) => `<tr><td>${day(x.t)}</td><td>${x.mkt}</td><td>${x.dir === 1 ? 'long' : 'short'}</td><td>${f(x.fill, x.fill > 100 ? 1 : 3)}</td><td>${day(x.exit_t)}${x.exit_t >= R.last_bar ? ' (open)' : ''}</td><td class="${x.R >= 0 ? 'good' : 'bad'}">${f(x.R)}</td><td>${usd(x.risk_usd)}</td><td class="${x.pnl >= 0 ? 'good' : 'bad'}">${usd(x.pnl)}</td><td>${usd(x.equity)}</td></tr>`).join('')}</table>`;
  draw();
}
function chart(id, series, opts = {}) {
  const cv = $(id), dpr = devicePixelRatio || 1, W = cv.clientWidth, H = cv.clientHeight, x = cv.getContext('2d');
  cv.width = W * dpr; cv.height = H * dpr; x.setTransform(dpr, 0, 0, dpr, 0, 0);
  const PL = 64, PR = 10, PT = 10, PB = 22, all = series.flatMap((s) => s.y).filter(Number.isFinite);
  let lo = Math.min(...all, opts.min ?? Infinity), hi = Math.max(...all, opts.max ?? -Infinity); if (hi === lo) hi = lo + 1;
  const n = Math.max(...series.map((s) => s.y.length)), X = (i, len) => PL + (i / Math.max(1, len - 1)) * (W - PL - PR), Y = (v) => PT + (hi - v) / (hi - lo) * (H - PT - PB);
  x.font = '11px ' + css('--mono'); x.fillStyle = css('--dim'); x.strokeStyle = css('--hair');
  for (let g = 0; g <= 4; g++) { const v = lo + (hi - lo) * g / 4; x.beginPath(); x.moveTo(PL, Y(v)); x.lineTo(W - PR, Y(v)); x.stroke(); x.fillText(opts.fmt ? opts.fmt(v) : f(v), 4, Y(v) + 4); }
  if (opts.labels) { const L = opts.labels, k = Math.max(1, Math.min(4, Math.floor((W - PL - PR) / 110))); for (let g = 0; g <= k; g++) { const i = Math.round((L.length - 1) * g / k); x.fillText(L[i], Math.min(W - 70, X(i, L.length) - 30), H - 6); } }
  for (const s of series) {
    if (s.band) { x.fillStyle = css('--band'); x.beginPath(); s.band[0].forEach((v, i) => x.lineTo(X(i, s.band[0].length), Y(v))); for (let i = s.band[1].length - 1; i >= 0; i--) x.lineTo(X(i, s.band[1].length), Y(s.band[1][i])); x.fill(); continue; }
    x.strokeStyle = s.color; x.lineWidth = s.w ?? 1.6; x.beginPath(); s.y.forEach((v, i) => (i ? x.lineTo(X(i, s.y.length), Y(v)) : x.moveTo(X(i, s.y.length), Y(v)))); x.stroke();
  }
  void n;
}
function draw() {
  if (!R || R.error) return;
  const c = R.curves, labels = c.days.map(day), fan = R.montecarlo.fan;
  chart('#eq', [{ band: [fan[0], fan[4]], y: [...fan[0], ...fan[4]] }, { y: c.placebo, color: css('--dim'), w: 1.2 }, { y: c.real, color: css('--acc'), w: 2 }, { y: [1e5, 1e5], color: css('--hair'), w: 1 }], { fmt: (v) => '$' + Math.round(v / 1000) + 'k', labels });
  const peak = []; c.real.reduce((m, v, i) => (peak[i] = Math.max(m, v)), -Infinity);
  chart('#dd', [{ y: c.real.map((v, i) => v / peak[i] - 1), color: css('--dn') }], { fmt: (v) => (100 * v).toFixed(0) + '%', max: 0, labels });
}
async function status() { try { const s = await (await fetch('/api/status')).json(); $('#status').textContent = `${s.live ? 'live on' : 'live off'} · ${s.busy ? 'working…' : s.message}${s.last_check ? ' · checked ' + new Date(s.last_check).toLocaleTimeString() : ''}`; } catch { /* server stopped */ } }
$('#recompute').onclick = async () => { $('#status').textContent = 'recomputing…'; await fetch('/api/recompute', { method: 'POST' }); load(); };
$('#live').onclick = async () => { $('#status').textContent = 'fetching new SOL hours and re-running kev…'; const r = await (await fetch('/api/live', { method: 'POST' })).json(); $('#status').textContent = r.message; load(); };
addEventListener('resize', draw);
load(); status(); setInterval(status, 15000);
