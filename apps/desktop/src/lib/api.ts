/**
 * The gateway's side of the contract with the Rust core (src-tauri/src/commands.rs). Types mirror the
 * serialized Rust structs. In the desktop app every call is a Tauri command; in a browser preview
 * (pnpm dev) the same calls go to an in-memory demo (lib/demo.ts), which the interface labels as such.
 */
import { invoke } from '@tauri-apps/api/core';
import { runningInDesktop } from './native';

export type Billing = 'free' | 'local' | 'subscription' | 'paid' | 'unknown';
export const BILLINGS: Billing[] = ['free', 'local', 'subscription', 'paid', 'unknown'];

export type GatewayStatus = {
  state: 'running' | 'stopped' | 'failed';
  port: number;
  baseUrl: string | null;
  error: string | null;
  autostart: boolean;
  accountingError: string | null;
};

export type Wire = 'openai' | 'anthropic';
export type Preset = { id: string; name: string; endpoint: string | null; needsKey: boolean; defaultBilling: Billing; wire: Wire };

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

export type Model = {
  providerId: string; modelId: string; billing: Billing; enabled: boolean; price: Price | null; observedAt: number | null;
};

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
};

export type UsageSummary = {
  since: number; total: UsageGroup; byModel: UsageGroup[]; byClient: UsageGroup[]; recent: UsageEvent[];
};

export type NewConnection = { preset: string; label: string; endpoint?: string; apiKey?: string; billing: Billing };
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
    restrictConnections?: boolean; allowedConnections?: string[]; paidRetries?: number}): Promise<void>;
  usage(days: number): Promise<UsageSummary>;
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
  usage: days => invoke('usage_summary', {days}),
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
