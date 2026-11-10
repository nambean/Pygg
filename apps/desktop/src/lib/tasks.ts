/**
 * Task model as the interface sees it, and the sidebar filters over it (docs/design/SCREENS.md §2.2, §5.1).
 * The Rust core will own these records; until it exists the list is filled from sample data (sample.ts).
 */

export type Tier = 0 | 1 | 2 | 3 | 4;
export type Billing = 'free' | 'local' | 'subscription' | 'paid' | 'unknown';

export type TaskState =
  | 'running'            // preparing, generating or checking an attempt
  | 'waiting_approval'   // paused before a paid attempt
  | 'needs_revalidation' // planned, but the catalog, a connection or the workspace changed
  | 'checks_passed'
  | 'manually_accepted'
  | 'check_failed'
  | 'blocked'            // no eligible model, or the policy or budget allows nothing more
  | 'canceled'
  | 'interrupted';       // the app stopped while it ran; needs reconciliation

export type AttemptOutcome = 'passed' | 'failed' | 'running' | 'waiting' | 'canceled';

export type Attempt = {
  tier: Tier;
  billing: Billing;
  /** Provider / Account · Model (SCREENS.md §3.1). */
  model: string;
  outcome: AttemptOutcome;
};

/** Money is kept apart by certainty (SCREENS.md §3.3): unknown is never folded into a total. */
export type TaskCost = {
  actualUsd: number;
  estimatedUsd: number;
  unknownCalls: number;
  /** Every attempt so far ran on a local model: API cost is $0 by construction. */
  localOnly: boolean;
};

export type RunningPhase = 'preparing' | 'generating' | 'checking';

export type TaskSummary = {
  id: string;
  title: string;
  workspaceId: string;
  state: TaskState;
  phase?: RunningPhase;
  attempts: Attempt[];
  cost: TaskCost;
  /** Unix milliseconds. */
  startedAt: number;
  updatedAt: number;
};

export type Workspace = { id: string; name: string; path: string };

export type TaskFilter = 'all' | 'running' | 'approval' | 'completed' | 'failed' | 'stopped';

export const FILTERS: TaskFilter[] = ['all', 'running', 'approval', 'completed', 'failed', 'stopped'];

export function matchesFilter(task: TaskSummary, filter: TaskFilter): boolean {
  switch (filter) {
    case 'all': return true;
    // Running excludes approval waits: those have their own filter.
    case 'running': return task.state === 'running';
    case 'approval': return task.state === 'waiting_approval';
    case 'completed': return task.state === 'checks_passed' || task.state === 'manually_accepted';
    case 'failed': return task.state === 'check_failed' || task.state === 'blocked';
    case 'stopped': return task.state === 'canceled' || task.state === 'interrupted';
  }
}

/** Tasks that wait on the user; pinned above the list and not repeated in it. */
export function needsAttention(task: TaskSummary): boolean {
  return task.state === 'waiting_approval' || task.state === 'interrupted' || task.state === 'needs_revalidation';
}

export type DayBudget = {
  /** Recorded spend today: actual plus estimated, in USD. */
  recordedUsd: number;
  /** Capacity held for running attempts; not spend. */
  reservedUsd: number;
  limitUsd: number;
  /** Calls whose cost is not known make the remaining budget uncertain. */
  unknownCalls: number;
};
