// Works out what a price file looks like without being told: delimiter, header, which column is what, and how time is written.
// Known layouts without a header: MT4 history (datetime,o,h,l,c,v), MT4 CSV (date,time,o,h,l,c,v), Binance klines (epoch,o,h,l,c,v,...).

export interface Layout {
  name: string;
  delim: string;
  header: string[] | null;
  time: number;
  /** a separate time-of-day column (date + time layouts) */
  time2: number | null;
  o: number; h: number; l: number; c: number; v: number | null;
  /** day-first dates (31.12.2024) rather than month-first (12/31/2024) */
  dayFirst: boolean;
  /** European numbers ("1234,5" with ';' as the delimiter) */
  decimalComma: boolean;
}

const norm = (s: string) => s.trim().toLowerCase().replace(/[<>"'\s_.-]/g, '');
const NAMES = {
  time: ['time', 'datetime', 'timestamp', 'opentime', 'date', 'gmttime', 'localtime', 'unix', 'ts', 'timeutc', 'dateutc'],
  time2: ['time', 'hour'],
  o: ['open', 'o', 'openprice', 'bidopen'], h: ['high', 'h', 'highprice', 'bidhigh'], l: ['low', 'l', 'lowprice', 'bidlow'],
  c: ['close', 'c', 'closeprice', 'last', 'bidclose', 'price'],
  v: ['volume', 'vol', 'v', 'tickvol', 'tickvolume', 'realvolume', 'basevolume', 'volumefrom'],
};
const isNum = (s: string) => /^[-+]?(\d+([.,]\d*)?|[.,]\d+)([eE][-+]?\d+)?$/.test(s.trim());
const isDateish = (s: string) => /^\d{1,4}[-./]\d{1,2}[-./]\d{1,4}/.test(s.trim()) || /^\d{9,16}$/.test(s.trim());
const isClock = (s: string) => /^\d{1,2}:\d{2}(:\d{2}(\.\d+)?)?$/.test(s.trim());

export function sniff(lines: string[]): Layout {
  const sample = lines.filter((l) => l.trim()).slice(0, 30);
  if (!sample.length) throw new Error('empty file');
  // delimiter: the candidate that splits every sample line into the same number (>= 5) of fields
  const delim = ['\t', ',', ';', '|'].map((d) => ({ d, n: sample.slice(1).map((l) => l.split(d).length) }))
    .filter((x) => x.n.length === 0 || (x.n.every((k) => k === x.n[0]) && x.n[0]! >= 5))
    .sort((a, b) => (b.n[0] ?? 0) - (a.n[0] ?? 0))[0]?.d;
  if (!delim) throw new Error('could not find a delimiter that gives at least 5 columns on every line');
  const rows = sample.map((l) => l.split(delim).map((x) => x.trim()));
  const first = rows[0]!;
  const header = first.slice(1).some((x) => x !== '' && !isNum(x) && !isClock(x)) || !first.some((x) => isDateish(x) || isNum(x)) ? first : null;
  const body = header ? rows.slice(1) : rows;
  const decimalComma = delim === ';' && body.some((r) => r.slice(1).some((x) => /^\d+,\d+$/.test(x)));
  // day-first if any date has a first field > 12, month-first if any has a second field > 12; ISO (year first) is unambiguous
  let dayFirst = true;
  for (const r of body) { const m = /^(\d{1,2})[-./](\d{1,2})[-./](\d{4})/.exec(r[0] ?? ''); if (m) { if (+m[1]! > 12) { dayFirst = true; break; } if (+m[2]! > 12) { dayFirst = false; break; } if ((r[0] ?? '').includes('/')) dayFirst = false; } }

  if (header) {
    const H = header.map(norm), find = (names: string[], not: number[] = []) => { for (const n of names) { const i = H.findIndex((h, k) => h === n && !not.includes(k)); if (i >= 0) return i; } return -1; };
    let time = find(NAMES.time.filter((n) => n !== 'date')), dateCol = H.indexOf('date');
    let time2: number | null = null;
    if (time < 0 && dateCol >= 0) time = dateCol;
    else if (time >= 0 && dateCol >= 0 && dateCol !== time && isClock(body[0]?.[time] ?? '')) { time2 = time; time = dateCol; }
    const o = find(NAMES.o), h = find(NAMES.h), l = find(NAMES.l), c = find(NAMES.c, [o, h, l]);
    let v = find(NAMES.v);
    if (v < 0) v = H.findIndex((x) => x.includes('vol'));
    if ([time, o, h, l, c].some((x) => x < 0)) throw new Error(`header found but could not map columns: ${header.join(', ')}`);
    return { name: 'header', delim, header, time, time2, o, h, l, c, v: v >= 0 ? v : null, dayFirst, decimalComma };
  }
  const r = body[0]!;
  if (isDateish(r[0]!) && isClock(r[1]!) && r.length >= 6) return { name: 'date+time (MT4 CSV)', delim, header: null, time: 0, time2: 1, o: 2, h: 3, l: 4, c: 5, v: r.length > 6 ? 6 : null, dayFirst, decimalComma };
  if (/^\d{9,16}$/.test(r[0]!)) return { name: 'epoch (Binance klines)', delim, header: null, time: 0, time2: null, o: 1, h: 2, l: 3, c: 4, v: r.length > 5 ? 5 : null, dayFirst, decimalComma };
  if (isDateish(r[0]!)) return { name: delim === '\t' ? 'MT4 history' : 'datetime,o,h,l,c[,v]', delim, header: null, time: 0, time2: null, o: 1, h: 2, l: 3, c: 4, v: r.length > 5 ? 5 : null, dayFirst, decimalComma };
  throw new Error(`unrecognised layout; first line: ${lines[0]?.slice(0, 120)}`);
}

/**
 * Parses a timestamp to epoch ms. Naive times are read on the file's own clock as if UTC; ingest converts that clock to UTC afterwards.
 * Accepts ISO and variants (2024-01-02 03:04[:05][.sss][Z|+02:00]), 2024.01.02, day- or month-first dates, and epoch s/ms/us.
 */
export function parseTime(s: string, s2: string | null, dayFirst: boolean): number {
  const x = s2 ? `${s.trim()} ${s2.trim()}` : s.trim();
  if (/^\d{9,16}$/.test(x)) { const n = Number(x); return n >= 1e15 ? Math.round(n / 1000) : n >= 1e12 ? n : n * 1000; }
  let m = /^(\d{4})[-./](\d{1,2})[-./](\d{1,2})(?:[ T](\d{1,2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3})\d*)?)?)?\s*(Z|[+-]\d{2}:?\d{2})?$/.exec(x);
  let y: number, mo: number, d: number;
  if (m) { y = +m[1]!; mo = +m[2]!; d = +m[3]!; }
  else {
    m = /^(\d{1,2})[-./](\d{1,2})[-./](\d{4})(?:[ T](\d{1,2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3})\d*)?)?)?\s*(Z|[+-]\d{2}:?\d{2})?$/.exec(x);
    if (!m) return NaN;
    y = +m[3]!; mo = dayFirst ? +m[2]! : +m[1]!; d = dayFirst ? +m[1]! : +m[2]!;
  }
  let t = Date.UTC(y, mo - 1, d, +(m[4] ?? 0), +(m[5] ?? 0), +(m[6] ?? 0), +((m[7] ?? '0').padEnd(3, '0')));
  const tz = m[8];
  if (tz && tz !== 'Z') { const sign = tz[0] === '-' ? -1 : 1, hh = +tz.slice(1, 3), mm = +tz.slice(-2); t -= sign * (hh * 60 + mm) * 60_000; }
  return t;
}
