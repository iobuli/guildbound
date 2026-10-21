import { readFileSync } from 'node:fs';
import type { Candle } from '../core/types.js';

/** MT4 history export: "YYYY-MM-DD HH:MM<TAB>open<TAB>high<TAB>low<TAB>close<TAB>tickvolume". Broker-server time, read as UTC so the day boundary is the broker's. */
export function parseMt4(path: string): { candles: Candle[]; problems: Record<string, number> } {
  const problems = { badRows: 0, duplicates: 0, outOfOrder: 0, repairedOhlc: 0 };
  const out: Candle[] = [];
  for (const line of readFileSync(path, 'utf8').split(/\r?\n/)) {
    if (!line.trim()) continue;
    const p = line.split('\t');
    const t = Date.parse(p[0]!.replace(' ', 'T') + ':00Z');
    const [o, h, l, c, v] = p.slice(1).map(Number);
    if (p.length < 6 || !Number.isFinite(t) || ![o, h, l, c].every((x) => Number.isFinite(x) && x! > 0)) { problems.badRows++; continue; }
    const b: Candle = { t, o: o!, h: h!, l: l!, c: c!, v: v ?? 0 };
    const last = out[out.length - 1];
    if (last && b.t === last.t) { problems.duplicates++; continue; }
    if (last && b.t < last.t) { problems.outOfOrder++; continue; }
    if (b.h < Math.max(b.o, b.c) || b.l > Math.min(b.o, b.c) || b.h < b.l) { problems.repairedOhlc++; const hi = Math.max(b.h, b.l, b.o, b.c), lo = Math.min(b.h, b.l, b.o, b.c); b.h = hi; b.l = lo; }
    out.push(b);
  }
  return { candles: out, problems };
}
