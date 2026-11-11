/**
 * The gateway's side of the contract with the Rust core (src-tauri/src/commands.rs). Types mirror the
 * serialized Rust structs. In the desktop app every call is a Tauri command; in a browser preview
 * (pnpm dev) the same calls go to an in-memory demo (lib/demo.ts), which the interface labels as such.
 */
import { invoke } from '@tauri-apps/api/core';
import { runningInDesktop } from './native';
import type { DayBudget, NewTask, TaskDetail, TaskSummary, WorkspaceView } from './tasks';

export type Billing = 'free' | 'local' | 'subscription' | 'paid' | 'unknown';
export const BILLINGS: Billing[] = ['free', 'local', 'subscription', 'paid', 'unknown'];

export type GatewayStatus = {
  state: 'running' | 'stopped' | 'failed';
  port: number;
  baseUrl: string | null;
  error: string | null;
  autostart: boolean;
  accountingError: string | null;
  /** Stopped, but requests that were in flight are still finishing. */
  finishing: boolean;
};

export type Wire = 'openai' | 'anthropic';
export type Preset = {
  id: string; name: string; endpoint: string | null; needsKey: boolean; defaultBilling: Billing; wire: Wire;
  /** How to start using it, per the provider's docs (docs/PROVIDERS.md). */
  offer: 'free' | 'credits' | 'paid' | 'local' | 'custom';
  offerNote: string | null;
};

export type HealthState = 'invalid_credentials' | 'forbidden' | 'payment_required' | 'rate_limited' | 'model_unavailable'
  | 'server_error' | 'unreachable';
/** A cooldown or block on a connection (modelId '') or one of its models. untilMs null: until fixed. */
export type Health = {
  providerId: string; modelId: string; state: HealthState; reason: string; httpStatus: number | null;
  untilMs: number | null; updatedAt: number;
};

export type Connection = {
  id: string; kind: string; label: string; endpoint: string; enabled: boolean; credentialRef: string | null;
  defaultBilling: Billing; dataUseTerms: string; createdAt: number;
  wire: Wire; quotaGroup: string; separateQuota: boolean; quotaConfirmedAt: number | null;
  presetName: string; hasKey: boolean; needsKey: boolean; models: number; health: Health[]; sharesQuotaWith: string[];
};

export type Price = {
  inputMicroPerMtok: number; outputMicroPerMtok: number; cachedInputMicroPerMtok: number | null;
  cacheWriteMicroPerMtok?: number | null; source: string; observedAt: number;
};

export type Tier = 'T0' | 'T1' | 'T2' | 'T3' | 'T4';
export const TIERS: Tier[] = ['T0', 'T1', 'T2', 'T3', 'T4'];

/** What auto routing knows about a model. null is unknown, and unknown counts as missing. */
export type Capabilities = {
  tier: Tier | null; contextTokens: number | null; maxOutputTokens: number | null;
  supportsTools: boolean | null; supportsVision: boolean | null;
  /** 'user', or 'provider: <kind> /models'. */
  source: string | null; observedAt: number | null;
};

export type Model = {
  providerId: string; modelId: string; billing: Billing; enabled: boolean; price: Price | null; observedAt: number | null;
  capabilities: Capabilities;
};

export type AutoAlias = 'auto' | 'auto/cheap';
export type AutoNeeds = { surfaceAnthropic: boolean; tools: boolean; vision: boolean; inputTokens: number; maxTokens: number | null };
export type AutoRanked = { providerId: string; modelId: string; tier: Tier | null; billing: Billing; estimateMicroUsd: number; reason: string };
export type AutoExcluded = { providerId: string; modelId: string; reason: string };
export type AutoPlan = { alias: AutoAlias; strategy: 'tier' | 'cheap'; minTier: Tier | null; ranked: AutoRanked[]; excluded: AutoExcluded[] };
/** How many ranked models make up an auto route. */
export const AUTO_ROUTE_LENGTH = 5;

export type Target = { providerId: string; modelId: string };
export type Combo = { id: string; name: string; members: Target[] };

export type Client = {
  id: string; name: string; tokenPrefix: string; monthlyBudgetMicroUsd: number | null; createdAt: number;
  lastUsedAt: number | null; revokedAt: number | null; monthSpendMicroUsd: number; monthUnknownRequests: number;
  maxConcurrency: number; unknownCostPolicy: 'block' | 'allow'; reservedMicroUsd: number;
  /** Spend that may have been charged but is unknown, counted at the amount held for it. */
  monthAtRiskMicroUsd: number;
  /** null: every connection. */
  allowedConnections: string[] | null;
  /** Further targets tried after a server error the provider may have billed; 0 stops. */
  paidRetries: number;
  /** The lowest tier `auto` may choose for this client. */
  autoMinTier: Tier;
  /** May start smart tasks through the task API (pygg-mcp). */
  mayRunTasks: boolean;
};

export type UsageGroup = {
  key: string; requests: number; failed: number; fallbacks: number; inputTokens: number; outputTokens: number;
  knownMicroUsd: number; actualMicroUsd: number; unknownRequests: number;
};

export type CostStatus = 'actual' | 'estimated' | 'unknown';
export type Outcome = 'ok' | 'fallback' | 'error' | 'client_disconnected' | 'stream_error' | 'interrupted';

export type UsageEvent = {
  id: string; createdAt: number; client: string | null; provider: string; model: string; requestedModel: string | null;
  inputTokens: number | null; outputTokens: number | null; costMicroUsd: number | null; costStatus: CostStatus;
  costSource: string; outcome: Outcome; httpStatus: number | null; latencyMs: number | null; error: string | null;
  cacheTokens: number | null; cacheWriteTokens: number | null; sessionKey: string | null; routeMode: string | null;
  atRiskMicroUsd: number | null;
  /** Why auto routing chose this target. */
  routeReason: string | null;
};

export type UsageSummary = {
  since: number; total: UsageGroup; byModel: UsageGroup[]; byClient: UsageGroup[]; recent: UsageEvent[];
};

/** An official agent CLI (Claude Code, Codex CLI, Gemini CLI, OpenCode) and how it signs in. */
export type AgentInfo = {
  id: string; name: string; installed: boolean; program: string | null; version: string | null;
  /** The CLI's own status line, where it has one. */
  login: string | null; signIn: string; install: string;
};
export type AgentTask = {
  agent: string; workspace: string; prompt: string; model: string | null; allowShell: boolean; timeoutSecs: number | null;
};
export type AgentUsage = {
  model: string | null; inputTokens: number | null; outputTokens: number | null; cacheReadTokens: number | null;
  cacheWriteTokens: number | null; estimateUsd: number | null; result: string | null; error: string | null;
};
export type AgentRun = {
  id: string; agent: string; model: string | null; workspace: string; worktree: string; base: string;
  outcome: 'ok' | 'failed' | 'timed_out' | 'canceled' | 'not_started'; exit: string; durationMs: number;
  filesChanged: number; insertions: number; deletions: number; usage: AgentUsage; logTail: string;
};

/** How an IDE starts the MCP server: the app's program with `args` (`mcp`). */
export type McpSetup = { path: string; args: string[]; exists: boolean; url: string };

export type NewConnection ={ preset: string; label: string; endpoint?: string; apiKey?: string; billing: Billing };
export type PriceInput = {
  inputUsdPerMtok: number; outputUsdPerMtok: number; cachedInputUsdPerMtok?: number | null; cacheWriteUsdPerMtok?: number | null;
};

export interface GatewayApi {
  gatewayStatus(): Promise<GatewayStatus>;
  gatewayStart(): Promise<GatewayStatus>;
  gatewayStop(): Promise<GatewayStatus>;
  gatewayConfigure(port: number, autostart: boolean): Promise<GatewayStatus>;
  presets(): Promise<Preset[]>;
  connections(): Promise<Connection[]>;
  addConnection(connection: NewConnection): Promise<Connection>;
  updateConnection(id: string, change: {enabled?: boolean; billing?: Billing; separateQuota?: boolean}): Promise<Connection>;
  clearConnectionHealth(id: string): Promise<Connection>;
  setConnectionKey(id: string, apiKey: string): Promise<Connection>;
  removeConnection(id: string): Promise<void>;
  refreshModels(id: string): Promise<{listed: number; added: number}>;
  models(providerId?: string): Promise<Model[]>;
  addModel(providerId: string, modelId: string): Promise<void>;
  updateModel(providerId: string, modelId: string,
    change: {enabled?: boolean; billing?: Billing; price?: PriceInput; clearPrice?: boolean}): Promise<void>;
  removeModel(providerId: string, modelId: string): Promise<void>;
  combos(): Promise<Combo[]>;
  saveCombo(id: string | null, name: string, members: Target[]): Promise<Combo>;
  deleteCombo(id: string): Promise<void>;
  clients(): Promise<Client[]>;
  createClient(name: string, monthlyBudgetUsd: number | null): Promise<{client: Client; token: string}>;
  revokeClient(id: string): Promise<void>;
  setClientBudget(id: string, monthlyBudgetUsd: number | null): Promise<void>;
  setClientPolicy(id: string, policy: {maxConcurrency?: number; unknownCostPolicy?: 'block' | 'allow';
    /** true with allowedConnections restricts the client; false lets it use every connection. */
    restrictConnections?: boolean; allowedConnections?: string[]; paidRetries?: number; autoMinTier?: Tier;
    /** Lets the client start smart tasks through the task API (pygg-mcp). */
    mayRunTasks?: boolean}): Promise<void>;
  setModelCapabilities(providerId: string, modelId: string, capabilities: Capabilities): Promise<void>;
  /** What an auto alias would choose, from a client's point of view (null: a new client's defaults). Sends nothing. */
  autoSimulate(clientId: string | null, alias: AutoAlias, needs: AutoNeeds): Promise<AutoPlan>;
  usage(days: number): Promise<UsageSummary>;
  agents(): Promise<AgentInfo[]>;
  /** Runs a task through an agent with its own sign-in, in a worktree of the workspace. */
  runAgent(task: AgentTask): Promise<AgentRun>;
  agentDiff(runId: string): Promise<string>;
  /** Keeps the run's changes as a branch (keep = its title) or discards them; returns the branch. */
  finishAgentRun(runId: string, keep: string | null): Promise<string | null>;
  workspaces(): Promise<WorkspaceView[]>;
  /** Adds a Git repository as a workspace; the checks it already has arrive as suggestions. */
  addWorkspace(path: string, name: string | null): Promise<WorkspaceView>;
  removeWorkspace(id: string): Promise<void>;
  detectChecks(id: string): Promise<WorkspaceView>;
  workspaceCheck(id: string, command: string, action: 'approve' | 'unapprove' | 'add' | 'remove'): Promise<WorkspaceView>;
  tasks(): Promise<TaskSummary[]>;
  task(id: string): Promise<TaskDetail>;
  /** Creates a task and starts it; returns its id. */
  createTask(task: NewTask): Promise<string>;
  cancelTask(id: string): Promise<void>;
  /** Approves or declines the paid step a task waits on. */
  decideTask(id: string, approve: boolean, capUsd: number | null): Promise<void>;
  retryTask(id: string): Promise<void>;
  taskDiff(id: string): Promise<string>;
  /** Keeps the result as a branch (returned) or discards it. */
  finishTask(id: string, keep: boolean, title: string | null): Promise<string | null>;
  /** Where pygg-mcp is, for the IDE setup instructions. */
  mcpSetup(): Promise<McpSetup>;
  /** Today's paid steps against the daily budget. */
  taskBudget(): Promise<DayBudget>;
  setTaskBudget(dailyUsd: number | null): Promise<DayBudget>;
  setWorkspaceBudget(id: string, dailyUsd: number | null): Promise<WorkspaceView>;
  /** `sensitive`: every step asks before it runs. */
  setWorkspacePrivacy(id: string, privacyClass: 'standard' | 'sensitive'): Promise<WorkspaceView>;
  /** The system's folder picker (desktop only); null when canceled. */
  pickFolder(): Promise<string | null>;
  /** Saves the task's result as a patch where the user chooses; the path, or null when canceled. */
  exportPatch(id: string): Promise<string | null>;
}

const native: GatewayApi = {
  gatewayStatus: () => invoke('gateway_status'),
  gatewayStart: () => invoke('gateway_start'),
  gatewayStop: () => invoke('gateway_stop'),
  gatewayConfigure: (port, autostart) => invoke('gateway_configure', {port, autostart}),
  presets: () => invoke('presets_list'),
  connections: () => invoke('connections_list'),
  addConnection: connection => invoke('connection_add', {connection}),
  updateConnection: (id, change) => invoke('connection_update', {id, ...change}),
  setConnectionKey: (id, apiKey) => invoke('connection_set_key', {id, apiKey}),
  clearConnectionHealth: id => invoke('connection_clear_health', {id}),
  removeConnection: id => invoke('connection_remove', {id}),
  refreshModels: id => invoke('connection_refresh_models', {id}),
  models: providerId => invoke('models_list', {providerId: providerId ?? null}),
  addModel: (providerId, modelId) => invoke('model_add', {providerId, modelId}),
  updateModel: (providerId, modelId, change) => invoke('model_update', {providerId, modelId, ...change}),
  removeModel: (providerId, modelId) => invoke('model_remove', {providerId, modelId}),
  combos: () => invoke('combos_list'),
  saveCombo: (id, name, members) => invoke('combo_save', {id, name, members}),
  deleteCombo: id => invoke('combo_delete', {id}),
  clients: () => invoke('clients_list'),
  createClient: (name, monthlyBudgetUsd) => invoke('client_create', {name, monthlyBudgetUsd}),
  revokeClient: id => invoke('client_revoke', {id}),
  setClientBudget: (id, monthlyBudgetUsd) => invoke('client_set_budget', {id, monthlyBudgetUsd}),
  setClientPolicy: (id, policy) => invoke('client_set_policy', {id, ...policy}),
  setModelCapabilities: (providerId, modelId, capabilities) => invoke('model_set_capabilities', {providerId, modelId, capabilities}),
  autoSimulate: (clientId, alias, needs) => invoke('auto_simulate', {clientId, alias, needs}),
  usage: days => invoke('usage_summary', {days}),
  agents: () => invoke('agents_detect'),
  runAgent: task => invoke('agent_run', {request: task}),
  agentDiff: runId => invoke('agent_diff', {id: runId}),
  finishAgentRun: (runId, keep) => invoke('agent_finish', {id: runId, keep}),
  workspaces: () => invoke('workspaces_list'),
  addWorkspace: (path, name) => invoke('workspace_add', {path, name}),
  removeWorkspace: id => invoke('workspace_remove', {id}),
  detectChecks: id => invoke('workspace_detect_checks', {id}),
  workspaceCheck: (id, command, action) => invoke('workspace_check', {id, command, action}),
  tasks: () => invoke('tasks_list'),
  task: id => invoke('task_get', {id}),
  createTask: task => invoke('task_create', {task}),
  cancelTask: id => invoke('task_cancel', {id}),
  decideTask: (id, approve, capUsd) => invoke('task_decide', {id, approve, capUsd}),
  retryTask: id => invoke('task_retry', {id}),
  taskDiff: id => invoke('task_diff', {id}),
  finishTask: (id, keep, title) => invoke('task_finish', {id, keep, title}),
  mcpSetup: () => invoke('mcp_setup'),
  taskBudget: () => invoke('task_budget'),
  setTaskBudget: dailyUsd => invoke('task_set_budget', {dailyUsd}),
  setWorkspaceBudget: (id, dailyUsd) => invoke('workspace_set_budget', {id, dailyUsd}),
  setWorkspacePrivacy: (id, privacyClass) => invoke('workspace_set_privacy', {id, class: privacyClass}),
  pickFolder: () => invoke('pick_folder'),
  exportPatch: id => invoke('task_export_patch', {id}),
};

let demo: Promise<GatewayApi> | null = null;

/** The API: the core in the desktop app, the labeled demo in a browser preview. */
export async function gatewayApi(): Promise<GatewayApi> {
  if (runningInDesktop) return native;
  demo ??= import('./demo').then(m => m.demoApi());
  return demo;
}

/** Shorthand: `call(api => api.connections())`. */
export async function call<T>(f: (api: GatewayApi) => Promise<T>): Promise<T> {
  return f(await gatewayApi());
}

/** A Tauri command rejects with the Rust error string; anything else becomes its message. */
export function errorText(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

/** Changes made on one screen that others should reflect (a new connection appears in Combos, …). */
const listeners = new Set<() => void>();
export function onGatewayChange(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
export function gatewayChanged() {
  listeners.forEach(listener => listener());
}
