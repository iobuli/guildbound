import { z } from 'zod';
import type { Check, Conflict, Knowable, Rule, Setup, Stage, View } from './types.js';

type Shape = Record<string, z.ZodType>;
type Infer<S extends Shape> = { [K in keyof S]: z.infer<S[K]> };

/**
 * A node is one isolated unit of the method: typed input ports, typed output ports, the rules it owns
 * (each with provenance), and optionally an evaluate() for the part that can be computed.
 * A node file may import only from core/. Data reaches it through its input ports, and nothing else.
 */
export interface NodeDef<I extends Shape, O extends Shape> {
  id: string;
  title: string;
  stage: Stage;
  summary: string;
  decideAt: Knowable;
  /** Position in the trader's order of operations (shown by Graph.flow). Feeder nodes use fractions. */
  step?: { n: number; label: string };
  inputs: I;
  outputs: O;
  rules: Rule[];
  conflicts?: Conflict[];
  evaluate?: (inp: Infer<I>, view: View, setup: Setup) => { out: Infer<O>; checks?: Check[] };
}
export type AnyNode = NodeDef<any, any>;

export function defineNode<I extends Shape, O extends Shape>(def: NodeDef<I, O>): NodeDef<I, O> {
  return def;
}
