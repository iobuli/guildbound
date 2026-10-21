// One entry point for everything kev does with data: `npx tsx src/cli.ts <command> ...`
import { ingest } from './data/ingest.js';
import { listMarkets, loadProfile } from './core/market-io.js';
import { PRESETS } from './core/presets.js';
import { runMarket } from './study/run.js';
import { cell, table } from './study/analyze.js';
import { auditPages } from './study/audit-pages.js';
import { analyse, runJobs } from './study/hypotheses.js';
import { buildInspector } from './viz/build.js';
import { interpret } from './interpret.js';
import { atlas, summary } from './study/atlas.js';
import { auditKev } from './study/audit-kev.js';
import { openMarket } from './core/market-io.js';
import { context, log, processInbox, search, writeMarketNotes, writeMethodNotes, writeSourceIndex } from './brain/brain.js';

const [cmd, ...rest] = process.argv.slice(2);
const flags = new Set(rest.filter((a) => a.startsWith('--')));
const opt = (k: string) => rest.find((a) => a.startsWith(`--${k}=`))?.slice(k.length + 3);
const date = (s?: string) => (s ? Date.parse(s.length === 4 ? `${s}-01-01` : s) : undefined);
const args = rest.filter((a) => !a.startsWith('--'));
const HELP = `kev commands:
  ingest SYMBOL FILE...    any CSV/TSV/JSON price files for one market -> markets/SYMBOL (add --no-net to skip the Binance clock check)
  markets                  list ingested markets with their profile
  brain build              regenerate method/, markets/ and sources/ notes from the code and data
  brain search "QUERY"     BM25 search over the vault and every source (--k=N, --in=sources/)
  brain context            what the SessionStart hook injects
  brain log "TEXT"         append to today's daily log
  brain process            route files dropped in kev-brain/inbox (prices -> ingest, pages/notes -> sources)
  study [--force]          run the pre-registered cross-market hypotheses (H1-H5) and write results to the brain
  atlas SYMBOL --from=YYYY --to=YYYY [--preset=...]   replay every hour of a window, follow every level to the end of its life, phases and stats
  interpret SYMBOL [--at=ISO]   kev's layered reading (regime, bias, location, structure, decision) at a moment
  inspect                  build out/inspector.html (every market, the user's pages, the study results)
  audit-pages              re-check the user's Trade Check and One Hour pages against the market data
  backtest SYMBOL          causal harness + matched placebos on one market
                           --preset=${Object.keys(PRESETS).join('|')}  --from=YYYY[-MM-DD]  --to=...  --session=utc|ny-close|utc-raw  --no-ltf`;

async function main() {
  switch (cmd) {
    case 'ingest': {
      const [sym, ...files] = args;
      if (!sym || !files.length) throw new Error('usage: ingest SYMBOL FILE...');
      const r = await ingest(sym.toUpperCase(), files, { net: !flags.has('--no-net') });
      console.log(r.report);
      log(`ingested ${sym.toUpperCase()} from ${files.length} file(s): ${r.profile.notes.length} notes, clock ${r.profile.clock.method}`);
      break;
    }
    case 'markets':
      for (const m of listMarkets()) { const p = loadProfile(m); console.log(`${m.padEnd(10)} ${p.cls.padEnd(9)} ${new Date(p.data.from).toISOString().slice(0, 10)} -> ${new Date(p.data.to).toISOString().slice(0, 10)}  session ${p.session.day}  clock ${p.clock.kind}${p.clock.kind === 'fixed' ? `+${p.clock.offsetHours}` : ''}  ltf ${p.data.ltf.map((x) => x.file).join(',') || '-'}  notes ${p.notes.length}`); }
      break;
    case 'brain': {
      const sub = args[0];
      if (sub === 'build') { const m = writeMethodNotes(); writeMarketNotes(); const n = writeSourceIndex(); console.log(`method notes for ${m.nodes} nodes (${m.weak} rules without a strong supporting passage), ${listMarkets().length} market notes, ${n} sources indexed`); log(`brain build: ${m.nodes} method notes, ${m.weak} weakly supported rules`); }
      else if (sub === 'search') { for (const h of search(args.slice(1).join(' '), Number(opt('k') ?? 8), opt('in'))) console.log(`${h.score.toFixed(1).padStart(5)}  ${h.file}:${h.line}  [${h.heading}]\n       ${h.snippet}`); }
      else if (sub === 'context') console.log(context());
      else if (sub === 'log') { log(args.slice(1).join(' ')); console.log('logged'); }
      else if (sub === 'process') { const d = await processInbox((sym, files) => ingest(sym, files, { net: !flags.has('--no-net') })); console.log(d.length ? d.join('\n') : 'inbox is empty'); }
      else console.log(HELP);
      break;
    }
    case 'audit-pages': console.log(auditPages()); break;
    case 'audit-kev': console.log(auditKev((s) => console.error(s))); break;
    case 'inspect': console.log(buildInspector()); break;
    case 'atlas': {
      const sym = (args[0] ?? 'SOLUSDT').toUpperCase(), y0 = Number(opt('from') ?? 2020), y1 = Number(opt('to') ?? 2022);
      const a = atlas(sym, Date.UTC(y0, 0, 1), Date.UTC(y1 + 1, 0, 1), opt('preset') ?? 'kev-default', (s) => console.error(s));
      console.log(summary(a));
      break;
    }
    case 'interpret': {
      const m = openMarket((args[0] ?? 'SOLUSDT').toUpperCase()), at = date(opt('at')) ?? m.h1.at(-1)!.t + 3_600_000;
      const r = interpret(m.feed(), at, { instrument: m.instrument });
      console.log(`${m.profile.symbol} ${new Date(at).toISOString().slice(0, 16)} UTC: ${r.headline}\n`);
      for (const l of r.layers) console.log(`${l.title}\n  ${l.text}\n`);
      console.log('salience: ' + Object.entries(r.salience).filter(([, v]) => v >= 0.5).sort((a, b) => b[1] - a[1]).map(([k, v]) => `${k} ${v.toFixed(2)}`).join(', '));
      break;
    }
    case 'study': await runJobs(flags.has('--force')); console.log(analyse()); break;
    case 'backtest': {
      const sym = args[0]?.toUpperCase();
      if (!sym) throw new Error('usage: backtest SYMBOL [--preset=...]');
      const r = runMarket(sym, { preset: opt('preset') ?? 'kev-default', session: opt('session') as never, ltf: !flags.has('--no-ltf'), from: date(opt('from')), to: date(opt('to')) }, (s) => console.error(s));
      const isEnd = r.split.isEndYear, all = () => true;
      const cells = [
        cell('all fills', r.trades, r.placebo, 'S0.5_T2', all), cell('GO at fill', r.trades, r.placebo, 'S0.5_T2', (t) => t.go),
        cell('space ok (prior bars)', r.trades, r.placebo, 'S0.5_T2', (t) => t.spaceOk), cell('touches >= 2', r.trades, r.placebo, 'S0.5_T2', (t) => t.touches >= 2),
        cell(`in-sample <= ${isEnd}`, r.trades, r.placebo, 'S0.5_T2', (t) => t.year <= isEnd), cell('out-of-sample', r.trades, r.placebo, 'S0.5_T2', (t) => t.year > isEnd),
      ];
      console.log(`${sym} ${r.preset}: ${r.trades.length} fills, ${r.placebo.length} placebos, ${r.scans} scans, ${r.seconds.toFixed(0)}s. Outcome S0.5_T2 (stop 0.5 x daily ATR, 2R, net of costs).\n`);
      console.log(table(cells));
      break;
    }
    default: console.log(HELP);
  }
}
main().catch((e) => { console.error(String(e?.message ?? e)); process.exit(1); });
