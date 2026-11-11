/**
 * Sample tasks for the browser preview (pnpm dev), which has no task engine; the preview says so on every
 * screen. Agents, models, prices and results are illustrative, not measurements.
 */
import type { DayBudget, TaskSummary, Workspace } from './tasks';

const minute = 60_000;

export const SAMPLE_WORKSPACES: Workspace[] = [
  {id: 'ws-str', name: 'smarttaskrouter', path: 'F:\\2026\\smarttaskrouter'},
  {id: 'ws-docs', name: 'docs-site', path: 'F:\\2026\\docs-site'},
];

const LOCAL = 'OpenCode · ollama/qwen2.5-coder:7b';
const MINI = 'OpenCode · openrouter/openai/gpt-4.1-mini';
const MID = 'Claude Code · default model';

export function sampleTasks(now = Date.now()): TaskSummary[] {
  const at = (minutesAgo: number) => now - minutesAgo * minute;
  return [
    {
      id: 't-101', title: 'Fix timeout handling and add regression tests', workspaceId: 'ws-str',
      state: 'waiting_approval',
      attempts: [{tier: 0, billing: 'local', model: LOCAL, outcome: 'failed'},
        {tier: 1, billing: 'paid', model: MINI, outcome: 'waiting'}],
      cost: {actualUsd: 0, estimatedUsd: 0, unknownCalls: 0, localOnly: true},
      startedAt: at(6), updatedAt: at(2),
    },
    {
      id: 't-102', title: 'Rename settings keys to camelCase', workspaceId: 'ws-docs',
      state: 'interrupted',
      attempts: [{tier: 1, billing: 'free', model: 'Gemini CLI · gemini-2.5-flash', outcome: 'canceled'}],
      cost: {actualUsd: 0, estimatedUsd: 0, unknownCalls: 1, localOnly: false},
      startedAt: at(95), updatedAt: at(90),
    },
    {
      id: 't-103', title: 'Add retry tests for the provider adapter', workspaceId: 'ws-str',
      state: 'running', phase: 'checking',
      attempts: [{tier: 0, billing: 'local', model: LOCAL, outcome: 'running'}],
      cost: {actualUsd: 0, estimatedUsd: 0, unknownCalls: 0, localOnly: true},
      startedAt: at(1.2), updatedAt: at(0.1),
    },
    {
      id: 't-104', title: 'Update parser for the new config format', workspaceId: 'ws-str',
      state: 'running', phase: 'generating',
      attempts: [{tier: 1, billing: 'paid', model: MINI, outcome: 'failed'},
        {tier: 2, billing: 'paid', model: MID, outcome: 'running'}],
      cost: {actualUsd: 0.02, estimatedUsd: 0.01, unknownCalls: 0, localOnly: false},
      startedAt: at(4), updatedAt: at(0.2),
    },
    {
      id: 't-105', title: 'Update README with build commands', workspaceId: 'ws-docs',
      state: 'checks_passed',
      attempts: [{tier: 0, billing: 'local', model: LOCAL, outcome: 'passed'}],
      cost: {actualUsd: 0, estimatedUsd: 0, unknownCalls: 0, localOnly: true},
      startedAt: at(12), updatedAt: at(8),
    },
    {
      id: 't-106', title: 'Split router module into eligibility and selection', workspaceId: 'ws-str',
      state: 'checks_passed',
      attempts: [{tier: 1, billing: 'paid', model: MINI, outcome: 'failed'},
        {tier: 2, billing: 'paid', model: MID, outcome: 'passed'}],
      cost: {actualUsd: 0.08, estimatedUsd: 0, unknownCalls: 0, localOnly: false},
      startedAt: at(64), updatedAt: at(58),
    },
    {
      id: 't-107', title: 'Explain the ledger schema in ARCHITECTURE.md', workspaceId: 'ws-str',
      state: 'manually_accepted',
      attempts: [{tier: 2, billing: 'subscription', model: 'Codex CLI · default model', outcome: 'passed'}],
      cost: {actualUsd: 0, estimatedUsd: 0, unknownCalls: 1, localOnly: false},
      startedAt: at(180), updatedAt: at(171),
    },
    {
      id: 't-108', title: 'Migrate tests to the new fixture layout', workspaceId: 'ws-str',
      state: 'check_failed',
      attempts: [{tier: 1, billing: 'paid', model: MINI, outcome: 'failed'},
        {tier: 2, billing: 'paid', model: MID, outcome: 'failed'},
        {tier: 3, billing: 'paid', model: 'Claude Code · opus', outcome: 'failed'}],
      cost: {actualUsd: 0.41, estimatedUsd: 0, unknownCalls: 0, localOnly: false},
      startedAt: at(300), updatedAt: at(282),
    },
    {
      id: 't-109', title: 'Rename config key in docs examples', workspaceId: 'ws-docs',
      state: 'canceled',
      attempts: [{tier: 1, billing: 'paid', model: MINI, outcome: 'canceled'}],
      cost: {actualUsd: 0.02, estimatedUsd: 0, unknownCalls: 0, localOnly: false},
      startedAt: at(30), updatedAt: at(23),
    },
  ];
}

export const SAMPLE_BUDGET: DayBudget = {recordedUsd: 3.42, reservedUsd: 0.3, limitUsd: 10, unknownCalls: 0};
