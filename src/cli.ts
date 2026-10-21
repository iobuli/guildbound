// One entry point for everything kev does with data: `npx tsx src/cli.ts <command> ...`
import { ingest } from './data/ingest.js';
import { listMarkets, loadProfile } from './core/market-io.js';
import { PRESETS } from './core/presets.js';
import { runMarket } from './study/run.js';
import { cell, table } from './study/analyze.js';

const [cmd, ...rest] = process.argv.slice(2);
const flags = new Set(rest.filter((a) => a.startsWith('--')));
const opt = (k: string) => rest.find((a) => a.startsWith(`--${k}=`))?.slice(k.length + 3);
const date = (s?: string) => (s ? Date.parse(s.length === 4 ? `${s}-01-01` : s) : undefined);
const args = rest.filter((a) => !a.startsWith('--'));
const HELP = `kev commands:
  ingest SYMBOL FILE...    any CSV/TSV/JSON price files for one market -> markets/SYMBOL (add --no-net to skip the Binance clock check)
  markets                  list ingested markets with their profile
  backtest SYMBOL          causal harness + matched placebos on one market
                           --preset=${Object.keys(PRESETS).join('|')}  --from=YYYY[-MM-DD]  --to=...  --session=utc|ny-close|utc-raw  --no-ltf`;

async function main() {
  switch (cmd) {
    case 'ingest': {
      const [sym, ...files] = args;
      if (!sym || !files.length) throw new Error('usage: ingest SYMBOL FILE...');
      const r = await ingest(sym.toUpperCase(), files, { net: !flags.has('--no-net') });
      console.log(r.report);
      break;
    }
    case 'markets':
      for (const m of listMarkets()) { const p = loadProfile(m); console.log(`${m.padEnd(10)} ${p.cls.padEnd(9)} ${new Date(p.data.from).toISOString().slice(0, 10)} -> ${new Date(p.data.to).toISOString().slice(0, 10)}  session ${p.session.day}  clock ${p.clock.kind}${p.clock.kind === 'fixed' ? `+${p.clock.offsetHours}` : ''}  ltf ${p.data.ltf.map((x) => x.file).join(',') || '-'}  notes ${p.notes.length}`); }
      break;
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
