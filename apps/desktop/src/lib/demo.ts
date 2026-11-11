/**
 * Browser-preview stand-in for the core (pnpm dev), so the gateway screens can be reviewed without the
 * desktop app. Everything lives in memory, nothing is sent anywhere, and the interface shows a "Demo
 * data" banner whenever this is in use (ADR-014). It follows the core's rules (slugs, validation, honest
 * costs) closely enough for design review, not as a test of them.
 */
import type { Billing, Client, Combo, Connection, GatewayApi, GatewayStatus, Health, Model, Preset, Target, UsageEvent,
  UsageGroup, UsageSummary, Wire } from './api';

const PRESETS: Preset[] = [
  {id: 'gemini', name: 'Google Gemini', endpoint: 'https://generativelanguage.googleapis.com/v1beta/openai', needsKey: true, defaultBilling: 'unknown', wire: 'openai'},
  {id: 'anthropic', name: 'Anthropic', endpoint: 'https://api.anthropic.com/v1', needsKey: true, defaultBilling: 'paid', wire: 'anthropic'},
  {id: 'openrouter', name: 'OpenRouter', endpoint: 'https://openrouter.ai/api/v1', needsKey: true, defaultBilling: 'paid', wire: 'openai'},
  {id: 'openai', name: 'OpenAI', endpoint: 'https://api.openai.com/v1', needsKey: true, defaultBilling: 'paid', wire: 'openai'},
  {id: 'groq', name: 'Groq', endpoint: 'https://api.groq.com/openai/v1', needsKey: true, defaultBilling: 'unknown', wire: 'openai'},
  {id: 'deepseek', name: 'DeepSeek', endpoint: 'https://api.deepseek.com/v1', needsKey: true, defaultBilling: 'paid', wire: 'openai'},
  {id: 'ollama', name: 'Ollama', endpoint: 'http://127.0.0.1:11434/v1', needsKey: false, defaultBilling: 'local', wire: 'openai'},
  {id: 'custom', name: 'Custom (OpenAI-compatible)', endpoint: null, needsKey: false, defaultBilling: 'unknown', wire: 'openai'},
  {id: 'custom-anthropic', name: 'Custom (Anthropic-compatible)', endpoint: null, needsKey: false, defaultBilling: 'unknown', wire: 'anthropic'},
];

const DEMO_MODELS: Record<string, string[]> = {
  anthropic: ['claude-haiku-4-5', 'claude-sonnet-4-5'],
  'custom-anthropic': [],
  gemini: ['gemini-2.5-flash', 'gemini-2.5-flash-lite', 'gemini-2.5-pro', 'gemini-3.1-flash-lite', 'gemini-3.1-pro-preview', 'gemini-3.5-flash'],
  openrouter: ['anthropic/claude-sonnet-4.5', 'deepseek/deepseek-chat', 'openai/gpt-4.1-mini', 'qwen/qwen3-coder'],
  openai: ['gpt-4.1', 'gpt-4.1-mini', 'gpt-5-mini'],
  groq: ['llama-3.3-70b-versatile', 'openai/gpt-oss-120b'],
  deepseek: ['deepseek-chat', 'deepseek-reasoner'],
  ollama: ['qwen2.5-coder:7b', 'llama3.2:3b'],
  custom: [],
};

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const clone = <T>(value: T): T => structuredClone(value);
const hex = (n: number) => Array.from(crypto.getRandomValues(new Uint8Array(n)), b => b.toString(16).padStart(2, '0')).join('');

function slug(label: string): string {
  const out = label.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 32);
  return out || 'connection';
}

type Stored = Omit<Connection, 'presetName' | 'hasKey' | 'needsKey' | 'models' | 'health' | 'sharesQuotaWith' | 'quotaGroup'>
  & {key: boolean};

export function demoApi(): GatewayApi {
  const now = Date.now();
  let status: GatewayStatus = {state: 'running', port: 8741, baseUrl: 'http://127.0.0.1:8741', error: null, autostart: true,
    accountingError: null, finishing: false};
  const connections: Stored[] = [
    {id: 'gemini', kind: 'gemini', label: 'Gemini', endpoint: PRESETS[0].endpoint!, enabled: true, credentialRef: 'provider-demo1',
      defaultBilling: 'unknown', dataUseTerms: 'unknown', createdAt: now - 6 * 86_400_000, key: true, wire: 'openai', separateQuota: false, quotaConfirmedAt: null},
    {id: 'anthropic', kind: 'anthropic', label: 'Anthropic', endpoint: PRESETS[1].endpoint!, enabled: true, credentialRef: 'provider-demo3',
      defaultBilling: 'paid', dataUseTerms: 'unknown', createdAt: now - 6 * 86_400_000, key: true, wire: 'anthropic', separateQuota: false, quotaConfirmedAt: null},
    {id: 'openrouter', kind: 'openrouter', label: 'OpenRouter', endpoint: PRESETS[1].endpoint!, enabled: true, credentialRef: 'provider-demo2',
      defaultBilling: 'paid', dataUseTerms: 'unknown', createdAt: now - 5 * 86_400_000, key: true, wire: 'openai', separateQuota: false, quotaConfirmedAt: null},
    {id: 'ollama', kind: 'ollama', label: 'Ollama', endpoint: PRESETS[5].endpoint!, enabled: true, credentialRef: null,
      defaultBilling: 'local', dataUseTerms: 'unknown', createdAt: now - 4 * 86_400_000, key: false, wire: 'openai', separateQuota: false, quotaConfirmedAt: null},
  ];
  const models: Model[] = [];
  const addModels = (providerId: string, ids: string[]) => {
    for (const modelId of ids) {
      if (!models.some(m => m.providerId === providerId && m.modelId === modelId)) {
        models.push({providerId, modelId, billing: connections.find(c => c.id === providerId)?.defaultBilling ?? 'unknown',
          enabled: true, price: null, observedAt: Date.now()});
      }
    }
  };
  addModels('gemini', DEMO_MODELS.gemini);
  addModels('anthropic', DEMO_MODELS.anthropic);
  addModels('openrouter', DEMO_MODELS.openrouter);
  addModels('ollama', DEMO_MODELS.ollama);
  const priced = models.find(m => m.modelId === 'gemini-3.1-flash-lite')!;
  priced.price = {inputMicroPerMtok: 100_000, outputMicroPerMtok: 400_000, cachedInputMicroPerMtok: 25_000, source: 'user', observedAt: now - 86_400_000};
  const sonnet = models.find(m => m.modelId === 'claude-sonnet-4-5')!;
  sonnet.price = {inputMicroPerMtok: 3_000_000, outputMicroPerMtok: 15_000_000, cachedInputMicroPerMtok: 300_000,
    cacheWriteMicroPerMtok: 3_750_000, source: 'user', observedAt: now - 86_400_000};
  // A cooldown to show what a 429 looks like.
  const health: Health[] = [{providerId: 'gemini', modelId: 'gemini-3.1-pro-preview', state: 'rate_limited',
    reason: 'Gemini answered HTTP 429: Resource has been exhausted (e.g. check quota).', httpStatus: 429,
    untilMs: now + 95_000, updatedAt: now - 25_000}];

  const combos: Combo[] = [
    {id: 'c1', name: 'coding', members: [{providerId: 'gemini', modelId: 'gemini-3.1-flash-lite'},
      {providerId: 'openrouter', modelId: 'qwen/qwen3-coder'}, {providerId: 'gemini', modelId: 'gemini-3.1-pro-preview'}]},
    {id: 'c3', name: 'claude', members: [{providerId: 'anthropic', modelId: 'claude-sonnet-4-5'},
      {providerId: 'anthropic', modelId: 'claude-haiku-4-5'}]},
    {id: 'c2', name: 'local-first', members: [{providerId: 'ollama', modelId: 'qwen2.5-coder:7b'},
      {providerId: 'gemini', modelId: 'gemini-3.1-flash-lite'}]},
  ];
  const clients: (Client & {spend: number})[] = [
    {id: 'k1', name: 'Claude Code', tokenPrefix: 'str-3f9a1c0b', monthlyBudgetMicroUsd: 20_000_000, createdAt: now - 5 * 86_400_000,
      lastUsedAt: now - 4 * 60_000, revokedAt: null, monthSpendMicroUsd: 0, monthUnknownRequests: 0, spend: 0,
      maxConcurrency: 8, unknownCostPolicy: 'block', reservedMicroUsd: 412_000, monthAtRiskMicroUsd: 0,
      allowedConnections: ['anthropic', 'gemini'], paidRetries: 0},
    {id: 'k2', name: 'Scripts', tokenPrefix: 'str-81d2e7aa', monthlyBudgetMicroUsd: null, createdAt: now - 3 * 86_400_000,
      lastUsedAt: now - 3 * 3_600_000, revokedAt: null, monthSpendMicroUsd: 0, monthUnknownRequests: 0, spend: 0,
      maxConcurrency: 4, unknownCostPolicy: 'allow', reservedMicroUsd: 0, monthAtRiskMicroUsd: 0,
      allowedConnections: null, paidRetries: 1},
  ];

  // A day and a half of plausible traffic: mostly priced Flash-Lite, some fallbacks, some unknown costs.
  const events: UsageEvent[] = [];
  let seed = 7;
  const random = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  for (let i = 0; i < 64; i++) {
    const at = now - Math.round(i * 37 * 60_000 * (0.6 + random()));
    const client = random() < 0.75 ? 'Claude Code' : 'Scripts';
    const input = Math.round(2_000 + random() * 30_000);
    const output = Math.round(100 + random() * 2_000);
    const roll = random();
    const base = {createdAt: at, client, requestedModel: 'coding', latencyMs: Math.round(400 + random() * 5_000), error: null,
      cacheTokens: null, cacheWriteTokens: null, sessionKey: client === 'Claude Code' ? 'shell-7f3a' : null, routeMode: 'native',
      atRiskMicroUsd: null};
    if (roll < 0.05) {
      // Claude Code on Anthropic, passed through unchanged: most of the prompt is read from the cache.
      const read = Math.round(input * 0.9), write = Math.round(input * 0.08);
      events.push({...base, id: `e${i}`, requestedModel: 'claude', provider: 'anthropic', model: 'claude-sonnet-4-5',
        inputTokens: input, outputTokens: output, cacheTokens: read, cacheWriteTokens: write,
        costMicroUsd: Math.round((input - read - write) * 3 + read * 0.3 + write * 3.75 + output * 15), costStatus: 'estimated',
        costSource: 'price: user', outcome: 'ok', httpStatus: 200});
    } else if (roll < 0.1) {
      events.push({...base, id: `e${i}a`, provider: 'gemini', model: 'gemini-3.1-flash-lite', inputTokens: null, outputTokens: null,
        costMicroUsd: 0, costStatus: 'estimated', costSource: 'rejected by the provider (HTTP 429)', outcome: 'fallback',
        httpStatus: 429, error: 'Gemini answered HTTP 429: Resource has been exhausted (e.g. check quota).'});
      events.push({...base, id: `e${i}b`, createdAt: at + 900, provider: 'openrouter', model: 'qwen/qwen3-coder', inputTokens: input,
        outputTokens: output, costMicroUsd: Math.round(input * 0.3 + output * 1.2), costStatus: 'actual', costSource: 'provider response',
        outcome: 'ok', httpStatus: 200});
    } else if (roll < 0.12) {
      // An overloaded provider: the request may have run, so the cost is unknown and the held amount counts.
      events.push({...base, id: `e${i}a`, provider: 'gemini', model: 'gemini-3.1-flash-lite', inputTokens: null, outputTokens: null,
        costMicroUsd: null, costStatus: 'unknown', costSource: 'HTTP 503: the provider may have run the request', outcome: 'fallback',
        httpStatus: 503, error: 'Gemini answered HTTP 503: The model is overloaded.', atRiskMicroUsd: 4_800});
      events.push({...base, id: `e${i}b`, createdAt: at + 1200, provider: 'openrouter', model: 'qwen/qwen3-coder', inputTokens: input,
        outputTokens: output, costMicroUsd: Math.round(input * 0.3 + output * 1.2), costStatus: 'actual', costSource: 'provider response',
        outcome: 'ok', httpStatus: 200});
    } else if (roll < 0.2) {
      events.push({...base, id: `e${i}`, provider: 'gemini', model: 'gemini-3.1-pro-preview', inputTokens: input, outputTokens: output,
        costMicroUsd: null, costStatus: 'unknown', costSource: 'no price recorded for this model', outcome: 'ok', httpStatus: 200});
    } else if (roll < 0.27) {
      events.push({...base, id: `e${i}`, requestedModel: 'local-first', provider: 'ollama', model: 'qwen2.5-coder:7b', inputTokens: input,
        outputTokens: output, costMicroUsd: 0, costStatus: 'estimated', costSource: 'local model', outcome: 'ok', httpStatus: 200});
    } else if (roll < 0.3) {
      events.push({...base, id: `e${i}`, provider: 'gemini', model: 'gemini-3.1-flash-lite', inputTokens: null, outputTokens: null,
        costMicroUsd: 0, costStatus: 'estimated', costSource: 'rejected by the provider (HTTP 400)', outcome: 'error', httpStatus: 400,
        error: 'Gemini answered HTTP 400: Invalid JSON payload received.'});
    } else {
      events.push({...base, id: `e${i}`, provider: 'gemini', model: 'gemini-3.1-flash-lite', inputTokens: input, outputTokens: output,
        costMicroUsd: Math.round(input * 0.1 + output * 0.4), costStatus: 'estimated', costSource: 'price: user', outcome: 'ok', httpStatus: 200});
    }
  }

  // `?empty` starts from nothing, to review first-run states.
  if (new URLSearchParams(location.search).has('empty')) {
    connections.length = 0;
    models.length = 0;
    combos.length = 0;
    clients.length = 0;
    events.length = 0;
  }

  const monthStart = () => { const d = new Date(); return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1); };
  function clientView(c: Client & {spend: number}): Client {
    const mine = events.filter(e => e.client === c.name && e.createdAt >= monthStart());
    return {...c, monthSpendMicroUsd: mine.reduce((s, e) => s + (e.costMicroUsd ?? 0), 0),
      monthUnknownRequests: mine.filter(e => e.costStatus === 'unknown').length,
      monthAtRiskMicroUsd: mine.reduce((s, e) => s + (e.costMicroUsd === null ? e.atRiskMicroUsd ?? 0 : 0), 0)};
  }
  const quotaGroup = (c: Stored) => c.separateQuota ? c.id : c.kind;
  function view(c: Stored): Connection {
    const preset = PRESETS.find(p => p.id === c.kind);
    const {key, ...rest} = c;
    return {...rest, quotaGroup: quotaGroup(c), presetName: preset?.name ?? c.kind, hasKey: key, needsKey: preset?.needsKey ?? false,
      models: models.filter(m => m.providerId === c.id).length,
      health: health.filter(h => h.providerId === c.id && (h.untilMs === null || h.untilMs > Date.now())),
      sharesQuotaWith: connections.filter(o => o.id !== c.id && quotaGroup(o) === quotaGroup(c)).map(o => o.label)};
  }
  function find(id: string): Stored {
    const c = connections.find(x => x.id === id);
    if (!c) throw 'Connection not found';
    return c;
  }
  function group(key: string, list: UsageEvent[]): UsageGroup {
    return {key, requests: list.length,
      failed: list.filter(e => e.outcome !== 'ok' && e.outcome !== 'fallback').length,
      fallbacks: list.filter(e => e.outcome === 'fallback').length,
      inputTokens: list.reduce((s, e) => s + (e.inputTokens ?? 0), 0),
      outputTokens: list.reduce((s, e) => s + (e.outputTokens ?? 0), 0),
      knownMicroUsd: list.reduce((s, e) => s + (e.costMicroUsd ?? 0), 0),
      actualMicroUsd: list.filter(e => e.costStatus === 'actual').reduce((s, e) => s + (e.costMicroUsd ?? 0), 0),
      unknownRequests: list.filter(e => e.costStatus === 'unknown').length};
  }
  function groups(list: UsageEvent[], key: (e: UsageEvent) => string): UsageGroup[] {
    const map = new Map<string, UsageEvent[]>();
    for (const e of list) map.set(key(e), [...(map.get(key(e)) ?? []), e]);
    return [...map].map(([k, v]) => group(k, v)).sort((a, b) => b.requests - a.requests);
  }
  const touch = async <T>(value: T): Promise<T> => { await sleep(120); return clone(value); };

  return {
    gatewayStatus: () => touch(status),
    gatewayStart: () => { status = {...status, state: 'running', baseUrl: `http://127.0.0.1:${status.port}`, error: null}; return touch(status); },
    gatewayStop: () => { status = {...status, state: 'stopped', baseUrl: null, error: null}; return touch(status); },
    gatewayConfigure: (port, autostart) => {
      if (port < 1024 || port > 65535) return Promise.reject('Choose a port from 1024 to 65535');
      status = {...status, port, autostart, baseUrl: status.state === 'running' ? `http://127.0.0.1:${port}` : null};
      return touch(status);
    },
    presets: () => touch(PRESETS),
    connections: () => touch(connections.map(view)),
    addConnection: async connection => {
      await sleep(500);
      const preset = PRESETS.find(p => p.id === connection.preset);
      if (!preset) throw 'Unknown provider';
      const endpoint = (connection.endpoint?.trim() || preset.endpoint || '').replace(/\/+$/, '');
      if (!endpoint) throw 'Enter the endpoint URL';
      if (!/^https?:\/\//.test(endpoint)) throw 'The endpoint must start with https:// or http://';
      if (preset.needsKey && !connection.apiKey?.trim()) throw `${preset.name} needs an API key`;
      const label = connection.label.trim() || preset.name;
      let id = slug(label), n = 2;
      while (connections.some(c => c.id === id)) id = `${slug(label)}-${n++}`;
      const stored: Stored = {id, kind: preset.id, label, endpoint, enabled: true, credentialRef: connection.apiKey ? `provider-${hex(8)}` : null,
        defaultBilling: connection.billing, dataUseTerms: 'unknown', createdAt: Date.now(), key: !!connection.apiKey?.trim(),
        wire: preset.wire as Wire, separateQuota: false, quotaConfirmedAt: null};
      connections.push(stored);
      return view(stored);
    },
    updateConnection: async (id, change) => {
      const c = find(id);
      if (change.enabled !== undefined) c.enabled = change.enabled;
      if (change.billing) c.defaultBilling = change.billing;
      if (change.separateQuota !== undefined) {
        c.separateQuota = change.separateQuota;
        c.quotaConfirmedAt = change.separateQuota ? Date.now() : null;
      }
      return touch(view(c));
    },
    setConnectionKey: async id => {
      const c = find(id);
      c.key = true;
      for (let i = health.length - 1; i >= 0; i--) if (health[i].providerId === id) health.splice(i, 1);
      return touch(view(c));
    },
    clearConnectionHealth: async id => {
      for (let i = health.length - 1; i >= 0; i--) if (health[i].providerId === id) health.splice(i, 1);
      return touch(view(find(id)));
    },
    removeConnection: async id => {
      find(id);
      connections.splice(connections.findIndex(c => c.id === id), 1);
      for (let i = models.length - 1; i >= 0; i--) if (models[i].providerId === id) models.splice(i, 1);
      for (const combo of combos) combo.members = combo.members.filter(m => m.providerId !== id);
      await sleep(120);
    },
    refreshModels: async id => {
      await sleep(700);
      const c = find(id);
      if (c.kind !== 'ollama' && !c.key) throw 'The provider refused the key (HTTP 401): API key not valid.';
      const listed = DEMO_MODELS[c.kind] ?? [];
      const before = models.length;
      addModels(id, listed);
      return {listed: listed.length, added: models.length - before};
    },
    models: providerId => touch(models.filter(m => !providerId || m.providerId === providerId)
      .map(m => ({...m, billing: (m.billing === 'unknown' ? find(m.providerId).defaultBilling : m.billing) as Billing}))),
    addModel: async (providerId, modelId) => {
      if (!modelId.trim()) throw 'Enter a model name';
      addModels(providerId, [modelId.trim()]);
      await sleep(80);
    },
    updateModel: async (providerId, modelId, change) => {
      const m = models.find(x => x.providerId === providerId && x.modelId === modelId);
      if (!m) throw 'Model not found';
      if (change.enabled !== undefined) m.enabled = change.enabled;
      if (change.billing) m.billing = change.billing;
      if (change.clearPrice) m.price = null;
      else if (change.price) {
        const micro = (usd: number) => Math.round(usd * 1_000_000);
        m.price = {inputMicroPerMtok: micro(change.price.inputUsdPerMtok), outputMicroPerMtok: micro(change.price.outputUsdPerMtok),
          cachedInputMicroPerMtok: change.price.cachedInputUsdPerMtok == null ? null : micro(change.price.cachedInputUsdPerMtok),
          cacheWriteMicroPerMtok: change.price.cacheWriteUsdPerMtok == null ? null : micro(change.price.cacheWriteUsdPerMtok),
          source: 'user', observedAt: Date.now()};
      }
      await sleep(80);
    },
    removeModel: async (providerId, modelId) => {
      const i = models.findIndex(x => x.providerId === providerId && x.modelId === modelId);
      if (i >= 0) models.splice(i, 1);
      await sleep(80);
    },
    combos: () => touch(combos),
    saveCombo: async (id, name, members: Target[]) => {
      name = name.trim();
      if (!name || /[\s/]/.test(name)) throw 'A combo name needs at least one character and no spaces or slashes';
      if (!members.length) throw 'A combo needs at least one model';
      if (combos.some(c => c.name === name && c.id !== id)) throw `A combo named ${name} already exists`;
      let combo = combos.find(c => c.id === id);
      if (combo) Object.assign(combo, {name, members});
      else combos.push(combo = {id: hex(6), name, members});
      return touch(combo);
    },
    deleteCombo: async id => { const i = combos.findIndex(c => c.id === id); if (i >= 0) combos.splice(i, 1); await sleep(80); },
    clients: () => touch(clients.map(clientView)),
    createClient: async (name, monthlyBudgetUsd) => {
      if (!name.trim()) throw 'Enter a name for the client';
      const token = `str-${hex(32)}`;
      const client = {id: hex(6), name: name.trim(), tokenPrefix: token.slice(0, 12),
        monthlyBudgetMicroUsd: monthlyBudgetUsd == null ? null : Math.round(monthlyBudgetUsd * 1_000_000), createdAt: Date.now(),
        lastUsedAt: null, revokedAt: null, monthSpendMicroUsd: 0, monthUnknownRequests: 0, spend: 0,
        maxConcurrency: 8, unknownCostPolicy: 'block' as const, reservedMicroUsd: 0, monthAtRiskMicroUsd: 0,
        allowedConnections: null as string[] | null, paidRetries: 0};
      clients.push(client);
      return touch({client: clientView(client), token});
    },
    revokeClient: async id => { const c = clients.find(x => x.id === id); if (c) c.revokedAt = Date.now(); await sleep(80); },
    setClientBudget: async (id, usd) => {
      const c = clients.find(x => x.id === id);
      if (c) c.monthlyBudgetMicroUsd = usd == null ? null : Math.round(usd * 1_000_000);
      await sleep(80);
    },
    setClientPolicy: async (id, policy) => {
      const c = clients.find(x => x.id === id);
      if (policy.maxConcurrency !== undefined && (policy.maxConcurrency < 1 || policy.maxConcurrency > 256)) {
        throw 'Allow from 1 to 256 requests at once';
      }
      if (c && policy.maxConcurrency !== undefined) c.maxConcurrency = policy.maxConcurrency;
      if (c && policy.unknownCostPolicy) c.unknownCostPolicy = policy.unknownCostPolicy;
      if (c && policy.restrictConnections !== undefined) c.allowedConnections = policy.restrictConnections ? [...(policy.allowedConnections ?? [])] : null;
      if (c && policy.paidRetries !== undefined) c.paidRetries = policy.paidRetries;
      await sleep(80);
    },
    usage: async days => {
      const since = Date.now() - days * 86_400_000;
      const list = events.filter(e => e.createdAt >= since);
      const summary: UsageSummary = {since, total: group('all', list),
        byModel: groups(list, e => `${e.provider}/${e.model}`), byClient: groups(list, e => e.client ?? ''),
        recent: [...list].sort((a, b) => b.createdAt - a.createdAt).slice(0, 200)};
      return touch(summary);
    },
  };
}
