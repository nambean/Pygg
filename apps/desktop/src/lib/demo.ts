/**
 * Browser-preview stand-in for the core (pnpm dev), so the gateway screens can be reviewed without the
 * desktop app. Everything lives in memory, nothing is sent anywhere, and the interface shows a "Demo
 * data" banner whenever this is in use (ADR-014). It follows the core's rules (slugs, validation, honest
 * costs) closely enough for design review, not as a test of them.
 */
import type { AgentInfo, AutoExcluded, AutoPlan, AutoRanked, Billing, Capabilities, Client, Combo, Connection, GatewayApi, GatewayStatus,
  Health, Model, Preset, Target, Tier, UsageEvent, UsageGroup, UsageSummary, Wire } from './api';
import { TIERS } from './api';
import { SAMPLE_WORKSPACES, sampleTasks } from './sample';
import type { AttemptView, NewTask, TaskDetail, TaskSummary, WorkspaceView } from './tasks';

type BasePreset = Omit<Preset, 'offer' | 'offerNote'>;
const BASE_PRESETS: BasePreset[] = [
  {id: 'gemini', name: 'Google Gemini', endpoint: 'https://generativelanguage.googleapis.com/v1beta/openai', needsKey: true, defaultBilling: 'unknown', wire: 'openai'},
  {id: 'anthropic', name: 'Anthropic', endpoint: 'https://api.anthropic.com/v1', needsKey: true, defaultBilling: 'paid', wire: 'anthropic'},
  {id: 'openrouter', name: 'OpenRouter', endpoint: 'https://openrouter.ai/api/v1', needsKey: true, defaultBilling: 'paid', wire: 'openai'},
  {id: 'openai', name: 'OpenAI', endpoint: 'https://api.openai.com/v1', needsKey: true, defaultBilling: 'paid', wire: 'openai'},
  {id: 'groq', name: 'Groq', endpoint: 'https://api.groq.com/openai/v1', needsKey: true, defaultBilling: 'unknown', wire: 'openai'},
  ...([
    // Same list as gateway/presets.rs: [id, name, endpoint, billing, wire].
    ['deepseek', 'DeepSeek', 'https://api.deepseek.com', 'paid', 'openai'],
    ['deepseek-anthropic', 'DeepSeek (Anthropic API)', 'https://api.deepseek.com/anthropic/v1', 'paid', 'anthropic'],
    ['qwen', 'Qwen (Alibaba Model Studio)', 'https://dashscope-intl.aliyuncs.com/compatible-mode/v1', 'unknown', 'openai'],
    ['mistral', 'Mistral', 'https://api.mistral.ai/v1', 'unknown', 'openai'],
    ['cerebras', 'Cerebras', 'https://api.cerebras.ai/v1', 'paid', 'openai'],
    ['together', 'Together AI', 'https://api.together.ai/v1', 'paid', 'openai'],
    ['fireworks', 'Fireworks AI', 'https://api.fireworks.ai/inference/v1', 'paid', 'openai'],
    ['xai', 'xAI (Grok)', 'https://api.x.ai/v1', 'paid', 'openai'],
    ['nvidia', 'NVIDIA (build.nvidia.com)', 'https://integrate.api.nvidia.com/v1', 'unknown', 'openai'],
    ['sambanova', 'SambaNova Cloud', 'https://api.sambanova.ai/v1', 'unknown', 'openai'],
    ['huggingface', 'Hugging Face (Inference Providers)', 'https://router.huggingface.co/v1', 'paid', 'openai'],
    ['deepinfra', 'DeepInfra', 'https://api.deepinfra.com/v1/openai', 'paid', 'openai'],
    ['siliconflow', 'SiliconFlow', 'https://api.siliconflow.com/v1', 'unknown', 'openai'],
    ['moonshot', 'Kimi (Moonshot)', 'https://api.moonshot.ai/v1', 'paid', 'openai'],
    ['moonshot-anthropic', 'Kimi (Anthropic API)', 'https://api.moonshot.ai/anthropic/v1', 'paid', 'anthropic'],
    ['zai', 'Z.AI (GLM)', 'https://api.z.ai/api/paas/v4', 'unknown', 'openai'],
    ['zai-anthropic', 'Z.AI (Anthropic API)', 'https://api.z.ai/api/anthropic/v1', 'unknown', 'anthropic'],
    ['cohere', 'Cohere', 'https://api.cohere.ai/compatibility/v1', 'unknown', 'openai'],
    ['minimax', 'MiniMax', 'https://api.minimax.io/v1', 'paid', 'openai'],
    ['minimax-anthropic', 'MiniMax (Anthropic API)', 'https://api.minimax.io/anthropic/v1', 'paid', 'anthropic'],
    ['nebius', 'Nebius Token Factory', 'https://api.tokenfactory.nebius.com/v1', 'paid', 'openai'],
  ] as const).map(([id, name, endpoint, defaultBilling, wire]): BasePreset => ({id, name, endpoint, needsKey: true, defaultBilling, wire})),
  ...([
    ['ollama', 'Ollama', 'http://127.0.0.1:11434/v1'],
    ['lmstudio', 'LM Studio', 'http://localhost:1234/v1'],
    ['llamacpp', 'llama.cpp server', 'http://127.0.0.1:9931/v1'],
    ['vllm', 'vLLM', 'http://localhost:8000/v1'],
    ['docker-model-runner', 'Docker Model Runner', 'http://localhost:12434/engines/v1'],
    ['jan', 'Jan', 'http://127.0.0.1:1337/v1'],
  ] as const).map(([id, name, endpoint]): BasePreset => ({id, name, endpoint, needsKey: false, defaultBilling: 'local', wire: 'openai'})),
  {id: 'custom', name: 'Custom (OpenAI-compatible)', endpoint: null, needsKey: false, defaultBilling: 'unknown', wire: 'openai'},
  {id: 'custom-anthropic', name: 'Custom (Anthropic-compatible)', endpoint: null, needsKey: false, defaultBilling: 'unknown', wire: 'anthropic'},
];
/** What gateway/presets.rs says about starting with each provider. */
const OFFERS: Record<string, [Preset['offer'], string]> = {
  gemini: ['free', 'Free tier in Google AI Studio'], openrouter: ['free', 'Free models (:free)'], groq: ['free', 'Free plan'],
  mistral: ['free', 'Free mode, lowest limits'], nvidia: ['free', 'Developer Program access'],
  sambanova: ['free', 'Free tier without a card'], zai: ['free', 'Some free Flash models'],
  'zai-anthropic': ['free', 'Some free Flash models'], cohere: ['free', 'Trial key, 1,000 calls a month'],
  deepseek: ['credits', 'Granted balance'], 'deepseek-anthropic': ['credits', 'Granted balance'],
  cerebras: ['credits', '$5 trial credit, card needed'], fireworks: ['credits', 'Small starting credit'],
  huggingface: ['credits', 'Monthly credit with PRO'],
};
const PRESETS: Preset[] = BASE_PRESETS.map(p => {
  const offer = OFFERS[p.id];
  return {...p, offer: offer?.[0] ?? (p.endpoint === null ? 'custom' : p.defaultBilling === 'local' ? 'local' : 'paid'),
    offerNote: offer?.[1] ?? null};
});

/** As found on a developer machine: three agents installed, Codex signed in with a ChatGPT plan. */
const DEMO_AGENTS: AgentInfo[] = [
  {id: 'claude-code', name: 'Claude Code', installed: false, program: null, version: null, login: null,
    signIn: 'Your Claude subscription or Anthropic Console login, managed by Claude Code (`claude` then /login).',
    install: 'https://docs.claude.com/en/docs/claude-code/setup'},
  {id: 'codex', name: 'Codex CLI', installed: true, program: 'codex.exe', version: 'codex-cli 0.163.0', login: 'Logged in using ChatGPT',
    signIn: 'Your ChatGPT plan or an OpenAI API key, managed by Codex (`codex login`).', install: 'https://developers.openai.com/codex/cli'},
  {id: 'gemini-cli', name: 'Gemini CLI', installed: true, program: 'gemini.js', version: '0.66.0', login: null,
    signIn: 'A Gemini API key set in Gemini CLI (run `gemini`, choose the API key). Google no longer accepts its free Google-account login from this CLI.',
    install: 'https://github.com/google-gemini/gemini-cli'},
  {id: 'opencode', name: 'OpenCode', installed: true, program: 'opencode.exe', version: '1.18.4', login: null,
    signIn: 'The providers configured in OpenCode (`opencode auth login`).', install: 'https://opencode.ai/docs'},
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
  const UNKNOWN: Capabilities = {tier: null, contextTokens: null, maxOutputTokens: null, supportsTools: null, supportsVision: null,
    source: null, observedAt: null};
  const models: Model[] = [];
  const addModels = (providerId: string, ids: string[]) => {
    for (const modelId of ids) {
      if (!models.some(m => m.providerId === providerId && m.modelId === modelId)) {
        models.push({providerId, modelId, billing: connections.find(c => c.id === providerId)?.defaultBilling ?? 'unknown',
          enabled: true, price: null, observedAt: Date.now(), capabilities: {...UNKNOWN}});
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
  // What auto routing knows about some of them: sample values for the preview, as if the user entered them
  // (OpenRouter's as if read from its model list).
  const facts = (modelId: string, tier: Tier | null, context: number, output: number, tools: boolean | null, source = 'user') => {
    const m = models.find(x => x.modelId === modelId);
    if (m) m.capabilities = {tier, contextTokens: context, maxOutputTokens: output, supportsTools: tools, supportsVision: tools,
      source, observedAt: now - 2 * 86_400_000};
  };
  facts('gemini-3.1-flash-lite', 'T1', 1_000_000, 64_000, true);
  facts('gemini-3.1-pro-preview', 'T3', 1_000_000, 64_000, true);
  facts('claude-sonnet-4-5', 'T3', 200_000, 64_000, true);
  facts('claude-haiku-4-5', 'T2', 200_000, 64_000, true);
  facts('qwen/qwen3-coder', 'T2', 262_000, 32_000, true, 'provider: openrouter /models');
  facts('deepseek/deepseek-chat', null, 128_000, 8_000, true, 'provider: openrouter /models');
  facts('qwen2.5-coder:7b', 'T0', 32_000, 8_000, null);
  const haiku = models.find(m => m.modelId === 'claude-haiku-4-5')!;
  haiku.price = {inputMicroPerMtok: 1_000_000, outputMicroPerMtok: 5_000_000, cachedInputMicroPerMtok: 100_000,
    cacheWriteMicroPerMtok: 1_250_000, source: 'user', observedAt: now - 86_400_000};
  const qwen = models.find(m => m.modelId === 'qwen/qwen3-coder')!;
  qwen.price = {inputMicroPerMtok: 300_000, outputMicroPerMtok: 1_200_000, cachedInputMicroPerMtok: null,
    source: 'provider list price', observedAt: now - 2 * 86_400_000};
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
      allowedConnections: ['anthropic', 'gemini'], paidRetries: 0, autoMinTier: 'T2', mayRunTasks: true},
    {id: 'k2', name: 'Scripts', tokenPrefix: 'str-81d2e7aa', monthlyBudgetMicroUsd: null, createdAt: now - 3 * 86_400_000,
      lastUsedAt: now - 3 * 3_600_000, revokedAt: null, monthSpendMicroUsd: 0, monthUnknownRequests: 0, spend: 0,
      maxConcurrency: 4, unknownCostPolicy: 'allow', reservedMicroUsd: 0, monthAtRiskMicroUsd: 0,
      allowedConnections: null, paidRetries: 1, autoMinTier: 'T1', mayRunTasks: false},
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
      atRiskMicroUsd: null, routeReason: null as string | null};
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

  /** A short port of gateway/auto.rs for the preview: the same filters and order, on the demo models. */
  function simulate(clientId: string | null, alias: 'auto' | 'auto/cheap', needs: Parameters<GatewayApi['autoSimulate']>[2]): AutoPlan {
    const client = clients.find(c => c.id === clientId);
    const cheap = alias === 'auto/cheap';
    const minTier: Tier | null = cheap ? null : client?.autoMinTier ?? 'T2';
    const rank = (t: Tier) => TIERS.indexOf(t);
    const billingRank = (b: Billing) => ({local: 0, free: 1, subscription: 2} as Record<string, number>)[b] ?? 3;
    const output = needs.maxTokens ?? 4096;
    const ranked: AutoRanked[] = [], excluded: AutoExcluded[] = [];
    for (const m of models) {
      const conn = connections.find(c => c.id === m.providerId);
      if (!conn?.enabled || !m.enabled) continue;
      const caps = m.capabilities, billing = m.billing === 'unknown' ? conn.defaultBilling : m.billing;
      const no = (reason: string) => excluded.push({providerId: m.providerId, modelId: m.modelId, reason});
      if (client?.allowedConnections && !client.allowedConnections.includes(conn.id)) { no(`this client may not use ${conn.label}`); continue; }
      if (!needs.surfaceAnthropic && conn.wire === 'anthropic') { no('speaks the Anthropic API, which an OpenAI-format request cannot reach yet'); continue; }
      const h = health.find(x => x.providerId === conn.id && (x.modelId === '' || x.modelId === m.modelId) && (x.untilMs ?? Infinity) > Date.now());
      if (h) { no(`${h.state.replace('_', ' ')} (${h.reason})`); continue; }
      if (minTier) {
        if (!caps.tier) { no('no tier set'); continue; }
        if (rank(caps.tier) < rank(minTier)) { no(`tier ${caps.tier} is below this client's minimum ${minTier}`); continue; }
      }
      if (needs.tools && caps.supportsTools !== true) { no(caps.supportsTools === false ? 'does not take tools' : 'tool support unknown'); continue; }
      if (needs.vision && caps.supportsVision !== true) { no(caps.supportsVision === false ? 'does not take images' : 'image support unknown'); continue; }
      if (!caps.contextTokens) { no('context size unknown'); continue; }
      if (needs.inputTokens + output > caps.contextTokens) {
        no(`about ${needs.inputTokens} prompt and ${output} output tokens do not fit its ${caps.contextTokens}-token context`); continue;
      }
      if (needs.maxTokens != null) {
        if (caps.maxOutputTokens == null) { no('maximum output unknown, and the request sets one'); continue; }
        if (needs.maxTokens > caps.maxOutputTokens) { no(`the request allows ${needs.maxTokens} output tokens; it gives at most ${caps.maxOutputTokens}`); continue; }
      }
      let estimate = 0, cost = `${billing} (as set)`;
      if (billing === 'subscription') cost = "the plan's quota, not money; quota left not tracked";
      if (billing !== 'free' && billing !== 'local' && billing !== 'subscription') {
        if (!m.price) { no('price unknown'); continue; }
        const inPrice = Math.max(m.price.inputMicroPerMtok, m.price.cacheWriteMicroPerMtok ?? 0);
        estimate = Math.ceil(needs.inputTokens * inPrice / 1e6) + Math.ceil(output * m.price.outputMicroPerMtok / 1e6);
        cost = `about $${(estimate / 1e6).toFixed(4)} at ${m.price.source === 'user' ? 'your price' : "the provider's list price"}`;
      }
      const reason = cheap ? `${billing}; ${cost}`
        : `tier ${caps.tier} (minimum ${minTier}); ${billing}; ${cost}`;
      ranked.push({providerId: m.providerId, modelId: m.modelId, tier: caps.tier, billing, estimateMicroUsd: estimate, reason});
    }
    ranked.sort((a, b) => cheap
      ? a.estimateMicroUsd - b.estimateMicroUsd || billingRank(a.billing) - billingRank(b.billing) || rank(b.tier ?? 'T0') - rank(a.tier ?? 'T0')
      : rank(a.tier!) - rank(b.tier!) || billingRank(a.billing) - billingRank(b.billing) || a.estimateMicroUsd - b.estimateMicroUsd);
    return {alias, strategy: cheap ? 'cheap' : 'tier', minTier, ranked, excluded};
  }

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
    setModelCapabilities: async (providerId, modelId, capabilities) => {
      const m = models.find(x => x.providerId === providerId && x.modelId === modelId);
      if (!m) throw 'Model not found';
      m.capabilities = {...capabilities, source: 'user', observedAt: Date.now()};
      await sleep(80);
    },
    autoSimulate: async (clientId, alias, needs) => touch(simulate(clientId, alias, needs)),
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
        allowedConnections: null as string[] | null, paidRetries: 0, autoMinTier: 'T2' as Tier, mayRunTasks: false};
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
      if (c && policy.autoMinTier) c.autoMinTier = policy.autoMinTier;
      if (c && policy.mayRunTasks !== undefined) c.mayRunTasks = policy.mayRunTasks;
      await sleep(80);
    },
    agents: () => touch(DEMO_AGENTS),
    runAgent: async task => {
      await sleep(1500);
      const agent = DEMO_AGENTS.find(a => a.id === task.agent);
      if (!agent?.installed) throw `${agent?.name ?? task.agent} is not installed, or not on PATH.`;
      if (!task.prompt.trim()) throw 'Describe the task';
      return {id: hex(8), agent: task.agent, model: task.model, workspace: task.workspace, worktree: `${task.workspace}/../.worktrees/demo`,
        base: 'a91e7e7', outcome: 'ok', exit: 'exit 0', durationMs: 28_541, filesChanged: 1, insertions: 1, deletions: 1,
        usage: {model: null, inputTokens: 83_166, outputTokens: 291, cacheReadTokens: 65_664, cacheWriteTokens: null, estimateUsd: null,
          result: 'Updated `calc.py` so `add()` returns `a + b`. Nothing else changed.', error: null}, logTail: ''};
    },
    agentDiff: async () => touch('diff --git a/calc.py b/calc.py\n--- a/calc.py\n+++ b/calc.py\n@@ -1,2 +1,2 @@\n def add(a, b):\n-    return a - b\n+    return a + b\n'),
    finishAgentRun: async (runId, keep) => { await sleep(300); return keep ? `str/${keep.toLowerCase().replace(/[^a-z0-9]+/g, '-')}-${runId}` : null; },
    ...demoTasks(touch),
    mcpSetup: () => touch({path: 'C:\\Users\\you\\AppData\\Local\\SmartTaskRouter\\smarttaskrouter-desktop.exe', args: ['mcp'], exists: true, url: 'http://127.0.0.1:8741'}),
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

const DEMO_DIFF = 'diff --git a/src/timeout.ts b/src/timeout.ts\n--- a/src/timeout.ts\n+++ b/src/timeout.ts\n@@ -3,3 +3,3 @@\n-  return setTimeout(fn, ms * 100);\n+  return setTimeout(fn, ms);\n';

/** Tasks for the browser preview: the sample tasks, and new ones that "run" for a few seconds. Nothing runs. */
function demoTasks(touch: <T>(value: T) => Promise<T>): Pick<GatewayApi, 'workspaces' | 'addWorkspace' | 'removeWorkspace'
  | 'detectChecks' | 'workspaceCheck' | 'tasks' | 'task' | 'createTask' | 'cancelTask' | 'decideTask' | 'retryTask' | 'taskDiff'
  | 'finishTask'> {
  const workspaces: WorkspaceView[] = SAMPLE_WORKSPACES.map((w, i) => ({...w, tasks: 0, checks: i === 0
    ? [{command: 'pnpm install --frozen-lockfile', source: 'package.json', approved: true},
       {command: 'pnpm typecheck', source: 'package.json', approved: true}, {command: 'pnpm test', source: 'package.json', approved: true},
       {command: 'cargo test', source: 'Cargo.toml', approved: false}]
    : [{command: 'npm ci', source: 'package.json', approved: false}, {command: 'npm run build', source: 'package.json', approved: false}]}));
  const details = new Map<string, TaskDetail>();
  for (const s of sampleTasks()) details.set(s.id, detailOf(s));

  function detailOf(s: TaskSummary): TaskDetail {
    const ws = workspaces.find(w => w.id === s.workspaceId)!;
    const checks = ws.checks.filter(c => c.approved).map(({command, source}) => ({command, source}));
    const attempts: AttemptView[] = s.attempts.map((a, i) => ({
      id: `${s.id}-${i}`, number: i + 1, step: i, agent: a.model.startsWith('Codex') ? 'codex' : a.model.startsWith('Claude') ? 'claude-code'
        : a.model.startsWith('Gemini') ? 'gemini-cli' : 'opencode', label: a.model, agentModel: null, tier: a.tier, billing: a.billing,
      outcome: a.outcome, detail: a.outcome === 'running' || a.outcome === 'waiting' ? null : 'exit 0',
      escalationReason: a.outcome === 'failed' ? '`pnpm test` exited 1' : null,
      checks: a.outcome === 'passed' || a.outcome === 'failed' ? checks.map((c, j) => ({command: c.command,
        passed: a.outcome === 'passed' || j < checks.length - 1, detail: a.outcome === 'passed' || j < checks.length - 1 ? 'exited 0' : 'exited 1',
        outputTail: a.outcome === 'failed' && j === checks.length - 1 ? 'FAIL src/timeout.test.ts\n  expected 50 ms, got 5000 ms' : '',
        durationMs: 2400 + j * 900})) : [],
      filesChanged: a.outcome === 'waiting' ? null : 2, insertions: 6, deletions: 3,
      summary: a.outcome === 'passed' ? 'Fixed the unit conversion in timeout handling and added a regression test.' : null,
      logTail: null, inputTokens: a.outcome === 'waiting' ? null : 41_200, outputTokens: a.outcome === 'waiting' ? null : 1_830,
      startedAt: s.startedAt, finishedAt: a.outcome === 'running' || a.outcome === 'waiting' ? null : s.updatedAt,
      open: a.outcome === 'passed' && (s.state === 'checks_passed'),
    }));
    return {...s, prompt: s.title, strategy: 'free_first', reason: s.state === 'interrupted'
      ? 'The app closed while this task ran. Retry runs the step again from the same code.'
      : s.state === 'waiting_approval' ? 'Step 2 is marked paid, so it waits for your approval.' : null,
      result: null, resultBranch: null, baseRevision: 'a91e7e7', workspaceName: ws.name, workspacePath: ws.path,
      policy: {steps: s.attempts.map((a, i) => ({agent: attempts[i].agent, model: null, tier: a.tier, billing: a.billing})), checks,
        allowShell: false, timeoutSecs: 900},
      attempts: s.attempts, attemptDetails: attempts};
  }
  const summary = (d: TaskDetail): TaskSummary => ({id: d.id, title: d.title, workspaceId: d.workspaceId, state: d.state, phase: d.phase,
    attempts: d.attempts, cost: d.cost, startedAt: d.startedAt, updatedAt: d.updatedAt});
  const get = (id: string) => { const d = details.get(id); if (!d) throw 'That task no longer exists'; return d; };
  const set = (id: string, change: Partial<TaskDetail>) => { details.set(id, {...get(id), ...change, updatedAt: Date.now()}); };
  const findWorkspace = (id: string) => { const w = workspaces.find(x => x.id === id); if (!w) throw 'That workspace no longer exists'; return w; };

  return {
    workspaces: () => touch(workspaces.map(w => ({...w, tasks: [...details.values()].filter(d => d.workspaceId === w.id).length}))),
    addWorkspace: async (path, name) => {
      await sleep(200);
      if (!path.trim()) throw 'Choose the workspace folder';
      if (workspaces.some(w => w.path === path.trim())) throw 'That folder is already a workspace';
      const w: WorkspaceView = {id: hex(6), name: name?.trim() || path.trim().split(/[\\/]/).filter(Boolean).pop() || path, path: path.trim(),
        tasks: 0, checks: [{command: 'npm ci', source: 'package.json', approved: false}, {command: 'npm run test', source: 'package.json', approved: false}]};
      workspaces.push(w);
      return clone(w);
    },
    removeWorkspace: async id => {
      if ([...details.values()].some(d => d.workspaceId === id)) throw 'This workspace has task history, so it stays.';
      workspaces.splice(workspaces.findIndex(w => w.id === id), 1);
    },
    detectChecks: id => touch(findWorkspace(id)),
    workspaceCheck: async (id, command, action) => {
      const w = findWorkspace(id);
      const c = w.checks.find(x => x.command === command);
      if (action === 'add') { if (!command.trim()) throw 'Type the command, e.g. pnpm test'; if (c) c.approved = true; else w.checks.push({command: command.trim(), source: 'user', approved: true}); }
      else if (action === 'remove') w.checks = w.checks.filter(x => x.command !== command);
      else if (c) c.approved = action === 'approve';
      return touch(w);
    },
    tasks: () => touch([...details.values()].map(summary).sort((a, b) => b.updatedAt - a.updatedAt)),
    task: async id => touch(get(id)),
    createTask: async (task: NewTask) => {
      await sleep(250);
      if (!task.prompt.trim()) throw 'Describe the task';
      if (task.steps.length === 0) throw 'Add at least one step';
      const w = findWorkspace(task.workspaceId);
      const id = `t-${hex(4)}`;
      const first = task.steps[0];
      const label = `${DEMO_AGENTS.find(a => a.id === first.agent)?.name ?? first.agent} · ${first.model ?? 'default model'}`;
      const now = Date.now();
      const base: TaskSummary = {id, title: task.title ?? task.prompt.trim().split('\n')[0].slice(0, 80), workspaceId: w.id, state: 'running',
        phase: 'generating', attempts: [{tier: first.tier, billing: first.billing, model: label, outcome: 'running'}],
        cost: {actualUsd: 0, estimatedUsd: 0, unknownCalls: 0, localOnly: false}, startedAt: now, updatedAt: now};
      details.set(id, {...detailOf(base), prompt: task.prompt, strategy: task.strategy,
        policy: {steps: task.steps, checks: w.checks.filter(c => c.approved).map(({command, source}) => ({command, source})),
          allowShell: task.allowShell, timeoutSecs: task.timeoutSecs ?? 900}});
      setTimeout(() => { if (details.get(id)?.state === 'running') set(id, {phase: 'checking'}); }, 2500);
      setTimeout(() => {
        const d = details.get(id);
        if (d?.state !== 'running') return;
        const done = {...detailOf({...summary(d), state: 'checks_passed', attempts: [{...d.attempts[0], outcome: 'passed'}],
          cost: {actualUsd: 0, estimatedUsd: 0, unknownCalls: 1, localOnly: false}})};
        set(id, {state: 'checks_passed', phase: undefined, attempts: done.attempts, cost: done.cost, attemptDetails: done.attemptDetails});
      }, 5000);
      return id;
    },
    cancelTask: async id => { const d = get(id); if (!['running', 'waiting_approval', 'interrupted'].includes(d.state)) throw 'Only a running or waiting task can be stopped';
      set(id, {state: 'canceled', phase: undefined, reason: 'You stopped it', attempts: d.attempts.map(a => a.outcome === 'running' || a.outcome === 'waiting' ? {...a, outcome: 'canceled'} : a)}); },
    decideTask: async (id, approve) => { await sleep(150);
      set(id, approve ? {state: 'running', phase: 'generating', reason: null, attempts: get(id).attempts.map(a => a.outcome === 'waiting' ? {...a, outcome: 'running'} : a)}
        : {state: 'blocked', reason: 'You declined the paid step, and no step after it is free to run', attempts: get(id).attempts.map(a => a.outcome === 'waiting' ? {...a, outcome: 'canceled'} : a)}); },
    retryTask: async id => { if (get(id).state !== 'interrupted') throw 'Only an interrupted task can be retried';
      set(id, {state: 'running', phase: 'preparing', reason: null}); },
    taskDiff: () => touch(DEMO_DIFF),
    finishTask: async (id, keep, title) => { await sleep(250); const d = get(id);
      const branch = keep ? `str/${(title || d.title).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 48)}-${id.replace(/\W/g, '').slice(0, 8)}` : null;
      set(id, {result: keep ? 'kept' : 'discarded', resultBranch: branch, state: d.state === 'needs_review' ? (keep ? 'manually_accepted' : 'canceled') : d.state});
      return branch; },
  };
}