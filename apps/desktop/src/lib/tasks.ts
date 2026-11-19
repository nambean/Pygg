import { t } from './i18n';
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
export type AttemptOutcome = 'passed' | 'done' | 'failed' | 'running' | 'waiting' | 'canceled' | 'skipped';

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

/** `held`: the agent is done and the run waits for room in Checking (the board's limit, ADR-042). */
export type RunningPhase = 'preparing' | 'generating' | 'checking' | 'held';

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
  /** sensitive: every step asks before it runs. */
  privacyClass: 'standard' | 'sensitive';
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
  /** The step's cost as Pygg knows it, when known; costSource says from where. */
  agentEstimateUsd: number | null;
  /** 'pygg': Pygg's ledger, for requests the agent sent through Pygg's gateway; 'agent': the agent's own
   * list-price estimate; null: recorded before schema v19 (the agent's estimate). */
  costSource?: 'pygg' | 'agent' | null;
  /** Requests this step sent through Pygg's gateway that had no price. */
  unpricedRequests?: number;
  /** The cap the user approved, for a paid step. */
  capUsd: number | null;
  /** What this attempt was told about earlier ones, after the task (ADR-030); null: the task alone. */
  handoffSent?: string | null;
  /** Paths this attempt changed, kept after a discarded worktree is gone. */
  changedPaths?: string[];
};

/** What earlier runs of the agent and model cost by its own report; null without history. */
export type Estimate = { runs: number; lowMicroUsd: number | null; highMicroUsd: number | null };

/** A paid step waiting for the user's decision (ADR-010: one attempt, one cap). */
export type ApprovalView = {
  step: number;
  /** The step spends (a cap is needed); otherwise it asks only because the workspace is sensitive. */
  paid: boolean;
  label: string; estimate: Estimate; suggestedCapUsd: number;
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

// ---------- pygg/brain (gateway/brain.rs, ADR-041) ----------

export type BrainRole = 'plan' | 'code' | 'test' | 'review';
export type BrainQuestion = { id: string; text: string; answered: boolean };
export type BrainCard = {
  id: string; title: string; role: BrainRole; done: string; dependsOn: string[];
  /** `connection/model`, picked by rule for the role; null when nothing was eligible. */
  model: string | null; reason: string | null;
  /** Tried after `model` when an attempt on it fails: the rule's suggestions, or the user's own picks (`your pick`). */
  fallbacks: {model: string; reason: string}[];
  runner: 'engine' | 'ide'; class: 'standard' | 'expedite'; dropped: boolean; wave: number;
};
export type BrainstormState = 'waiting_pick' | 'thinking' | 'proposed' | 'approved' | 'declined' | 'failed';
export type Brainstorm = {
  id: string; clientId: string | null; session: string; workspaceId: string | null; state: BrainstormState; requirement: string;
  planner: string | null; promptVersion: string; revision: number;
  proposal: { questions: BrainQuestion[]; cards: BrainCard[]; droppedCards: number };
  rawReply: string | null; note: string | null; error: string | null; createdAt: number; updatedAt: number;
};
/** A brainstorm the user must look at: it waits for a pick, a decision, or failed. */
export function brainstormNeedsAttention(b: Brainstorm): boolean {
  return b.state === 'waiting_pick' || b.state === 'proposed' || b.state === 'failed';
}

/** One fact behind a model's rank for a role, as the feed records it (gateway/facts.rs, ADR-040). */
export type FactView = {
  source: string; sourceName: string; licence: string; url: string | null; name: string; place: number; of: number; rank: number;
  score: number; unit: string; ci: number | null; date: string | null; derived: boolean; agent: string | null; effort: string | null;
  votes: number | null; ranks: boolean; measures: string;
};
/** The user's own runs on cards of a role with this model (ADR-029); ranks from ten attempts on. */
export type OwnEvidence = { attempts: number; checksPassed: number; kept: number; ranks: boolean };
export type RoleLine = {
  model: string; providerId: string; modelId: string; billing: Billing; tier: string | null; tierSource: string | null;
  layer: 'own' | 'feed' | 'tier' | 'none'; own: OwnEvidence | null; fact: FactView | null; others: FactView[]; outputMicroPerMtok: number | null;
};
/** The layer that won, in one line with its source and date (BRAIN.md 4.5). */
export function evidenceLine(l: RoleLine): string {
  if (l.own?.ranks) return `your runs ${l.own.kept}/${l.own.attempts} kept · ${l.own.checksPassed}/${l.own.attempts} checks passed`;
  if (l.fact) return factLine(l.fact);
  return l.tier ? `tier ${l.tier} only` : 'no tier';
}

// ---------- the board (board.rs, ADR-042) ----------

export type BoardLane = 'discovery' | 'ready' | 'running' | 'checking' | 'blocked' | 'review' | 'done' | 'discarded';
export const BOARD_LANES: BoardLane[] = ['discovery', 'ready', 'running', 'checking', 'blocked', 'review', 'done'];
export type BoardCard = {
  id: string; workspaceId: string; brainstormId: string | null; cardId: string; title: string; role: BrainRole; done: string; prompt: string;
  dependsOn: string[]; ladder: string[]; agent: string; runner: 'engine' | 'ide'; class: 'standard' | 'expedite'; lane: BoardLane; held: boolean;
  taskId: string | null; blockedReason: string | null; blockedSince: number | null; blockedMs: number; readyAt: number | null;
  startedAt: number | null; doneAt: number | null; position: number; createdAt: number; updatedAt: number; task: TaskSummary | null;
};
export type BoardLimits = { wipRunning: number; wipChecking: number; own: boolean };
export type RoleMoney = { role: BrainRole; kept: number; knownUsd: number; unknownCalls: number; usdPerKept: number | null };
export type BoardMeasures = { measured: boolean; done: number; doneToday: number; cycleTimeMedianMs: number | null; cycleTimed: number; leadTimeMedianMs: number | null; blockedTodayMs: number; money: RoleMoney[] };
/** A running task's phase as a word. */
export function phaseLabel(phase: RunningPhase | undefined): string {
  return phase === 'preparing' ? t('Preparing') : phase === 'checking' ? t('Checking') : phase === 'held' ? t('Waiting for Checking') : t('Generating');
}
export type Board = { workspaceId: string; workspaceName: string; workspacePath: string; limits: BoardLimits; cards: BoardCard[]; measures: BoardMeasures };
export type BoardSummary = { workspaceId: string; workspaceName: string; cards: number; attention: number };
export type BoardAction = 'expedite' | 'standard' | 'hold' | 'release' | 'drop' | 'engine';
/** The Brain Storming defaults (BRAIN.md 4.7b). */
export type BrainSettings = { wipRunning: number; wipChecking: number; runner: 'engine' | 'ide'; rememberPlanner: boolean; changeScope: 'ask' | 'card' | 'role' };
export type BrainPick = { scope: string; role: BrainRole; model: string };
/** The model facts feed's state (gateway/facts.rs). */
export type FactsSource = { id: string; name: string; url: string; licence: string; attribution: string; measures: string; roles: string[]; status: string; observed: string | null; rows: number | null; termsRead: string | null; error: string | null };
export type FactsStatus = { enabled: boolean; fetchedAt: number | null; built: string | null; facts: number; sources: FactsSource[]; lastError: string | null; stale: boolean };
/** The fact in one line: source, place, score, date, and `derived` where the source measures something near the role. */
export function factLine(f: FactView): string {
  const ci = f.ci != null ? ` ±${f.ci}` : '';
  const unit = f.unit.startsWith('%') ? '%' : ` ${f.unit}`;
  return `${f.sourceName} #${f.place} of ${f.of} · ${f.score}${unit}${ci} · ${f.date ?? '—'}${f.derived ? ' · derived' : ''}`;
}
