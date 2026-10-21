import { mkdirSync, writeFileSync } from 'node:fs';
import { z } from 'zod';
import { nodes, edges, graph } from './kev.js';
import { GLOSSARY } from './glossary.js';
import { KNOWABLE } from './core/types.js';

const out = new URL('../docs/', import.meta.url);
mkdirSync(new URL('nodes/', out), { recursive: true });
const shape = (schema: unknown) => {
  const js = z.toJSONSchema(schema as z.ZodType) as { properties?: Record<string, { type?: string; enum?: string[]; items?: { type?: string } }> };
  return Object.entries(js.properties ?? {}).map(([k, v]) => `${k}: ${v.enum ? v.enum.join('|') : v.type === 'array' ? `${v.items?.type ?? 'object'}[]` : v.type ?? 'object'}`).join(', ');
};
const esc = (s: string) => s.replace(/\|/g, '\\|').replace(/\n/g, ' ');
const src = (r: { kind: string; ref: string; date?: string }) => `${r.kind}: ${r.ref}${r.date ? ` (${r.date})` : ''}`;

for (const n of nodes) {
  const ins = Object.entries(n.inputs).map(([k, v]) => `- \`${k}\` (from ${edges.find((e) => e.to === `${n.id}.${k}`)?.from ?? '?'}): ${shape(v)}`);
  const outs = Object.entries(n.outputs).map(([k, v]) => `- \`${k}\`: ${shape(v)}`);
  const md = [
    `# ${n.title}`, '',
    `\`${n.id}\` · stage **${n.stage}** · decides at **${n.decideAt}** · ${n.evaluate ? 'computed' : 'rule card (nothing to compute)'}`, '',
    n.summary, '',
    '## Ports', '', '**Inputs**', ...(ins.length ? ins : ['- none']), '', '**Outputs**', ...(outs.length ? outs : ['- none']), '',
    '## Rules', '', '| Rule | Kind | Status | Coded | Sources |', '|---|---|---|---|---|',
    ...n.rules.map((r) => `| **${r.id}**<br>${esc(r.text)}${r.note ? `<br>*${esc(r.note)}*` : ''} | ${r.kind} | ${r.status} | ${r.codable ? 'yes' : 'no'} | ${r.sources.map(src).map(esc).join('<br>')} |`),
    ...(n.conflicts?.length ? ['', '## Conflicts', '', ...n.conflicts.map((c) => `- \`${c.a}\` vs \`${c.b}\`: ${c.note}`)] : []), '',
  ].join('\n');
  writeFileSync(new URL(`nodes/${n.id}.md`, out), md);
}

const stages: Record<string, string> = { D: 'Direction', O: 'Objection', L: 'Location', E: 'Execution', M: 'Management', R: 'Risk', X: 'Context' };
const mm = ['```mermaid', 'flowchart LR'];
for (const [k, label] of Object.entries(stages)) {
  const ns = nodes.filter((n) => n.stage === k);
  if (!ns.length) continue;
  mm.push(`  subgraph ${k}["${k} · ${label}"]`, ...ns.map((n) => `    ${n.id.replace(/-/g, '_')}["${n.title}<br/><i>${n.decideAt}</i>"]`), '  end');
}
for (const e of edges) mm.push(`  ${e.from.split('.')[0]!.replace(/-/g, '_')} --> ${e.to.split('.')[0]!.replace(/-/g, '_')}`);
mm.push('```');
writeFileSync(new URL('graph.md', out), [
  '# The node graph', '',
  `${nodes.length} nodes, ${edges.length} wires. Order of evaluation: ${graph.order().join(' → ')}.`, '',
  `Every wire is checked at load: the producing node must be knowable no later than the consuming node decides. Phases, earliest to latest: ${KNOWABLE.join(' → ')}.`, '',
  ...mm, '',
  'Rule-only cards (no wires): ' + nodes.filter((n) => !n.evaluate).map((n) => `\`${n.id}\``).join(', '), '',
].join('\n'));

const conflicts = nodes.flatMap((n) => (n.conflicts ?? []).map((c) => ({ node: n.id, ...c })));
writeFileSync(new URL('conflicts.md', out), ['# Where the sources disagree', '', 'These are kept, not resolved. Each node still reports the rule it owns.', '',
  ...conflicts.map((c) => `## ${c.a} vs ${c.b}\n\nOwned by \`${c.node}\`.\n\n${c.note}\n`)].join('\n'));

writeFileSync(new URL('glossary.md', out), ['# Glossary', '', ...Object.entries(GLOSSARY).map(([k, v]) => `- **${k}**: ${v}`), ''].join('\n'));

const rules = nodes.flatMap((n) => n.rules.map((r) => ({ node: n.id, ...r })));
const count = <T,>(xs: T[], f: (x: T) => string) => Object.entries(xs.reduce<Record<string, number>>((m, x) => ((m[f(x)] = (m[f(x)] ?? 0) + 1), m), {})).sort((a, b) => b[1] - a[1]);
writeFileSync(new URL('sources.md', out), ['# What the knowledge rests on', '',
  `${rules.length} rules, ${rules.filter((r) => r.codable).length} of them tested by code.`, '',
  '## By status', '', ...count(rules, (r) => r.status).map(([k, v]) => `- ${k}: ${v}`), '',
  '## By source kind (a rule with several sources counts under each)', '', ...count(rules.flatMap((r) => r.sources), (s) => s.kind).map(([k, v]) => `- ${k}: ${v}`), '',
  '## Reading the status column', '',
  '- **stated**: the trader said it as a rule (via a note-taker\'s record, a lesson, or a post).',
  '- **observed**: seen repeatedly in his trades but not stated as a rule.',
  '- **inferred**: a note-taker\'s guess or a threshold this build chose so the rule can be computed.',
  '- **backtest-reported / backtest-rejected**: from the user-supplied backtest pages. Not reproduced here.', '',
  'The source is a third party\'s notes of a discretionary trader. None of it has been tested on SOL or crypto.', ''].join('\n'));
console.log(`wrote ${nodes.length} node pages + graph, conflicts, glossary, sources`);
