/**
 * Task model as the interface sees it, and the sidebar filters over it (docs/design/SCREENS.md §2.2, §5.1).
 * The Rust core owns these records (src-tauri/src/tasks.rs); the browser preview fills them from sample data (sample.ts).
 */

export type Tier = 0 | 1 | 2 | 3 | 4;
export type Billing = 'free' | 'local' | 'subscription' | 'paid' | 'unknown';

export type TaskState =
  | 'running'            // preparing, generating or checking an attempt
  | 'waiting_approval'   // paused before a paid attempt
  | 'needs_revalidation' // planned, but the catalog, a connection or the workspace changed
  | 'checks_passed'
  | 'needs_review'       // finished, but nothing confirmed it (no checks, or it changed what defines them)
  | 'manually_accepted'
  | 'check_failed'
  | 'blocked'            // no eligible model, or the policy or budget allows nothing more
  | 'canceled'
  | 'interrupted';       // the app stopped while it ran; needs reconciliation

/** done: the attempt finished and no check judged it. */
export type AttemptOutcome = 'passed' | 'done' | 'failed' | 'running' | 'waiting' | 'canceled';

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
  /** The client that started it through the task API (an IDE agent via pygg-mcp); null: the app. */
  source?: string | null;
};

export type Workspace = { id: string; name: string; path: string };

/** A check command; only approved ones run. Detected ones arrive as suggestions. */
export type WorkspaceCheck = { command: string; source: string; approved: boolean };
export type WorkspaceView = Workspace & {
  checks: WorkspaceCheck[]; tasks: number;
  /** This workspace's own daily limit for paid steps, on top of the overall one. */
  dailyBudgetUsd: number | null;
};

export type Strategy = 'free_first' | 'balanced' | 'quality';

/** One step of a task's ladder: an agent with its own sign-in, the model it is asked for, how the user classes it. */
export type Step = { agent: string; model: string | null; tier: Tier; billing: Billing };

export type NewTask = {
  workspaceId: string; title: string | null; prompt: string; strategy: Strategy; steps: Step[];
  allowShell: boolean; timeoutSecs: number | null;
};

export type CheckResult = { command: string; passed: boolean; detail: string; outputTail: string; durationMs: number };

export type AttemptView = {
  id: string; number: number; step: number; agent: string; label: string; agentModel: string | null;
  tier: Tier; billing: Billing; outcome: AttemptOutcome; detail: string | null; escalationReason: string | null;
  checks: CheckResult[]; filesChanged: number | null; insertions: number | null; deletions: number | null;
  summary: string | null; logTail: string | null; inputTokens: number | null; outputTokens: number | null;
  startedAt: number | null; finishedAt: number | null;
  /** Its worktree is still there: the result waits for the user. */
  open: boolean;
  /** The agent's own list-price estimate for the run, when it gave one. */
  agentEstimateUsd: number | null;
  /** The cap the user approved, for a paid step. */
  capUsd: number | null;
};

/** What earlier runs of the agent and model cost by its own report; null without history. */
export type Estimate = { runs: number; lowMicroUsd: number | null; highMicroUsd: number | null };

/** A paid step waiting for the user's decision (ADR-010: one attempt, one cap). */
export type ApprovalView = {
  step: number; label: string; estimate: Estimate; suggestedCapUsd: number;
  /** The agent stops at the cap itself (Claude Code); otherwise it is checked against the cost it reports. */
  enforcedByAgent: boolean;
  /** What today's budgets leave for paid steps here; null without a budget. The cap cannot be more. */
  remainingUsd: number | null;
};

export type TaskDetail = TaskSummary & {
  prompt: string; strategy: Strategy; reason: string | null; result: 'kept' | 'discarded' | null; resultBranch: string | null;
  baseRevision: string | null; workspaceName: string; workspacePath: string;
  policy: { steps: Step[]; checks: { command: string; source: string }[]; allowShell: boolean; timeoutSecs: number };
  /** Every attempt in full; `attempts` holds the chips. */
  attemptDetails: AttemptView[];
  approval: ApprovalView | null;
};

/** Display names of the agents (agents.rs AGENTS). */
export const AGENT_NAMES: Record<string, string> = {
  'claude-code': 'Claude Code', codex: 'Codex CLI', 'gemini-cli': 'Gemini CLI', opencode: 'OpenCode',
};

/** At most this many steps per task (tasks.rs MAX_STEPS). */
export const MAX_STEPS = 6;

export type TaskFilter = 'all' | 'running' | 'approval' | 'completed' | 'failed' | 'stopped';

export const FILTERS: TaskFilter[] = ['all', 'running', 'approval', 'completed', 'failed', 'stopped'];

export function matchesFilter(task: TaskSummary, filter: TaskFilter): boolean {
  switch (filter) {
    case 'all': return true;
    // Running excludes approval waits: those have their own filter.
    case 'running': return task.state === 'running';
    case 'approval': return task.state === 'waiting_approval';
    case 'completed': return task.state === 'checks_passed' || task.state === 'manually_accepted' || task.state === 'needs_review';
    case 'failed': return task.state === 'check_failed' || task.state === 'blocked';
    case 'stopped': return task.state === 'canceled' || task.state === 'interrupted';
  }
}

/** Tasks that wait on the user; pinned above the list and not repeated in it. */
export function needsAttention(task: TaskSummary): boolean {
  return task.state === 'waiting_approval' || task.state === 'interrupted' || task.state === 'needs_revalidation'
    || task.state === 'needs_review';
}

export type DayBudget = {
  /** Recorded spend today: actual plus estimated, in USD. */
  recordedUsd: number;
  /** Capacity held for running attempts; not spend. */
  reservedUsd: number;
  /** null: no daily limit. */
  limitUsd: number | null;
  /** Calls whose cost is not known make the remaining budget uncertain. */
  unknownCalls: number;
};
