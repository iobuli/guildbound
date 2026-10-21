// One entry point for everything kev does with data: `npx tsx src/cli.ts <command> ...`
import { ingest } from './data/ingest.js';
import { listMarkets, loadProfile } from './core/market-io.js';

const [cmd, ...rest] = process.argv.slice(2);
const flags = new Set(rest.filter((a) => a.startsWith('--')));
const args = rest.filter((a) => !a.startsWith('--'));
const HELP = `kev commands:
  ingest SYMBOL FILE...    any CSV/TSV/JSON price files for one market -> markets/SYMBOL (add --no-net to skip the Binance clock check)
  markets                  list ingested markets with their profile`;

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
    default: console.log(HELP);
  }
}
main().catch((e) => { console.error(String(e?.message ?? e)); process.exit(1); });
