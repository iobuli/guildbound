import { Graph, type Edge } from './core/graph.js';
import type { AnyNode } from './core/node.js';
import atr from './nodes/atr.js';
import swingDepth from './nodes/swing-depth.js';
import sfp from './nodes/sfp.js';
import dailyBias from './nodes/daily-bias.js';
import atrGate from './nodes/atr-gate.js';
import level from './nodes/level.js';
import levelScan from './nodes/level-scan.js';
import marketSide from './nodes/market-side.js';
import retest from './nodes/retest.js';
import confluence from './nodes/confluence.js';
import news from './nodes/news.js';
import tuesdayRto from './nodes/tuesday-rto.js';
import context from './nodes/context.js';
import stop from './nodes/stop.js';
import target from './nodes/target.js';
import entry from './nodes/entry.js';
import risk from './nodes/risk.js';
import tradeManagement from './nodes/trade-management.js';
import framework from './nodes/framework.js';
import causality from './nodes/causality.js';
import htfPatterns from './nodes/htf-patterns.js';
import equalLiquidity from './nodes/equal-liquidity.js';
import ltfReversal from './nodes/ltf-reversal.js';
import regime from './nodes/regime.js';

export const nodes: AnyNode[] = [
  atr, regime, swingDepth, sfp, dailyBias, atrGate, levelScan, marketSide, level, retest, confluence, news, tuesdayRto, context, stop, target, entry, risk, tradeManagement,
  framework, causality, htfPatterns, equalLiquidity, ltfReversal,
];

/** The only place nodes are connected. A node never names another node. */
export const edges: Edge[] = [
  { from: 'atr.atr', to: 'swing-depth.atr' },
  { from: 'atr.atr', to: 'regime.atr' }, // context only: nothing reads regime's output except the interpreter
  { from: 'atr.atr', to: 'sfp.atr' }, { from: 'swing-depth.swings', to: 'sfp.swings' },
  { from: 'sfp.sfp', to: 'daily-bias.sfp' },
  { from: 'atr.atr', to: 'atr-gate.atr' },
  { from: 'atr.atr', to: 'level.atr' },
  { from: 'atr.atr', to: 'level-scan.atr' }, { from: 'swing-depth.swings', to: 'level-scan.swings' },
  { from: 'market-side.side', to: 'entry.side' },
  { from: 'level.level', to: 'retest.level' }, { from: 'atr.atr', to: 'retest.atr' }, { from: 'daily-bias.bias', to: 'retest.bias' },
  { from: 'level.level', to: 'confluence.level' }, { from: 'daily-bias.bias', to: 'confluence.bias' }, { from: 'atr-gate.gate', to: 'confluence.gate' },
  { from: 'atr.atr', to: 'confluence.atr' }, { from: 'swing-depth.swings', to: 'confluence.swings' },
  { from: 'atr.atr', to: 'news.atr' },
  { from: 'daily-bias.bias', to: 'context.bias' },
  { from: 'level.level', to: 'stop.level' }, { from: 'atr.atr', to: 'stop.atr' }, { from: 'swing-depth.swings', to: 'stop.swings' },
  { from: 'sfp.sfp', to: 'stop.sfp' }, { from: 'tuesday-rto.tuesday', to: 'stop.tuesday' },
  { from: 'stop.stop', to: 'target.stop' }, { from: 'level.level', to: 'target.level' }, { from: 'swing-depth.swings', to: 'target.swings' },
  { from: 'daily-bias.bias', to: 'target.bias' }, { from: 'atr.atr', to: 'target.atr' }, { from: 'tuesday-rto.tuesday', to: 'target.tuesday' },
  { from: 'level.level', to: 'entry.level' }, { from: 'retest.retest', to: 'entry.retest' }, { from: 'confluence.confluence', to: 'entry.confluence' },
  { from: 'atr-gate.gate', to: 'entry.gate' }, { from: 'news.news', to: 'entry.news' }, { from: 'sfp.sfp', to: 'entry.sfp' },
  { from: 'daily-bias.bias', to: 'entry.bias' }, { from: 'tuesday-rto.tuesday', to: 'entry.tuesday' }, { from: 'context.context', to: 'entry.context' },
  { from: 'stop.stop', to: 'risk.stop' },
  { from: 'stop.stop', to: 'management.stop' }, { from: 'target.target', to: 'management.target' }, { from: 'context.context', to: 'management.context' },
];

export const graph = new Graph(nodes, edges);
