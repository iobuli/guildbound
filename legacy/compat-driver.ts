import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { f } from './stats.js';

const job = fileURLToPath(new URL('./compat-job.ts', import.meta.url));
const jobs: string[][] = [];
for (const mkt of ['sol', 'gold']) {
  for (const tier of [2, 3]) for (const space of [0.25, 0.5, 1]) jobs.push([`--mkt=${mkt}`, `--tier=${tier}`, `--space=${space}`, '--held=0']);
  jobs.push([`--mkt=${mkt}`, '--tier=2', '--space=0.5', '--held=1']);
}
const results: any[] = []; let next = 0, running = 0;
await new Promise<void>((resolve) => {
  const go = () => {
    while (running < 4 && next < jobs.length) {
      const a = jobs[next++]!; running++;
      const p = spawn(process.execPath, ['--import', 'tsx', job, ...a], { stdio: ['ignore', 'pipe', 'inherit'] });
      let buf = ''; p.stdout.on('data', (d) => (buf += d));
      p.on('exit', () => { running--; try { results.push(JSON.parse(buf.trim().split('\n').at(-1)!)); } catch { console.error('failed', a); } console.error(`done ${results.length}/${jobs.length}`); if (next >= jobs.length && running === 0) resolve(); else go(); });
    }
  }; go();
});
writeFileSync(new URL('../../out/compat.json', import.meta.url), JSON.stringify(results, null, 1));
const c = (x: any) => `${f(x.mean, 2)} [${f(x.lo, 2)},${f(x.hi, 2)}]`;
for (const mkt of ['sol', 'gold']) {
  console.log(`\n== ${mkt.toUpperCase()}: the SOL Trade Check spec, run causally (4R target, entry at the level, net of costs) ==`);
  for (const r of results.filter((x) => x.mkt === mkt).sort((a, b) => a.minTier - b.minTier || a.space - b.space || +a.held - +b.held))
    console.log(`touches>=${r.minTier} space ${String(r.space).padEnd(4)} ${r.held ? 'held-exit' : '         '} | fills ${String(r.fills).padStart(5)} (${f(r.perYear, 0)}/yr) | 4R ${c(r.r4).padEnd(18)} placebo ${f(r.pl4.mean, 2)} | IS ${f(r.is4.mean, 2)} OOS ${f(r.oos4.mean, 2)} | one-at-a-time ${r.one4.n} trades win ${f(100 * r.one4.win, 0)}% $${f(r.one4.perYear, 0)}/yr Sharpe ${f(r.one4.sharpe, 2)}`);
}
