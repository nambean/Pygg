/**
 * The gateway's side of the contract with the Rust core (src-tauri/src/commands.rs). Types mirror the
 * serialized Rust structs. In the desktop app every call is a Tauri command; in a browser preview
 * (pnpm dev) the same calls go to an in-memory demo (lib/demo.ts), which the interface labels as such.
 */
import { invoke } from '@tauri-apps/api/core';
import { engineText } from './engineText';
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
  offer: 'free' | 'credits' | 'paid' | 'local' | 'custom' | 'web';
  offerNote: string | null;
  /** Its free part is a set of $0 models (OpenRouter's :free): paid ones start off while it is set as free. */
  freeModels: boolean;
  /** A web session (ADR-035): a site with no API, used through a Pygg window the user signs in to. */
  web?: boolean;
};

/** The text of the web-session warning the user acknowledges; the same number as websession::WARNING_VERSION. */
export const WEB_WARNING_VERSION = 1;
export type WebState = 'signed_in' | 'signed_out' | 'needs_you' | 'page_changed';

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
  /** Where the user read the terms, and when they recorded them. */
  termsSource: string | null; termsCheckedAt: number | null;
  wire: Wire; quotaGroup: string; separateQuota: boolean; quotaConfirmedAt: number | null;
  /** The pool it belongs to (ADR-034) and its place in the pool's order. */
  pool?: string | null; poolRank?: number | null;
  presetName: string; hasKey: boolean; needsKey: boolean; models: number; health: Health[]; sharesQuotaWith: string[];
  /** See Preset.freeModels. */
  freeModels: boolean;
  account?: Account | null;
  /** Pygg can read its account's balance or free allowance. */
  readsAccount?: boolean;
  /** The rate-limit headers its provider last sent, per model (PV-06): as the provider said them. */
  limits?: RateLimits[];
  /** A web session (ADR-035): whether its warning was acknowledged in its current text, and what its window last showed
   * (in Pygg's own words, never the page's). */
  web?: boolean;
  webAckCurrent?: boolean;
  webAckAt?: number | null;
  webState?: WebState | null;
  webStateReason?: string | null;
  webStateAt?: number | null;
};

/** A limit a provider stated: what it allows, what is left, when it resets (ms). */
export type QuotaWindow = {kind: string; limit: number | null; remaining: number | null; resetsAt: number | null};
export type RateLimits = {modelId: string; headers: Record<string, string>; httpStatus: number | null; observedAt: number;
  windows?: QuotaWindow[]};
/** How a model has answered over the last 30 days, from the ledger. */
export type Reliability = {modelId: string; requests: number; ok: number; medianLatencyMs: number | null};

/** What routing saved, as an estimate against a model the user chose (gateway/savings.rs). */
export type Savings = {
  baseline: string | null; baselinePriced: boolean;
  answered: {free: number; local: number; subscription: number; paid: number; unknown: number};
  compared: number; routedMicroUsd: number; baselineMicroUsd: number; failedAttemptsMicroUsd: number;
  leftOutUnknownCost: number; leftOutNoTokens: number; failedUnknownCost: number;
  /** A baseline to propose, from evidence: the paid, priced model asked for by name most often. */
  suggested: string | null;
};

/** The newest request one API key sent: CLI Tools' "Working". */
export type LatestRequest = {clientId: string; createdAt: number; provider: string; model: string; requestedModel: string | null; outcome: Outcome};

/** The Claude Code launcher (`pygg run claude`): set up or not, and the console program that starts it. */
export type Launcher = {setUp: boolean; clientId: string | null; clientName: string | null; setUpAt: number | null; installed: boolean;
  cliPath: string | null};

/** What one short test request to a model came back with. */
export type ModelTest = {ok: boolean; text: string; latencyMs: number; inputTokens: number | null; outputTokens: number | null; route: string};

/** An account's balance or free allowance, as its provider's account endpoint said. */
export type Account = {source: string; observedAt: number; balance: number | null; currency: string | null;
  available: boolean | null; usedToday: number | null; freeRequests: QuotaWindow | null};

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
  /** On the Anthropic API: the thinking types it accepts (adaptive, enabled) and effort levels; null is unknown. */
  thinkingTypes: string[] | null; effortLevels: string[] | null;
  /** 'user', or 'curated:v2' (or an earlier version) for a starting tier from Pygg's table; tierNote says why the table gave it. */
  tierSource: string | null; tierNote: string | null;
  /** 'user', or 'provider: <kind> /models'. */
  source: string | null; observedAt: number | null;
};

export type Model = {
  providerId: string; modelId: string; billing: Billing; enabled: boolean; price: Price | null; observedAt: number | null;
  capabilities: Capabilities;
  /** Who last switched it: null (Pygg's default), 'user', or 'free-connection' (off: it has a price on a connection
   * set as free whose free part is its $0 models; ADR-032). */
  enabledSource: string | null;
};

export type AutoAlias = 'auto' | 'auto/cheap';
export type AutoNeeds = { surfaceAnthropic: boolean; tools: boolean; vision: boolean; inputTokens: number; maxTokens: number | null;
  /** An Anthropic request's thinking.type and output_config.effort. */
  thinking: 'adaptive' | 'enabled' | 'disabled' | null; effort: string | null };
export type AutoRanked = { providerId: string; modelId: string; tier: Tier | null; billing: Billing; estimateMicroUsd: number; reason: string };
export type AutoExcluded = { providerId: string; modelId: string; reason: string };
export type AutoPlan = { alias: AutoAlias; strategy: 'tier' | 'cheap'; minTier: Tier | null; ranked: AutoRanked[]; excluded: AutoExcluded[] };
/** How many ranked models make up an auto route. */
export const AUTO_ROUTE_LENGTH = 5;

export type Target = { providerId: string; modelId: string };
export type Combo = { id: string; name: string; members: Target[] };

export type Client = {
  /** Whether the token can be shown again (a copy is in the credential store). */
  tokenViewable?: boolean;
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
  /** Sends only to connections recorded as not training on requests. */
  requireNoTraining: boolean;
  /** A strict limit: each request held at the most it could cost; unknown price or maximum output not used. */
  strictBudget: boolean;
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

/** Repeated tool output in a period's requests (P1-4, shadow): measured, never removed. Tokens are about-figures. */
export type ShadowSummary = {requests: number; requestsWithRepeats: number; repeatedTokens: number; inputTokens: number;
  cacheReadTokens: number};

/** Something in the ledger worth a look; `value` and `against` are micro-USD for money, counts for requests. */
export type UsageAlert = {kind: 'near_limit' | 'over_limit' | 'spend_spike' | 'unknown_costs' | 'failures'; client: string | null;
  value: number; against: number};

/** Requests and money in one period of the usage chart (one hour or one day). */
export type UsageBucket = {
  start: number; requests: number; failed: number; knownMicroUsd: number; actualMicroUsd: number; unknownRequests: number;
};

export type UsageSummary = {
  since: number; total: UsageGroup; byModel: UsageGroup[]; byClient: UsageGroup[]; recent: UsageEvent[];
};

/** Usage → Subscriptions: one subscription connection's requests and tokens in the period, and its provider's stated limits. */
export type SubscriptionUsage = {
  providerId: string; label: string; kind: string | null; requests: number;
  /** Requests that failed or fell back to another model. */
  notOk: number; inputTokens: number; outputTokens: number; lastUsedAt: number | null;
  /** The rate-limit windows from the provider's last answer, per model; empty when it stated none. */
  limits: RateLimits[]; account: Account | null;
};
/** Usage → Local runtime: one model on a local connection. */
export type LocalModelUsage = {
  model: string; requests: number; notOk: number; inputTokens: number; outputTokens: number;
  /** Median time of its successful requests; null when none succeeded. */
  medianLatencyMs: number | null; lastUsedAt: number | null;
};
export type LocalUsage = {providerId: string; label: string; kind: string | null; models: LocalModelUsage[]};

/** How long usage history is kept (Settings → Usage History); null is forever. */
export type RetentionDays = 30 | 90 | 365 | null;
/** What one clean-up deleted: ledger rows and route decisions created before `before`. */
export type RetentionRun = {at: number; before: number; usageRows: number; routeDecisions: number};
export type Retention = {days: RetentionDays; lastRun: RetentionRun | null};

/** How tasks turned out (ROADMAP C4): every paid step counts toward the money per kept result. */
export type TaskQuality = {
  tasks: number; kept: number; discarded: number; retried: number; cancelled: number;
  checkedAttempts: number; passedAttempts: number; knownMicroUsd: number; unknownEntries: number;
  microUsdPerKept: number | null;
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
/** An agent Pygg can register its MCP server with, or point at its gateway (integrations.rs). */
export type Integration = {
  id: string; name: string; installed: boolean;
  /** 'mcp': Pygg's MCP server in the agent; 'gateway': the agent's models through Pygg's gateway. */
  kind?: 'mcp' | 'gateway';
  state: 'not_installed' | 'not_set_up' | 'set_up' | 'set_up_elsewhere' | 'removed_elsewhere';
  configPath: string; detail: string | null; clientName: string | null; setUpAt: number | null; backup: string | null;
};
/** What setting an integration up would change, shown before the user agrees. */
export type IntegrationPlan = {
  id: string; name: string; kind?: 'mcp' | 'gateway'; configPath: string; configExists: boolean; change: string; method: 'command' | 'file';
  clientName: string; backupDir: string;
};

export type NewConnection ={ preset: string; label: string; endpoint?: string; apiKey?: string; billing: Billing;
  /** For a web session: the warning text the user acknowledged (WEB_WARNING_VERSION). */
  webAck?: number };
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
  /** Connects by signing in on the provider's page in the browser, which issues an API key (OpenRouter). */
  signInConnection(connection: NewConnection): Promise<Connection>;
  updateConnection(id: string, change: {enabled?: boolean; billing?: Billing; separateQuota?: boolean}): Promise<Connection>;
  clearConnectionHealth(id: string): Promise<Connection>;
  /** Reads the account's balance or free allowance from the provider now. */
  checkAccount(id: string): Promise<Connection>;
  /** Sends one short request to a model and reports what came back (Test Model). */
  testModel(providerId: string, modelId: string): Promise<ModelTest>;
  /** How a connection's models have answered over the last 30 days. */
  reliability(connectionId: string): Promise<Reliability[]>;
  /** The newest request of each API key. */
  latestRequests(): Promise<LatestRequest[]>;
  savings(days: number): Promise<Savings>;
  setSavingsBaseline(baseline: string | null): Promise<void>;
  launcher(): Promise<Launcher>;
  launcherSetUp(): Promise<Launcher>;
  launcherRemove(): Promise<Launcher>;
  /** Puts a connection in a pool, or takes it out with null (ADR-034). */
  setConnectionPool(id: string, pool: string | null): Promise<Connection>;
  moveInPool(id: string, up: boolean): Promise<void>;
  /** Records what the provider's terms say about training on requests; no_training needs a source. */
  setConnectionTerms(id: string, terms: 'no_training' | 'may_train' | 'unknown', source: string | null): Promise<Connection>;
  setConnectionKey(id: string, apiKey: string): Promise<Connection>;
  removeConnection(id: string): Promise<void>;
  refreshModels(id: string): Promise<{listed: number; added: number}>;
  /** Shows a web session's window, where the user signs in on the site's own page (ADR-035). */
  openWebSession(id: string): Promise<void>;
  /** Opens a new chat in a web session's window to see whether it is signed in. */
  checkWebSession(id: string): Promise<Connection>;
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
  /** A key's token, from its copy in the credential store. */
  clientToken(id: string): Promise<string>;
  /** Takes a revoked key off the list. */
  deleteClient(id: string): Promise<void>;
  deleteRevokedClients(): Promise<number>;
  setClientBudget(id: string, monthlyBudgetUsd: number | null): Promise<void>;
  setClientPolicy(id: string, policy: {maxConcurrency?: number; unknownCostPolicy?: 'block' | 'allow';
    /** true with allowedConnections restricts the client; false lets it use every connection. */
    restrictConnections?: boolean; allowedConnections?: string[]; paidRetries?: number; autoMinTier?: Tier;
    /** Lets the client start smart tasks through the task API (pygg-mcp). */
    mayRunTasks?: boolean;
    /** Only connections whose recorded terms say the provider does not train on requests. */
    requireNoTraining?: boolean;
    /** Hold each request at the most it could cost, and skip models whose price or maximum output is unknown. */
    strictBudget?: boolean}): Promise<void>;
  setModelCapabilities(providerId: string, modelId: string, capabilities: Capabilities): Promise<void>;
  /** What an auto alias would choose, from a client's point of view (null: a new client's defaults). Sends nothing. */
  autoSimulate(clientId: string | null, alias: AutoAlias, needs: AutoNeeds): Promise<AutoPlan>;
  usage(days: number): Promise<UsageSummary>;
  /** Usage per hour for one day, per day for longer, on this computer's clock. */
  usageOverTime(days: number): Promise<UsageBucket[]>;
  /** Each subscription connection's requests and tokens over the last `days`, with its stated limits. */
  usageSubscriptions(days: number): Promise<SubscriptionUsage[]>;
  /** Each local connection's models over the last `days`: requests, tokens, median time. */
  usageLocal(days: number): Promise<LocalUsage[]>;
  /** Every request of the period as a CSV file the user saves; the path, or null when they cancel. */
  exportUsage(days: number): Promise<string | null>;
  /** What in the ledger is worth a look now. */
  usageAlerts(): Promise<UsageAlert[]>;
  /** Repeated tool output in the last `days` (shadow). */
  usageShadow(days: number): Promise<ShadowSummary>;
  retention(): Promise<Retention>;
  /** Saves the period and deletes what is older now; the rest goes once a day. */
  setRetention(days: RetentionDays): Promise<Retention>;
  /** How many requests keeping `days` days would delete now; deletes nothing. */
  retentionPreview(days: number): Promise<number>;
  /** The redacted diagnostics bundle as a zip the user saves; the path, or null when they cancel. */
  exportDiagnostics(): Promise<string | null>;
  /** How tasks turned out over the last `days`. */
  taskQuality(days: number): Promise<TaskQuality>;
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
  /** The agents Pygg can register its MCP server with, and whether it is. */
  integrations(): Promise<Integration[]>;
  integrationPlan(id: string): Promise<IntegrationPlan>;
  /** Backs up the agent's configuration, creates a token that may start tasks, registers the server, checks it. */
  applyIntegration(id: string): Promise<Integration>;
  /** Removes the entry and revokes the token Pygg made for it. */
  removeIntegration(id: string): Promise<Integration>;
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
  signInConnection: connection => invoke('connection_sign_in', {connection}),
  updateConnection: (id, change) => invoke('connection_update', {id, ...change}),
  setConnectionKey: (id, apiKey) => invoke('connection_set_key', {id, apiKey}),
  clearConnectionHealth: id => invoke('connection_clear_health', {id}),
  checkAccount: id => invoke('connection_check_account', {id}),
  setConnectionPool: (id, pool) => invoke('connection_set_pool', {id, pool}),
  testModel: (providerId, modelId) => invoke('model_test', {providerId, modelId}),
  reliability: id => invoke('connection_reliability', {id}),
  latestRequests: () => invoke('client_latest_requests'),
  savings: days => invoke('usage_savings', {days}),
  setSavingsBaseline: baseline => invoke('usage_set_savings_baseline', {baseline}),
  launcher: () => invoke('launcher_state'),
  launcherSetUp: () => invoke('launcher_set_up'),
  launcherRemove: () => invoke('launcher_remove'),
  moveInPool: (id, up) => invoke('connection_move_in_pool', {id, up}),
  setConnectionTerms: (id, terms, source) => invoke('connection_set_terms', {id, terms, source}),
  removeConnection: id => invoke('connection_remove', {id}),
  refreshModels: id => invoke('connection_refresh_models', {id}),
  openWebSession: id => invoke('web_session_open', {id}),
  checkWebSession: id => invoke('web_session_check', {id}),
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
  clientToken: id => invoke('client_token', {id}),
  deleteClient: id => invoke('client_delete', {id}),
  deleteRevokedClients: () => invoke('client_delete_revoked'),
  setClientBudget: (id, monthlyBudgetUsd) => invoke('client_set_budget', {id, monthlyBudgetUsd}),
  setClientPolicy: (id, policy) => invoke('client_set_policy', {id, ...policy}),
  setModelCapabilities: (providerId, modelId, capabilities) => invoke('model_set_capabilities', {providerId, modelId, capabilities}),
  autoSimulate: (clientId, alias, needs) => invoke('auto_simulate', {clientId, alias, needs}),
  usage: days => invoke('usage_summary', {days}),
  usageOverTime: days => invoke('usage_over_time', {days, offsetMinutes: -new Date().getTimezoneOffset()}),
  exportUsage: days => invoke('usage_export', {days}),
  usageAlerts: () => invoke('usage_alerts'),
  usageShadow: days => invoke('usage_shadow', {days}),
  usageSubscriptions: days => invoke('usage_subscriptions', {days}),
  usageLocal: days => invoke('usage_local', {days}),
  exportDiagnostics: () => invoke('diagnostics_export'),
  retention: () => invoke('retention_get'),
  setRetention: days => invoke('retention_set', {days}),
  retentionPreview: days => invoke('retention_preview', {days}),
  taskQuality: days => invoke('tasks_quality', {days}),
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
  integrations: () => invoke('integrations_list'),
  integrationPlan: id => invoke('integration_plan', {id}),
  applyIntegration: id => invoke('integration_apply', {id}),
  removeIntegration: id => invoke('integration_remove', {id}),
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
/** An error as text, in the interface language when it is one of the engine's templates (engineText.ts). */
export function errorText(error: unknown): string {
  const text = typeof error === 'string' ? error : error instanceof Error ? error.message : String(error);
  return engineText(text);
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
