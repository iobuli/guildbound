import type { Candle } from '../core/types.js';

/** SOL with no memory: bar shapes and volumes kept, bar order shuffled in blocks, drift removed. */
export function shuffledCandles(c: Candle[], seed: number, block = 24): Candle[] {
  let s = seed >>> 0;
  const rand = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  const shapes = c.slice(1).map((b, k) => ({ gap: Math.log(b.o / c[k]!.c), h: Math.log(b.h / b.o), l: Math.log(b.l / b.o), cl: Math.log(b.c / b.o), v: b.v }));
  const blocks: (typeof shapes)[] = [];
  for (let k = 0; k < shapes.length; k += block) blocks.push(shapes.slice(k, k + block));
  for (let k = blocks.length - 1; k > 0; k--) { const j = Math.floor(rand() * (k + 1)); [blocks[k], blocks[j]] = [blocks[j]!, blocks[k]!]; }
  const flat = blocks.flat(), mu = flat.reduce((a, x) => a + x.gap + x.cl, 0) / flat.length;
  const out: Candle[] = [c[0]!];
  let prev = c[0]!.c;
  flat.forEach((x, k) => {
    const o = prev * Math.exp(x.gap - mu), b = { t: c[k + 1]!.t, o, h: o * Math.exp(x.h), l: o * Math.exp(x.l), c: o * Math.exp(x.cl), v: x.v };
    out.push(b); prev = b.c;
  });
  return out;
}
