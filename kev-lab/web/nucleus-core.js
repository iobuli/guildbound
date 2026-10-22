// The nucleus, JS port of kev-lab/nucleus.py (same definitions, same integer-tick prices, so signs and orders are identical and positions agree to float32 storage precision; see
// tests/test_nucleus.py). Works in the browser and in Node. Data: { t: Float64Array (ms), h, l, c: Int32Array (ticks of 1e-4) }.
(function () {
  const TFS = [['1m', 60000], ['5m', 300000], ['15m', 900000], ['1h', 3600000], ['4h', 14400000], ['1D', 86400000]];
  const NS = [9, 26, 52];
  const ORDERS = ['9>26>52', '9>52>26', '26>9>52', '26>52>9', '52>9>26', '52>26>9'];
  const DAY = 86400000, WARMUP_DAYS = 106, MISSING = -9, TIE = -2;
  const sign = (x) => (Number.isNaN(x) ? NaN : x > 0 ? 1 : x < 0 ? -1 : 0);
  const z = (x) => (Number.isNaN(x) ? MISSING : x); // 0 stays a real tie

  function lowerBound(a, x) { let lo = 0, hi = a.length; while (lo < hi) { const m = (lo + hi) >> 1; if (a[m] < x) lo = m + 1; else hi = m; } return lo; }

  /** Bars of one timeframe over minutes [a, b): highs, lows, closes, and per minute its bar index and the forming bar's running high/low. */
  function timeframe(d, a, b, size) {
    const BH = [], BL = [], BC = [], J = new Int32Array(b - a), runH = new Float64Array(b - a), runL = new Float64Array(b - a);
    let cur = NaN;
    for (let i = a; i < b; i++) {
      const id = Math.floor(d.t[i] / size), k = i - a;
      if (id !== cur) { cur = id; BH.push(d.h[i]); BL.push(d.l[i]); BC.push(d.c[i]); }
      const j = BH.length - 1;
      if (d.h[i] > BH[j]) BH[j] = d.h[i];
      if (d.l[i] < BL[j]) BL[j] = d.l[i];
      BC[j] = d.c[i];
      J[k] = j; runH[k] = BH[j]; runL[k] = BL[j];
    }
    return { BH, BL, BC, J, runH, runL };
  }
  // max/min of bars j-w .. j-1 (NaN if j < w)
  function prevMax(B, j, w) { if (j < w) return NaN; let m = -Infinity; for (let i = j - w; i < j; i++) if (B[i] > m) m = B[i]; return m; }
  function prevMin(B, j, w) { if (j < w) return NaN; let m = Infinity; for (let i = j - w; i < j; i++) if (B[i] < m) m = B[i]; return m; }
  const cmidBar = (tf, j, n) => (prevMax(tf.BH, j, n) + prevMin(tf.BL, j, n)) / 2;

  function line(tf, k, c, n) {
    const j = tf.J[k];
    const pmH = prevMax(tf.BH, j, n - 1), pmL = prevMin(tf.BL, j, n - 1);
    const ok = !Number.isNaN(pmH);
    const HH = ok ? Math.max(pmH, tf.runH[k]) : tf.runH[k], LL = ok ? Math.min(pmL, tf.runL[k]) : tf.runL[k];
    const mid = ok ? (HH + LL) / 2 : NaN, half = ok ? (HH - LL) / 2 : NaN;
    const cH = prevMax(tf.BH, j, n), cL = prevMin(tf.BL, j, n);
    const cmid = (cH + cL) / 2, chalf = (cH - cL) / 2;
    const pos = !ok ? NaN : half > 0 ? (c - mid) / half : 0;
    const cpos = Number.isNaN(cmid) ? NaN : chalf > 0 ? (c - cmid) / chalf : 0;
    const js = j - n + 1, shifted = js >= 0 ? cmidBar(tf, js, n) : NaN;
    const jl = j - n, lagc = jl >= 0 ? tf.BC[jl] : NaN;
    return { n, mid, HH, LL, pos, cmid, cpos, slope: z(sign(mid - cmid)), shift: z(sign(c - shifted)), lag: z(sign(c - lagc)), okSlope: ok && !Number.isNaN(cmid) };
  }

  /** Index of the first minute of the UTC day `days` existing days before hourT's day. Matches nucleus.warmup_index. */
  function warmupIndex(t, hourT, days) {
    let i = lowerBound(t, (Math.floor(hourT / DAY) + 1) * DAY) - 1;
    if (i < 0) return 0;
    let seen = 0, prev = NaN;
    while (i >= 0) {
      const id = Math.floor(t[i] / DAY);
      if (id !== prev) {
        seen++;
        prev = id;
        if (seen === days + 1) {
          while (i > 0 && Math.floor(t[i - 1] / DAY) === id) i--;
          return i;
        }
      }
      i--;
    }
    return 0;
  }

  /** First and last hour present in the minute series (ms). The page uses this instead of a fixed calendar. */
  function hourBounds(d) {
    const H = 3600000;
    if (!d.t.length) return { t0: 0, t1: 0 };
    return { t0: Math.floor(d.t[0] / H) * H, t1: Math.floor(d.t[d.t.length - 1] / H) * H };
  }

  /** The minute-frames of the hour starting at hourT, from WARMUP_DAYS existing days before that day. */
  function frames(d, hourT) {
    const a = warmupIndex(d.t, hourT, WARMUP_DAYS), b = lowerBound(d.t, hourT + 3600000), k0 = lowerBound(d.t, hourT);
    const tfs = TFS.map(([name, size]) => [name, timeframe(d, a, b, size)]);
    const out = [];
    for (let i = k0; i < b; i++) {
      const k = i - a, c = d.c[i];
      const f = { t: d.t[i], c, tf: {}, lines: [] };
      let s = 0, sn = 0, sl = 0, sln = 0, valid = true;
      for (const [name, tf] of tfs) {
        const L = NS.map((n) => line(tf, k, c, n));
        let ts = 0, tn = 0;
        for (const x of L) {
          f.lines.push({ tf: name, ...x });
          if (!Number.isNaN(x.pos)) { s += sign(x.pos); sn++; ts += sign(x.pos); tn++; } else valid = false;
          if (x.slope === MISSING || x.shift === MISSING || x.lag === MISSING) valid = false;
          if (x.okSlope) { sl += x.slope; sln++; }
        }
        const mids = L.map((x) => x.mid), ok = mids.every((x) => !Number.isNaN(x));
        const tied = ok && (mids[0] === mids[1] || mids[0] === mids[2] || mids[1] === mids[2]);
        const rank = [0, 1, 2].sort((p, q) => (mids[q] - mids[p]) || (p - q));
        const order = !ok ? -1 : tied ? TIE : ORDERS.indexOf(rank.map((r) => NS[r]).join('>'));
        const jc = tf.J[k] - 25;
        let cloud = -9, A = NaN, B = NaN;
        if (jc >= 0) {
          A = (cmidBar(tf, jc, 9) + cmidBar(tf, jc, 26)) / 2; B = cmidBar(tf, jc, 52);
          if (!Number.isNaN(A) && !Number.isNaN(B)) cloud = c > Math.max(A, B) ? 1 : c < Math.min(A, B) ? -1 : 0; // MISSING (-9) until both spans exist
        }
        if (cloud === -9) valid = false;
        f.tf[name] = { order, cloud, spanA: A, spanB: B, bal: tn ? ts / tn : NaN };
      }
      f.bal = sn ? s / sn : NaN;
      f.slopeBal = sln ? sl / sln : NaN;
      f.valid = valid;
      out.push(f);
    }
    return out;
  }

  /** The export_bin layout: int32 [n, minute(n), h(n), l(n), c(n)], little-endian. */
  function decode(buf) {
    const v = new Int32Array(buf.buffer ? buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength) : buf);
    const n = v[0], t = new Float64Array(n);
    for (let i = 0; i < n; i++) t[i] = v[1 + i] * 60000;
    return { t, h: v.subarray(1 + n, 1 + 2 * n), l: v.subarray(1 + 2 * n, 1 + 3 * n), c: v.subarray(1 + 3 * n, 1 + 4 * n) };
  }

  const api = { TFS, NS, ORDERS, frames, decode, WARMUP_DAYS, MISSING, TIE, warmupIndex, hourBounds };
  globalThis.Nucleus = api; // browser global; Node scripts read it after require() (this folder is ESM under the repo's package.json)
})();
