// kev's memory (kev-brain/): BM25 search over the vault and every source, notes generated from the code and the data,
// a daily log that runs append to, the session-start context, and an inbox that routes new files.
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import { basename, extname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { nodes } from '../kev.js';
import { listMarkets, loadProfile, MARKETS_DIR } from '../core/market-io.js';

export const BRAIN = fileURLToPath(new URL('../../kev-brain/', import.meta.url));
const iso = (t: number) => new Date(t).toISOString().slice(0, 10);

// ---------------------------------------------------------------- search ----
// chat filler that dominates word counts in the course chat but carries no method content
const CHAT = new Set('image yeah okay bro shit man say fuck course really youre edited fucking curated thats today said gonna little nice nah work doing actually link tho doesnt note com let literally actual theres discord went phone thought quite cant didnt didn maybe yes call find bad lmao guy fact bruv ain probably saying ive naa bike http https www lol haha mate innit wanna gotta dude cheers thanks thank good great sure think know see well right going want need time make way back still even much'.split(' '));
const STOP = new Set('a an and are as at be but by for from has have he her his i if in into is it its me my not of on or our she so that the their them then there these they this to too up us was we were what when which who will with you your just like its im dont get got can would could should also about out all any been more some than only very do did does had how one two'.split(' '));
const tokens = (s: string) => s.toLowerCase().split(/[^a-z0-9]+/).filter((w) => w.length >= 2 && !STOP.has(w));
export interface Chunk { file: string; heading: string; line: number; text: string; tf: Map<string, number>; len: number }
let INDEX: { chunks: Chunk[]; df: Map<string, number>; avg: number } | null = null;

function mdFiles(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.name.startsWith('.') || e.name === 'inbox' ? [] : e.isDirectory() ? mdFiles(`${dir}${e.name}/`) : e.name.endsWith('.md') ? [`${dir}${e.name}`] : []));
}
/** Splits markdown into ~250-word chunks on paragraph boundaries, each remembering the heading above it. */
function chunk(file: string): Chunk[] {
  const lines = readFileSync(file, 'utf8').split('\n'), out: Chunk[] = [];
  let heading = basename(file, '.md'), buf: string[] = [], start = 1, words = 0;
  const flush = () => { const text = buf.join('\n').trim(); if (text) { const tk = tokens(text), tf = new Map<string, number>(); for (const w of tk) tf.set(w, (tf.get(w) ?? 0) + 1); out.push({ file: relative(BRAIN, file), heading, line: start, text, tf, len: tk.length }); } buf = []; words = 0; };
  lines.forEach((l, i) => {
    if (/^#{1,4} /.test(l)) { flush(); heading = l.replace(/^#+ /, '').trim(); start = i + 1; }
    if (!buf.length) start = i + 1;
    buf.push(l); words += l.split(/\s+/).length;
    if ((words >= 250 && !l.trim()) || words >= 350) flush(); // paragraph break after ~250 words, or force one (transcripts have no blank lines)
  });
  flush();
  return out;
}
export function index(rebuild = false) {
  if (INDEX && !rebuild) return INDEX;
  const chunks = mdFiles(BRAIN).flatMap(chunk), df = new Map<string, number>();
  for (const c of chunks) for (const w of c.tf.keys()) df.set(w, (df.get(w) ?? 0) + 1);
  INDEX = { chunks, df, avg: chunks.reduce((a, c) => a + c.len, 0) / Math.max(1, chunks.length) };
  return INDEX;
}
export interface Hit { file: string; heading: string; line: number; score: number; snippet: string; shared: number }
/** BM25 (k1 1.2, b 0.75). `under` limits the search to a folder of the vault, e.g. "sources/". */
export function search(query: string, k = 8, under?: string): Hit[] {
  const ix = index(), q = [...new Set(tokens(query))], N = ix.chunks.length;
  const scored: { c: Chunk; s: number }[] = [];
  for (const c of ix.chunks) {
    if (under && !c.file.startsWith(under)) continue;
    let s = 0;
    for (const w of q) { const f = c.tf.get(w); if (!f) continue; const n = ix.df.get(w)!; s += Math.log(1 + (N - n + 0.5) / (n + 0.5)) * ((f * 2.2) / (f + 1.2 * (0.25 + 0.75 * (c.len / ix.avg)))); }
    if (s > 0) scored.push({ c, s });
  }
  return scored.sort((a, b) => b.s - a.s).slice(0, k).map(({ c, s }) => ({ file: c.file, heading: c.heading, line: c.line, score: +s.toFixed(2), snippet: snippet(c.text, q), shared: q.filter((w) => c.tf.has(w)).length }));
}
function snippet(text: string, q: string[]): string {
  const lines = text.split('\n').filter((l) => l.trim());
  let best = lines[0] ?? '', bs = -1;
  for (const l of lines) { const t = new Set(tokens(l)); const s = q.filter((w) => t.has(w)).length; if (s > bs) { bs = s; best = l; } }
  return best.replace(/\s+/g, ' ').trim().slice(0, 240);
}

// ------------------------------------------------------------- daily log ----
export function log(text: string, t = Date.now()) {
  mkdirSync(`${BRAIN}daily/`, { recursive: true });
  const f = `${BRAIN}daily/${iso(t)}.md`;
  if (!existsSync(f)) writeFileSync(f, `# ${iso(t)}\n\n`);
  appendFileSync(f, `- ${new Date(t).toISOString().slice(11, 16)} UTC: ${text.replace(/\n+/g, ' ')}\n`);
}

// ------------------------------------------------------- session context ----
export function context(maxChars = 12_000): string {
  const read = (p: string) => (existsSync(`${BRAIN}${p}`) ? readFileSync(`${BRAIN}${p}`, 'utf8').trim() : '');
  const days = existsSync(`${BRAIN}daily/`) ? readdirSync(`${BRAIN}daily/`).filter((f) => f.endsWith('.md')).sort().slice(-2) : [];
  const parts = [read('KEV.md'), read('USER.md'), read('MEMORY.md'), ...days.map((d) => read(`daily/${d}`).split('\n').slice(-40).join('\n')), read('findings.md').split('\n').slice(0, 50).join('\n')];
  const s = `<kev-brain>\n${parts.filter(Boolean).join('\n\n---\n\n')}\n</kev-brain>`;
  return s.length > maxChars ? `${s.slice(0, maxChars)}\n…(truncated; read kev-brain/ for the rest)\n</kev-brain>` : s;
}

// ------------------------------------------------------------- generators ----
/** One note per node: what it does, its rules with status and sources, whether code tests them, and the passages that support each rule. */
export function writeMethodNotes() {
  mkdirSync(`${BRAIN}method/`, { recursive: true });
  index(true);
  const weak: string[] = [];
  for (const n of nodes) {
    const L = [`# ${n.title} (\`${n.id}\`)`, '', n.summary, '', `Decides at **${n.decideAt}**${n.step ? `, step ${n.step.n} of the order of operations: ${n.step.label}` : ''}. Inputs: ${Object.keys(n.inputs).join(', ') || 'none'}. Outputs: ${Object.keys(n.outputs).join(', ') || 'none'}.`, ''];
    for (const r of n.rules) {
      const hits = search(`${r.text} ${r.id.split('.').slice(1).join(' ').replace(/-/g, ' ')}`, 3, 'sources/');
      const strong = hits.filter((h) => h.score >= 10 && h.shared >= 4); // a real passage shares several specific words, not one common term
      if (!strong.length) weak.push(`- \`${r.id}\` (${r.status}): ${r.text}`);
      L.push(`## ${r.id}`, '', r.text, '', `*${r.kind}, ${r.status}, ${r.codable ? 'tested by code' : 'not coded'}.* Sources: ${r.sources.map((s) => `${s.kind} ${s.ref}${s.date ? ` (${s.date})` : ''}`).join('; ')}.${r.note ? ` Note: ${r.note}` : ''}`, '');
      L.push(strong.length ? 'Supporting passages:' : 'No strong supporting passage found in the sources (BM25 score < 10 or fewer than 4 shared words).', ...strong.map((h) => `- [${h.file}:${h.line}](../${h.file}) (${h.heading}, score ${h.score}): "${h.snippet}"`), '');
    }
    if (n.conflicts?.length) L.push('## Conflicts', '', ...n.conflicts.map((c) => `- \`${c.a}\` vs \`${c.b}\`: ${c.note}`), '');
    writeFileSync(`${BRAIN}method/${n.id}.md`, L.join('\n'));
  }
  // gaps: method words the course uses often that no rule, node title or summary mentions
  const covered = new Set(tokens(nodes.flatMap((n) => [n.title, n.summary, n.id, ...n.rules.flatMap((r) => [r.text, r.id])]).join(' ')));
  const ix = index(), freq = new Map<string, number>();
  const files = new Map<string, Set<string>>();
  for (const c of ix.chunks) if (c.file.startsWith('sources/')) for (const [w, f] of c.tf) if (!covered.has(w) && !CHAT.has(w) && !/^\d+$/.test(w) && w.length > 2) { freq.set(w, (freq.get(w) ?? 0) + f); (files.get(w) ?? files.set(w, new Set()).get(w)!).add(c.file); }
  for (const w of [...freq.keys()]) if ((files.get(w)?.size ?? 0) < 3) freq.delete(w); // must appear in 3+ source files
  const top = [...freq.entries()].sort((a, b) => b[1] - a[1]).slice(0, 80);
  writeFileSync(`${BRAIN}method/_gaps.md`, ['# Gaps between the sources and the code', '', `Generated ${iso(Date.now())}.`, '', '## Rules with no strong supporting passage', '', 'These may still be right (the source can be a transcript timestamp or an image), but the text search could not find them.', '', ...(weak.length ? weak : ['- none']), '', '## Words the sources use often that no rule mentions', '', 'Candidates for missing rules, or vocabulary to add to existing ones. Count = occurrences across the sources.', '',
    top.map(([w, f]) => `\`${w}\` ${f}`).join(' · '), ''].join('\n'));
  writeFileSync(`${BRAIN}method/README.md`, ['# Method', '', 'One note per node, generated from `src/nodes/*.ts` by `kev brain build`. Do not edit by hand: change the node, then rebuild.', '', ...nodes.map((n) => `- [${n.title}](${n.id}.md): ${n.rules.length} rules, ${n.rules.filter((r) => r.codable).length} coded`), '', `See also [_gaps.md](_gaps.md).`, ''].join('\n'));
  return { nodes: nodes.length, weak: weak.length };
}

/** One note per ingested market: profile, clock, session, costs, split, files, and the ingest notes. */
export function writeMarketNotes() {
  mkdirSync(`${BRAIN}markets/`, { recursive: true });
  for (const sym of listMarkets()) {
    const p = loadProfile(sym), ing = existsSync(`${MARKETS_DIR}${sym}/ingest.md`) ? readFileSync(`${MARKETS_DIR}${sym}/ingest.md`, 'utf8').split('\n').slice(1).join('\n') : '';
    const ov = existsSync(`${MARKETS_DIR}${sym}/override.json`) ? readFileSync(`${MARKETS_DIR}${sym}/override.json`, 'utf8').trim() : '';
    writeFileSync(`${BRAIN}markets/${sym}.md`, [`# ${sym}`, '', `class ${p.cls}; ${iso(p.data.from)} to ${iso(p.data.to)}; clock ${p.clock.kind === 'fixed' ? `UTC${p.clock.offsetHours >= 0 ? '+' : ''}${p.clock.offsetHours}` : 'New York + 7h'} (${p.clock.method}).`,
      `Session: ${p.session.day} days, stubs merged if <= ${p.session.stubHours}h, trader clock ${p.session.traderTz}. Costs: ${p.costs.spreadBps} bps + ${p.costs.slipR}R (${p.costs.note}). In-sample <= ${p.split.isEndYear}.`,
      ov ? `\nOverrides (markets/${sym}/override.json): \`${ov.replace(/\s+/g, ' ')}\`` : '', '', ing].join('\n'));
  }
}

/** Index of every source with its size, so a reader knows what exists before searching. */
export function writeSourceIndex() {
  const files = mdFiles(`${BRAIN}sources/`).filter((f) => !f.endsWith('sources/README.md'));
  const L = ['# Sources', '', 'Everything the method was built from. Search with `kev brain search "..."`. Course text has names, handles and IDs removed.', '', '| file | words |', '|---|---|'];
  for (const f of files) L.push(`| [${relative(`${BRAIN}sources/`, f)}](${relative(`${BRAIN}sources/`, f)}) | ${readFileSync(f, 'utf8').split(/\s+/).length} |`);
  writeFileSync(`${BRAIN}sources/README.md`, L.join('\n') + '\n');
  return files.length;
}

// ----------------------------------------------------------------- inbox ----
const PRICE_EXT = new Set(['.csv', '.tsv', '.txt', '.json']);
/** Guesses the market from a file name: "6f384fbe-ETHUSD5.csv" -> ETHUSD, "SOLUSDT_1h.json" -> SOLUSDT. */
export const symbolOf = (name: string) => basename(name, extname(name)).replace(/^[0-9a-f]{8}-/i, '').replace(/[_-]?(\d+[mhdw]?|[mhdw]\d+|1h|4h|1d)?(_\d+)?$/i, '').toUpperCase();

/** Routes every file in inbox/: price files go to markets/<SYM>/raw and the market is re-ingested; pages and notes go to sources/. */
export async function processInbox(ingestFn: (sym: string, files: string[]) => Promise<unknown>) {
  const dir = `${BRAIN}inbox/`, done: string[] = [];
  if (!existsSync(dir)) return done;
  const files = readdirSync(dir).filter((f) => !f.startsWith('.') && statSync(`${dir}${f}`).isFile());
  const bySym = new Map<string, string[]>();
  for (const f of files) {
    const ext = extname(f).toLowerCase(), head = readFileSync(`${dir}${f}`, 'utf8').slice(0, 400);
    const looksPrice = PRICE_EXT.has(ext) && /\d{4}[-./]\d{2}[-./]\d{2}|^\s*\[\s*[[{]|^\d{12,13},/m.test(head) && (head.match(/[\d.]+/g)?.length ?? 0) > 10;
    if (looksPrice) { const s = symbolOf(f); (bySym.get(s) ?? bySym.set(s, []).get(s)!).push(f); continue; }
    if (ext === '.html' || ext === '.htm') {
      const html = readFileSync(`${dir}${f}`, 'utf8'), name = basename(f, ext).replace(/^[0-9a-f]{8}-/i, '');
      mkdirSync(`${BRAIN}sources/pages/`, { recursive: true });
      const text = html.replace(/<script[\s\S]*?<\/script>/gi, '').replace(/<style[\s\S]*?<\/style>/gi, '').replace(/<[^>]+>/g, ' ').replace(/&[a-z]+;/g, ' ').replace(/\s+/g, ' ').trim();
      writeFileSync(`${BRAIN}sources/pages/${name}.md`, `# ${name} (page text)\n\nAdded from inbox on ${iso(Date.now())}. Embedded data, if any, is in ${name}.data.json.\n\n${text}\n`);
      const m = /(?:const|var|let)\s+D\s*=\s*(\{[\s\S]*?\});\s*\n/.exec(html);
      if (m) { try { JSON.parse(m[1]!); writeFileSync(`${BRAIN}sources/pages/${name}.data.json`, m[1]!); } catch { /* not plain JSON */ } }
      renameSync(`${dir}${f}`, `${BRAIN}sources/pages/${name}${ext}`);
      done.push(`page ${f} -> sources/pages/${name}.md`);
      continue;
    }
    if (ext === '.md' || ext === '.txt') { mkdirSync(`${BRAIN}sources/notes/`, { recursive: true }); renameSync(`${dir}${f}`, `${BRAIN}sources/notes/${basename(f, ext)}.md`); done.push(`note ${f} -> sources/notes/`); continue; }
    done.push(`left in inbox (unrecognised): ${f}`);
  }
  for (const [sym, fs] of bySym) {
    const raw = `${MARKETS_DIR}${sym}/raw/`;
    mkdirSync(raw, { recursive: true });
    for (const f of fs) renameSync(`${dir}${f}`, `${raw}${f}`);
    await ingestFn(sym, readdirSync(raw).map((f) => `${raw}${f}`));
    done.push(`prices ${fs.join(', ')} -> markets/${sym} (re-ingested)`);
  }
  if (done.length) log(`inbox processed: ${done.join('; ')}`);
  return done;
}
