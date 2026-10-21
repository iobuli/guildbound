import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { f } from './stats.js';

const tf = process.argv.find((a) => a.startsWith('--tf='))?.slice(5) ?? 'h1';
const job = fileURLToPath(new URL('./gold-opt.ts', import.meta.url));
const grid: { zone: number; react: number; space: number }[] = [];
for (const zone of [0.15, 0.25, 0.4]) for (const react of [0.75, 1, 1.5]) for (const space of [0.5, 1]) grid.push({ zone, react, space });
const jobs = [0, 101].flatMap((nul) => grid.map((g) => ({ ...g, nul })));
const results: any[] = [];
let next = 0, running = 0;
const CONC = Number(process.argv.find((a) => a.startsWith('--conc='))?.slice(7) ?? 4);
await new Promise<void>((resolve) => {
  const launch = () => {
    while (running < CONC && next < jobs.length) {
      const j = jobs[next++]!;
      running++;
      const p = spawn(process.execPath, ['--import', 'tsx', job, `--tf=${tf}`, `--zone=${j.zone}`, `--react=${j.react}`, `--space=${j.space}`, `--null=${j.nul}`], { stdio: ['ignore', 'pipe', 'inherit'] });
      let buf = '';
      p.stdout.on('data', (d) => (buf += d));
      p.on('exit', () => { running--; try { results.push(JSON.parse(buf.trim().split('\n').at(-1)!)); } catch { console.error('job failed', j); } console.error(`done ${results.length}/${jobs.length}`); if (results.length === jobs.length || (next >= jobs.length && running === 0)) resolve(); else launch(); });
    }
  };
  launch();
});
writeFileSync(new URL(`../../out/gold_opt_${tf}.json`, import.meta.url), JSON.stringify(results, null, 1));
for (const nul of [0, 101]) {
  const rs = results.filter((r) => r.nullSeed === nul).sort((a, b) => a.cfg.zoneHalfAtr - b.cfg.zoneHalfAtr || a.cfg.touchReactionAtr - b.cfg.touchReactionAtr || a.cfg.retestMinExcursionAtr - b.cfg.retestMinExcursionAtr);
  console.log(`\n== ${nul ? 'GOLD block-shuffled (memory destroyed)' : 'GOLD (real)'} ${tf.toUpperCase()}: mean net R at 4R; GO = every veto clears`);
  for (const r of rs) console.log(`zone ${r.cfg.zoneHalfAtr.toFixed(2)} react ${r.cfg.touchReactionAtr.toFixed(2)} space ${r.cfg.retestMinExcursionAtr.toFixed(1)} | GO IS n=${String(r.goIS.n).padStart(5)} ${f(r.goIS.m4, 3).padStart(6)}  OOS n=${String(r.goOOS.n).padStart(5)} ${f(r.goOOS.m4, 3).padStart(6)} | ALL IS ${f(r.allIS.m4, 3).padStart(6)} OOS ${f(r.allOOS.m4, 3).padStart(6)}`);
  const elig = rs.filter((r) => r.goIS.n >= 300), best = [...elig].sort((a, b) => b.goIS.m4 - a.goIS.m4)[0];
  const dflt = rs.find((r) => r.cfg.zoneHalfAtr === 0.25 && r.cfg.touchReactionAtr === 1 && r.cfg.retestMinExcursionAtr === 0.5);
  if (best) console.log(`-> best in-sample: zone ${best.cfg.zoneHalfAtr} react ${best.cfg.touchReactionAtr} space ${best.cfg.retestMinExcursionAtr}: IS ${f(best.goIS.m4, 3)} -> OOS ${f(best.goOOS.m4, 3)} (n=${best.goOOS.n}) | default OOS ${f(dflt?.goOOS.m4 ?? NaN, 3)} | mean over configs IS ${f(elig.reduce((a, r) => a + r.goIS.m4, 0) / elig.length, 3)} OOS ${f(elig.reduce((a, r) => a + r.goOOS.m4, 0) / elig.length, 3)}`);
}
