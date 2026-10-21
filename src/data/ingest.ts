// `kev ingest SYMBOL files...`: any set of price files for one market becomes markets/SYMBOL/ with a UTC H1 execution series,
// the finer series kept for intrabar settlement, a profile.json, and an ingest report saying what was found and assumed.
import { mkdirSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';
import type { Candle, InstrumentClass } from '../core/types.js';
import { toCols, type MarketProfile, type SourceFile } from '../core/market.js';
import { defaultSpec } from '../core/session.js';
import { MARKETS_DIR } from '../core/market-io.js';
import { loadFile, tfName, type Loaded } from './load.js';
import { binanceOffset, bucket, crossCheck, dailyBoundary, inferClock, stats, toUtc } from './profile.js';

const H = 3_600_000, D = 24 * H;
const iso = (t: number) => new Date(t).toISOString().slice(0, 16).replace('T', ' ');

export function classify(symbol: string, weekendShareRecent: number): InstrumentClass {
  const s = symbol.toUpperCase();
  if (/^(XAU|XAG|XPT|XPD|GOLD|SILVER)/.test(s)) return 'metal';
  if (/(BTC|ETH|SOL|XRP|ADA|DOGE|BNB|LTC|DOT|AVAX|LINK|MATIC|SHIB|TRX|TON|SUI|APT|ARB|OP|PEPE|USDT$)/.test(s)) return 'crypto';
  if (/(BUND|FGBL)/.test(s)) return 'bund';
  if (/(US30|US500|SPX|NAS|NDX|US100|DAX|GER|DE40|UK100|FTSE|JP225|NIKKEI|ES|NQ|STOXX|EU50|HK50|AUS200)/.test(s)) return 'index';
  const ccy = 'USD EUR GBP JPY CHF CAD AUD NZD SEK NOK DKK SGD HKD ZAR MXN TRY PLN CNH'.split(' ');
  if (s.length === 6 && ccy.includes(s.slice(0, 3)) && ccy.includes(s.slice(3))) return s.includes('USD') ? 'fx-major' : 'fx-cross';
  return weekendShareRecent > 0.15 ? 'crypto' : 'index';
}
const COSTS: Record<InstrumentClass, { spreadBps: number; note: string }> = {
  'fx-major': { spreadBps: 0.5, note: 'assumed typical retail spread' }, 'fx-cross': { spreadBps: 1, note: 'assumed typical retail spread' },
  metal: { spreadBps: 1.2, note: 'from the user\'s gold backtest page' }, index: { spreadBps: 1, note: 'assumed typical CFD spread' },
  bund: { spreadBps: 0.5, note: 'assumed' }, crypto: { spreadBps: 5, note: 'assumed broker CFD spread; set 0 for exchange spot via override.json' },
};

export interface IngestOptions { net: boolean; cls?: InstrumentClass }

export async function ingest(symbol: string, paths: string[], opt: IngestOptions) {
  const out = `${MARKETS_DIR}${symbol}/`;
  mkdirSync(out, { recursive: true });
  const notes: string[] = [], lines: string[] = [`# ${symbol}: ingest report`, '', `${new Date().toISOString().slice(0, 10)}. ${paths.length} file(s).`, ''];
  const files: Loaded[] = paths.map(loadFile).sort((a, b) => a.tf - b.tf || a.candles[0]!.t - b.candles[0]!.t);
  const decimals = Math.max(...files.map((f) => f.decimals)), tol = 0.51 * 10 ** -decimals;

  // ---- files ----
  lines.push('## Files', '', '| file | layout | tf | bars | from | to | fixes |', '|---|---|---|---|---|---|---|');
  const role = new Map<Loaded, SourceFile['role']>();
  for (const f of files) {
    const dup = files.find((g) => g !== f && role.get(g) !== 'duplicate' && g.tf === f.tf && g.candles.length === f.candles.length && g.candles[0]!.t === f.candles[0]!.t && g.candles.at(-1)!.c === f.candles.at(-1)!.c && files.indexOf(g) < files.indexOf(f));
    if (dup) { role.set(f, 'duplicate'); notes.push(`${basename(f.file)} is identical to ${basename(dup.file)} and was ignored.`); }
    const p = f.problems, fix = [p.badRows && `${p.badRows} bad rows`, p.duplicates && `${p.duplicates} duplicates`, p.outOfOrder && `${p.outOfOrder} out of order`, p.repairedOhlc && `${p.repairedOhlc} OHLC repaired`, p.descending && 'was newest-first', p.endStamped && 'end-stamped, moved to bar start', f.aligned < 0.99 && `${(100 * (1 - f.aligned)).toFixed(1)}% bars off the grid`].filter(Boolean).join(', ') || 'none';
    lines.push(`| ${basename(f.file)} | ${f.layout.name} | ${tfName(f.tf)} | ${f.candles.length} | ${iso(f.candles[0]!.t)} | ${iso(f.candles.at(-1)!.t)} | ${fix} |`);
  }
  const live = files.filter((f) => role.get(f) !== 'duplicate');

  // ---- consistency between timeframes (on the data clock, before any conversion) ----
  lines.push('', '## Consistency between timeframes', '');
  for (const fine of live) for (const coarse of live) {
    if (coarse.tf <= fine.tf || coarse.tf % fine.tf !== 0) continue;
    if (coarse.tf < D) {
      const r = crossCheck(fine.candles, fine.tf, coarse.candles, coarse.tf, tol);
      if (r.compared) lines.push(`- ${tfName(fine.tf)} -> ${tfName(coarse.tf)}: ${r.compared} complete buckets compared, ${r.mismatched} differ (largest difference ${r.maxDiff.toFixed(decimals)})`);
      if (r.compared && r.mismatched / r.compared > 0.01) notes.push(`${tfName(coarse.tf)} disagrees with ${tfName(fine.tf)} on ${(100 * r.mismatched / r.compared).toFixed(1)}% of buckets: probably a different feed.`);
    }
  }
  const hourly = live.find((f) => f.tf === H) ?? null;
  for (const d of live.filter((f) => f.tf === D)) {
    const ref = hourly?.candles ?? [...bucket(live.filter((f) => f.tf < H)[0]?.candles ?? [], H).values()];
    if (!ref.length) continue;
    const b = dailyBoundary(d.candles, ref, tol);
    lines.push(`- daily file ${basename(d.file)}: closes match the hourly series best with a day ending at ${b.anchorHour}:00 data clock (${(100 * b.matchedClose).toFixed(1)}% of ${b.compared} days)`);
    if (b.matchedClose >= 0.9) notes.push(`The file's own daily candles end at ${b.anchorHour}:00 on its clock (${(100 * b.matchedClose).toFixed(0)}% of closes match). kev builds its own days from the session instead (see Session).`);
    else { notes.push(`Daily file ${basename(d.file)} does not match the hourly series at any day boundary (best ${(100 * b.matchedClose).toFixed(0)}%): it comes from a different feed, so it is kept as a cross-check only.`); role.set(d, 'check'); }
  }

  // ---- the execution series: H1, from an hourly file or assembled from the finer ones ----
  const finer = live.filter((f) => f.tf < H && H % f.tf === 0);
  let h1raw: Candle[], partial = 0, filled = 0, h1src: string;
  if (hourly) {
    h1raw = hourly.candles.map((b) => ({ ...b })); h1src = basename(hourly.file);
    if (finer.length) {
      const have = new Set(h1raw.map((b) => b.t));
      for (const f of finer) for (const [k, x] of bucket(f.candles, H)) if (!have.has(k) && x.n === H / f.tf) { h1raw.push({ t: k, o: x.o, h: x.h, l: x.l, c: x.c, v: x.v }); have.add(k); filled++; }
      h1raw.sort((a, b) => a.t - b.t);
    }
  } else if (finer.length) {
    const maps = finer.map((f) => ({ f, need: H / f.tf, m: bucket(f.candles, H) }));
    const keys = [...new Set(maps.flatMap((x) => [...x.m.keys()]))].sort((a, b) => a - b);
    h1raw = keys.map((k) => {
      const opts = maps.map((x) => ({ x: x.m.get(k), full: (x.m.get(k)?.n ?? 0) / x.need })).filter((o) => o.x);
      const best = opts.find((o) => o.full === 1) ?? opts.sort((a, b) => b.full - a.full)[0]!;
      if (best.full < 1) partial++;
      const x = best.x!; return { t: k, o: x.o, h: x.h, l: x.l, c: x.c, v: x.v };
    });
    h1src = `assembled from ${finer.map((f) => tfName(f.tf)).join(', ')}`;
  } else throw new Error('need an hourly file or a finer one to build the H1 execution series');
  lines.push('', '## H1 execution series', '', `${h1raw.length} bars, ${h1src}.${filled ? ` ${filled} missing hours filled from finer files.` : ''}${partial ? ` ${partial} hours built from incomplete buckets (marked in notes).` : ''}`);
  if (partial > h1raw.length * 0.001) notes.push(`${partial} H1 bars were built from incomplete finer buckets; their high/low may be slightly understated.`);

  // ---- clock ----
  const st = stats(h1raw, H), recentYear = Math.max(...Object.keys(st.weekendShare).map(Number));
  const cls = opt.cls ?? classify(symbol, st.weekendShare[recentYear] ?? 0);
  let clock = inferClock(st), clockNote = clock.method;
  if (cls === 'crypto' && opt.net) {
    const ref = await binanceOffset(symbol, h1raw);
    if (ref && ref.corr > 0.8) { clock = { kind: 'fixed', offsetHours: ref.offsetHours, method: `matches Binance ${ref.pair} at an offset of ${ref.offsetHours}h (hourly return correlation ${ref.corr}, ${ref.windows} windows)`, confident: true }; clockNote = clock.method; }
    else if (ref) clockNote += `; Binance ${ref.pair} check inconclusive (best correlation ${ref.corr})`;
  }
  if (!clock.confident) notes.push(`Clock not confirmed: ${clockNote}. Session-based results (daily bars, Tuesday rule, sessions) depend on it; set clock in override.json if known.`);
  const conv = (c: Candle[]) => c.map((b) => ({ ...b, t: toUtc(b.t, clock) }));
  const h1 = conv(h1raw);
  lines.push('', '## Clock', '', `${clock.kind === 'fixed' ? `UTC${clock.offsetHours >= 0 ? '+' : ''}${clock.offsetHours}` : 'New York + 7h (UTC+2/+3)'}: ${clockNote}.`);

  // ---- weekend regime and volume ----
  const ws = Object.entries(st.weekendShare).map(([y, s]) => `${y}: ${(100 * s).toFixed(0)}%`).join(', ');
  lines.push('', '## Weekends and volume', '', `Share of H1 bars on Saturday/Sunday by year: ${ws}.`, `Weekend closes: ${st.weekendCloses}; other gaps of a day or more: ${st.holes}.`);
  const shares = Object.values(st.weekendShare);
  if (Math.min(...shares) < 0.05 && Math.max(...shares) > 0.2) notes.push(`Weekend trading changes over time (${ws}): the market was closed at weekends in some years and open in others. Sessions merge the short Sunday reopening into Monday in the closed years.`);
  if (st.holes > 5) notes.push(`${st.holes} gaps of 24h+ that are not weekends (holidays or missing data).`);
  const vm = Object.values(st.volumeMedianByYear).filter((v) => v > 0);
  lines.push(`Median bar volume by year: ${Object.entries(st.volumeMedianByYear).map(([y, v]) => `${y}: ${+v.toPrecision(3)}`).join(', ')}.`);
  if (vm.length > 1 && Math.max(...vm) / Math.min(...vm) > 10) notes.push(`Volume scale changes ${(Math.max(...vm) / Math.min(...vm)).toFixed(0)}x between years: only use volume relative to a trailing window.`);

  // ---- write ----
  const step = (tf: number) => Math.min(tf, H);
  writeFileSync(`${out}h1.json`, JSON.stringify(toCols(h1, H, decimals)));
  const ltf = finer.map((f) => {
    const c = conv(f.candles), file = `ltf-${tfName(f.tf)}.json`;
    writeFileSync(`${out}${file}`, JSON.stringify(toCols(c, step(f.tf), decimals)));
    return { tf: f.tf, file, from: c[0]!.t, to: c.at(-1)!.t };
  });
  const from = h1[0]!.t, to = h1.at(-1)!.t, sixty = from + 0.6 * (to - from), y60 = new Date(sixty).getUTCFullYear();
  const profile: MarketProfile = {
    symbol, cls, tick: +(10 ** -decimals).toFixed(decimals), decimals,
    clock: { kind: clock.kind, offsetHours: clock.offsetHours, method: clockNote },
    session: defaultSpec(cls),
    costs: { spreadBps: COSTS[cls].spreadBps, slipR: 0.1, note: `${COSTS[cls].note}; slippage 0.10R per trade as on the user's pages` },
    split: { isEndYear: new Date(sixty).getUTCMonth() < 6 ? y60 - 1 : y60 },
    data: { h1: 'h1.json', ltf, from, to, sources: files.map((f) => ({ file: basename(f.file), tf: f.tf, n: f.candles.length, from: f.candles[0]!.t, to: f.candles.at(-1)!.t, role: role.get(f) ?? (f.tf === H || (!hourly && f.tf < H) ? 'base' : f.tf < H ? 'ltf' : 'check') })) },
    notes,
  };
  writeFileSync(`${out}profile.json`, JSON.stringify(profile, null, 1));
  lines.push('', '## Profile', '', `class ${cls}; tick ${profile.tick}; session ${profile.session.day} days, Sunday stubs merged if <= ${profile.session.stubHours}h; costs ${profile.costs.spreadBps} bps + ${profile.costs.slipR}R (${COSTS[cls].note}); in-sample <= ${profile.split.isEndYear}. Override any of these in markets/${symbol}/override.json.`, '', '## Notes', '', ...(notes.length ? notes.map((n) => `- ${n}`) : ['- none']), '');
  writeFileSync(`${out}ingest.md`, lines.join('\n'));
  return { profile, report: lines.join('\n') };
}
