import { graph } from './kev.js';
const errs = graph.validate();
console.log(errs.length ? errs.join('\n') : `graph valid: ${graph.nodes.length} nodes, ${graph.edges.length} wires`);
console.log('order:', graph.order().join(' > '));
process.exit(errs.length ? 1 : 0);
