import type { Rule, SourceKind, SourceRef } from './types.js';

export const src = (kind: SourceKind, ref: string, date?: string): SourceRef => ({ kind, ref, date });

/** Shorthand for a rule: defaults to a hard, stated, not-yet-coded rule. */
export function rule(id: string, text: string, o: Partial<Omit<Rule, 'id' | 'text'>> & { sources: SourceRef[] }): Rule {
  return { id, text, kind: 'hard', status: 'stated', codable: false, ...o };
}
