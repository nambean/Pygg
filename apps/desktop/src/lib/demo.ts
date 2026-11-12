/**
 * Browser-preview stand-in for the core (pnpm dev), so the gateway screens can be reviewed without the
 * desktop app. Everything lives in memory, nothing is sent anywhere, and the interface shows a "Demo
 * data" banner whenever this is in use (ADR-014). It follows the core's rules (slugs, validation, honest
 * costs) closely enough for design review, not as a test of them.
 */
import type { AgentInfo, Integration, Launcher, IntegrationPlan, AutoExcluded, AutoPlan, AutoRanked, Billing, Capabilities, Client, Combo, Connection, GatewayApi, GatewayStatus,
  Health, Model, Preset, Target, Tier, UsageEvent, UsageBucket, UsageGroup, UsageSummary, Account, Wire,
  LocalUsage, SubscriptionUsage, Retention } from './api';
import { TIERS } from './api';
import { SAMPLE_WORKSPACES, sampleTasks } from './sample';
import type { AttemptView, NewTask, TaskDetail, TaskSummary, WorkspaceView } from './tasks';

type BasePreset = Omit<Preset, 'offer' | 'offerNote' | 'freeModels'>;
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
  const freeModels = ['openrouter', 'zai', 'zai-anthropic'].includes(p.id);
  return {...p, offer: offer?.[0] ?? (p.endpoint === null ? 'custom' : p.defaultBilling === 'local' ? 'local' : 'paid'),
    offerNote: offer?.[1] ?? null, freeModels, defaultBilling: freeModels ? 'free' : p.defaultBilling};
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
  qwen: ['qwen3-coder-plus', 'qwen3-coder-flash'],
  custom: [],
};

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const clone = <T>(value: T): T => structuredClone(value);
const hex = (n: number) => Array.from(crypto.getRandomValues(new Uint8Array(n)), b => b.toString(16).padStart(2, '0')).join('');

function slug(label: string): string {
  const out = label.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 32);
  return out || 'connection';
}

type Stored = Omit<Connection, 'presetName' | 'hasKey' | 'needsKey' | 'models' | 'health' | 'sharesQuotaWith' | 'quotaGroup' | 'freeModels'>
  & {key: boolean};

export function demoApi(): GatewayApi {
  const now = Date.now();
  let status: GatewayStatus = {state: 'running', port: 8741, baseUrl: 'http://127.0.0.1:8741', error: null, autostart: true,
    accountingError: null, finishing: false};
  const accounts = new Map<string, Account>();
  const connections: Stored[] = [
    {id: 'gemini', kind: 'gemini', label: 'Gemini', endpoint: PRESETS[0].endpoint!, enabled: true, credentialRef: 'provider-demo1',
      defaultBilling: 'unknown', dataUseTerms: 'unknown', termsSource: null, termsCheckedAt: null, createdAt: now - 6 * 86_400_000, key: true, wire: 'openai', separateQuota: false, quotaConfirmedAt: null},
    {id: 'anthropic', kind: 'anthropic', label: 'Anthropic', endpoint: PRESETS[1].endpoint!, enabled: true, credentialRef: 'provider-demo3',
      defaultBilling: 'paid', dataUseTerms: 'no_training', termsSource: 'anthropic.com/legal/commercial-terms (read 2026-11-10)', termsCheckedAt: now - 86_400_000, createdAt: now - 6 * 86_400_000, key: true, wire: 'anthropic', separateQuota: false, quotaConfirmedAt: null},
    {id: 'openrouter', kind: 'openrouter', label: 'OpenRouter', endpoint: PRESETS[1].endpoint!, enabled: true, credentialRef: 'provider-demo2',
      defaultBilling: 'paid', dataUseTerms: 'unknown', termsSource: null, termsCheckedAt: null, createdAt: now - 5 * 86_400_000, key: true, wire: 'openai', separateQuota: false, quotaConfirmedAt: null},
    {id: 'ollama', kind: 'ollama', label: 'Ollama', endpoint: PRESETS[5].endpoint!, enabled: true, credentialRef: null,
      defaultBilling: 'local', dataUseTerms: 'unknown', termsSource: null, termsCheckedAt: null, createdAt: now - 4 * 86_400_000, key: false, wire: 'openai', separateQuota: false, quotaConfirmedAt: null},
    {id: 'qwen-plan', kind: 'qwen', label: 'Qwen Coding Plan', endpoint: PRESETS.find(p => p.id === 'qwen')!.endpoint!, enabled: true, credentialRef: 'provider-demo4',
      defaultBilling: 'subscription', dataUseTerms: 'unknown', termsSource: null, termsCheckedAt: null, createdAt: now - 3 * 86_400_000, key: true, wire: 'openai', separateQuota: false, quotaConfirmedAt: null},
  ];
  const UNKNOWN: Capabilities = {tier: null, contextTokens: null, maxOutputTokens: null, supportsTools: null, supportsVision: null,
    thinkingTypes: null, effortLevels: null, tierSource: null, tierNote: null, source: null, observedAt: null};
  const models: Model[] = [];
  const addModels = (providerId: string, ids: string[]) => {
    for (const modelId of ids) {
      if (!models.some(m => m.providerId === providerId && m.modelId === modelId)) {
        models.push({providerId, modelId, billing: connections.find(c => c.id === providerId)?.defaultBilling ?? 'unknown',
          enabled: true, price: null, observedAt: Date.now(), capabilities: {...UNKNOWN}, enabledSource: null});
      }
    }
  };
  addModels('gemini', DEMO_MODELS.gemini);
  addModels('anthropic', DEMO_MODELS.anthropic);
  addModels('openrouter', DEMO_MODELS.openrouter);
  addModels('ollama', DEMO_MODELS.ollama);
  addModels('qwen-plan', DEMO_MODELS.qwen);
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
      // As Anthropic's model list reports them: the newer model takes adaptive thinking, the older only a budget.
      thinkingTypes: m.providerId !== 'anthropic' ? null : modelId === 'claude-sonnet-4-5' ? ['adaptive'] : ['enabled'],
      effortLevels: m.providerId !== 'anthropic' ? null : modelId === 'claude-sonnet-4-5' ? ['low', 'medium', 'high', 'max'] : [],
      tierSource: tier ? 'user' : null, tierNote: null, source, observedAt: now - 2 * 86_400_000};
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
      allowedConnections: ['anthropic', 'gemini'], paidRetries: 0, autoMinTier: 'T2', mayRunTasks: true, requireNoTraining: false, strictBudget: false,
      tokenViewable: true},
    {id: 'k2', name: 'Scripts', tokenPrefix: 'str-81d2e7aa', monthlyBudgetMicroUsd: null, createdAt: now - 3 * 86_400_000,
      lastUsedAt: now - 3 * 3_600_000, revokedAt: null, monthSpendMicroUsd: 0, monthUnknownRequests: 0, spend: 0,
      maxConcurrency: 4, unknownCostPolicy: 'allow', reservedMicroUsd: 0, monthAtRiskMicroUsd: 0,
      allowedConnections: null, paidRetries: 1, autoMinTier: 'T1', mayRunTasks: false, requireNoTraining: false, strictBudget: false},
    {id: 'k0', name: 'Old script', tokenPrefix: 'str-7a0c2d91', monthlyBudgetMicroUsd: null, createdAt: now - 9 * 86_400_000,
      lastUsedAt: now - 8 * 86_400_000, revokedAt: now - 7 * 86_400_000, monthSpendMicroUsd: 0, monthUnknownRequests: 0, spend: 0,
      maxConcurrency: 4, unknownCostPolicy: 'allow', reservedMicroUsd: 0, monthAtRiskMicroUsd: 0,
      allowedConnections: null, paidRetries: 0, autoMinTier: 'T1', mayRunTasks: false, requireNoTraining: false, strictBudget: false},
  ];

  let savingsBaseline: string | null = 'anthropic/claude-sonnet-4-5';
  let launcher: Launcher = {setUp: false, clientId: null, clientName: null, setUpAt: null, installed: true,
    cliPath: 'C:\\Users\\you\\AppData\\Local\\Pygg\\pygg-cli.exe'};

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

  // A coding plan billed by the month, and more local runs: the Subscriptions and Local Runtime tabs (SCREENS.md §5.6).
  for (let i = 0; i < 40; i++) {
    const at = now - Math.round(i * 55 * 60_000 * (0.6 + random()));
    const input = Math.round(3_000 + random() * 40_000), output = Math.round(200 + random() * 3_000);
    const base = {createdAt: at, client: 'Claude Code', error: null, cacheTokens: null, cacheWriteTokens: null, sessionKey: null,
      routeMode: 'native', atRiskMicroUsd: null, routeReason: null, httpStatus: 200};
    if (i % 3 === 2) {
      const small = i % 2 === 0;
      events.push({...base, id: `l${i}`, requestedModel: 'local-first', provider: 'ollama', model: small ? 'llama3.2:3b' : 'qwen2.5-coder:7b',
        inputTokens: input, outputTokens: output, costMicroUsd: 0, costStatus: 'estimated', costSource: 'local model', outcome: 'ok',
        latencyMs: Math.round((small ? 1_500 : 6_000) + random() * 9_000)});
    } else {
      const failed = random() < 0.06;
      events.push({...base, id: `s${i}`, requestedModel: 'qwen-plan/qwen3-coder-plus', provider: 'qwen-plan',
        model: i % 5 === 0 ? 'qwen3-coder-flash' : 'qwen3-coder-plus', inputTokens: failed ? null : input, outputTokens: failed ? null : output,
        costMicroUsd: null, costStatus: 'unknown', costSource: 'subscription: counts against a quota, not money',
        outcome: failed ? 'error' : 'ok', httpStatus: failed ? 429 : 200, latencyMs: Math.round(800 + random() * 4_000),
        error: failed ? 'Qwen answered HTTP 429: Requests rate limit exceeded.' : null});
    }
  }

  // Older history, outside every Usage period, for Settings → Usage History to count and delete.
  for (let i = 0; i < 12; i++) {
    events.push({id: `h${i}`, createdAt: now - (40 + i * 15) * 86_400_000, client: 'Scripts', requestedModel: 'coding', provider: 'gemini',
      model: 'gemini-3.1-flash-lite', inputTokens: 4_000, outputTokens: 300, costMicroUsd: 520, costStatus: 'estimated',
      costSource: 'price: user', outcome: 'ok', httpStatus: 200, latencyMs: 900, error: null, cacheTokens: null, cacheWriteTokens: null,
      sessionKey: null, routeMode: 'native', atRiskMicroUsd: null, routeReason: null});
  }

  // `?empty` starts from nothing, to review first-run states.
  if (new URLSearchParams(location.search).has('empty')) {
    connections.length = 0;
    models.length = 0;
    combos.length = 0;
    clients.length = 0;
    events.length = 0;
  }

  let retention: Retention = {days: null, lastRun: null};
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
      freeModels: preset?.freeModels ?? false,
      readsAccount: c.kind === 'openrouter' || c.kind === 'deepseek', account: accounts.get(c.id) ?? null,
      models: models.filter(m => m.providerId === c.id).length,
      health: health.filter(h => h.providerId === c.id && (h.untilMs === null || h.untilMs > Date.now())),
      sharesQuotaWith: connections.filter(o => o.id !== c.id && quotaGroup(o) === quotaGroup(c)).map(o => o.label),
      // Illustrative: what Anthropic's API sends with an answer.
      limits: c.kind === 'qwen' ? planLimits() : c.kind === 'anthropic' ? [{modelId: 'claude-sonnet-4-5', httpStatus: 200, observedAt: Date.now() - 6 * 60_000, headers: {
        'anthropic-ratelimit-requests-limit': '50', 'anthropic-ratelimit-requests-remaining': '49',
        'anthropic-ratelimit-tokens-remaining': '39000', 'anthropic-ratelimit-tokens-reset': '2026-11-12T02:31:00Z'}}] : []};
  }
  /** Illustrative: the windows a coding plan states with each answer, already read as quota.rs reads them. */
  function planLimits() {
    const observedAt = Date.now() - 3 * 60_000;
    return [{modelId: 'qwen3-coder-plus', httpStatus: 200, observedAt, headers: {'x-ratelimit-limit-requests': '6000',
      'x-ratelimit-remaining-requests': '5212', 'x-ratelimit-reset-requests': '4h12m'},
      windows: [{kind: 'requests', limit: 6000, remaining: 5212, resetsAt: observedAt + (4 * 60 + 12) * 60_000}]}];
  }
  /** Which tab a ledger row belongs to, as store.rs `row_billing!` decides it. */
  function rowBilling(e: UsageEvent): Billing {
    if (e.costSource.startsWith('subscription')) return 'subscription';
    if (e.costSource === 'local model') return 'local';
    if ((e.costMicroUsd ?? 0) > 0) return 'paid';
    return connections.find(c => c.id === e.provider)?.defaultBilling ?? 'unknown';
  }
  const tally = (list: UsageEvent[]) => ({requests: list.length, notOk: list.filter(e => e.outcome !== 'ok').length,
    inputTokens: list.reduce((s, e) => s + (e.inputTokens ?? 0), 0), outputTokens: list.reduce((s, e) => s + (e.outputTokens ?? 0), 0),
    lastUsedAt: list.length ? Math.max(...list.map(e => e.createdAt)) : null});
  function median(values: number[]): number | null {
    if (!values.length) return null;
    const sorted = [...values].sort((a, b) => a - b), mid = Math.floor(sorted.length / 2);
    return sorted.length % 2 ? sorted[mid] : Math.ceil((sorted[mid - 1] + sorted[mid]) / 2);
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
      if (conn.wire === 'anthropic' && needs.thinking) {
        if (caps.thinkingTypes === null) { no(`unknown whether it takes thinking: ${needs.thinking} (refresh its model list, or set it)`); continue; }
        if (!caps.thinkingTypes.includes(needs.thinking)) { no(`does not take thinking: ${needs.thinking}`); continue; }
      }
      if (conn.wire === 'anthropic' && needs.effort) {
        if (caps.effortLevels === null) { no(`unknown whether it takes effort: ${needs.effort} (refresh its model list, or set it)`); continue; }
        if (!caps.effortLevels.includes(needs.effort)) { no(`does not take effort: ${needs.effort}`); continue; }
      }
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
    // The preview cannot open a browser sign-in; it connects as if the provider had issued a key.
    async signInConnection(connection) { return this.addConnection({...connection, apiKey: 'sk-or-v1-demo'}); },
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
        defaultBilling: connection.billing, dataUseTerms: 'unknown', termsSource: null, termsCheckedAt: null, createdAt: Date.now(), key: !!connection.apiKey?.trim(),
        wire: preset.wire as Wire, separateQuota: false, quotaConfirmedAt: null};
      connections.push(stored);
      return view(stored);
    },
    // The preview sends nothing: a sample answer.
    latestRequests: async () => touch(clients.flatMap(c => {
      const e = events.filter(x => x.client === c.name).sort((a, b) => b.createdAt - a.createdAt)[0];
      return e ? [{clientId: c.id, createdAt: e.createdAt, provider: e.provider, model: e.model, requestedModel: e.requestedModel, outcome: e.outcome}] : [];
    })),
    // The preview's own figures: Flash-Lite and Qwen answers priced again at Claude Sonnet's list price.
    savings: async days => {
      const since = Date.now() - days * 86_400_000;
      const shown = events.filter(e => e.createdAt >= since);
      const ok = shown.filter(e => e.outcome === 'ok');
      const priced = ok.filter(e => e.costStatus !== 'unknown' && e.inputTokens !== null && e.outputTokens !== null);
      const failed = shown.filter(e => e.outcome !== 'ok' && e.costStatus !== 'unknown').reduce((n, e) => n + (e.costMicroUsd ?? 0), 0);
      const baseline = savingsBaseline;
      return touch({
        baseline, baselinePriced: baseline !== null,
        answered: {free: 0, local: ok.filter(e => e.provider === 'ollama').length, subscription: 0,
          paid: ok.filter(e => e.provider !== 'ollama' && e.costStatus !== 'unknown').length, unknown: ok.filter(e => e.costStatus === 'unknown').length},
        compared: baseline ? priced.length : 0,
        routedMicroUsd: baseline ? priced.reduce((n, e) => n + (e.costMicroUsd ?? 0), 0) + failed : 0,
        baselineMicroUsd: baseline ? priced.reduce((n, e) => n + (e.inputTokens ?? 0) * 2 + (e.outputTokens ?? 0) * 10, 0) : 0,
        failedAttemptsMicroUsd: baseline ? failed : 0,
        leftOutUnknownCost: baseline ? ok.filter(e => e.costStatus === 'unknown').length : 0, leftOutNoTokens: 0,
        failedUnknownCost: shown.filter(e => e.outcome !== 'ok' && e.costStatus === 'unknown').length,
      });
    },
    setSavingsBaseline: async baseline => { savingsBaseline = baseline; },
    launcher: async () => touch({...launcher}),
    launcherSetUp: async () => {
      await sleep(400);
      clients.push({id: 'k-launcher', name: 'Claude Code (launcher)', tokenPrefix: 'str-5c0e9a12', monthlyBudgetMicroUsd: null, createdAt: Date.now(),
        lastUsedAt: null, revokedAt: null, monthSpendMicroUsd: 0, monthUnknownRequests: 0, spend: 0, maxConcurrency: 8,
        unknownCostPolicy: 'block', reservedMicroUsd: 0, monthAtRiskMicroUsd: 0, allowedConnections: null, paidRetries: 0, autoMinTier: 'T2',
        mayRunTasks: false, requireNoTraining: false, strictBudget: false});
      launcher = {...launcher, setUp: true, clientId: 'k-launcher', clientName: 'Claude Code (launcher)', setUpAt: Date.now()};
      return touch({...launcher});
    },
    launcherRemove: async () => {
      const c = clients.find(x => x.id === launcher.clientId);
      if (c) c.revokedAt = Date.now();
      launcher = {...launcher, setUp: false, clientId: null, clientName: null, setUpAt: null};
      return touch({...launcher});
    },
    reliability: async id => touch(Object.entries(events.filter(e => e.provider === id).reduce<Record<string, {n: number; ok: number; ms: number[]}>>((acc, e) => {
      const r = acc[e.model] ??= {n: 0, ok: 0, ms: []};
      r.n += 1;
      if (e.outcome === 'ok') { r.ok += 1; if (e.latencyMs !== null) r.ms.push(e.latencyMs); }
      return acc;
    }, {})).map(([modelId, r]) => ({modelId, requests: r.n, ok: r.ok, medianLatencyMs: r.ms.length ? r.ms.sort((a, b) => a - b)[Math.floor(r.ms.length / 2)] : null}))),
    testModel: async () => { await sleep(600); return {ok: true, text: 'ok', latencyMs: 640, inputTokens: 14, outputTokens: 2, route: 'native'}; },
    setConnectionPool: async (id, pool) => {
      const c = find(id);
      if (pool && !c.separateQuota) throw 'Confirm first that this connection has its own provider quota (Independent quota confirmed): a pool is for separate accounts, not several keys of one account';
      c.pool = pool;
      c.poolRank = pool ? connections.filter(x => x.pool === pool).length : null;
      return touch(view(c));
    },
    moveInPool: async (id, up) => {
      const c = find(id);
      const members = connections.filter(x => x.pool && x.pool === c.pool).sort((a, b) => (a.poolRank ?? 0) - (b.poolRank ?? 0));
      const at = members.indexOf(c), to = up ? at - 1 : at + 1;
      if (to >= 0 && to < members.length) [members[at], members[to]] = [members[to], members[at]];
      members.forEach((m, i) => { m.poolRank = i + 1; });
      await touch(undefined);
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
    // As OpenRouter's /key would answer for a key that has never bought credit (sample figures, ADR-014).
    checkAccount: async id => {
      const c = connections.find(x => x.id === id);
      if (!c) throw 'Connection not found';
      if (c.kind === 'openrouter') accounts.set(id, {source: 'openrouter /key', observedAt: Date.now(), balance: null, currency: null,
        available: null, usedToday: 0,
        freeRequests: {kind: 'free_requests_per_day', limit: 50, remaining: 38, resetsAt: Math.ceil(Date.now() / 86_400_000) * 86_400_000}});
      return touch(view(c));
    },
    clearConnectionHealth: async id => {
      for (let i = health.length - 1; i >= 0; i--) if (health[i].providerId === id) health.splice(i, 1);
      return touch(view(find(id)));
    },
    setConnectionTerms: async (id, terms, source) => {
      if (terms === 'no_training' && !source?.trim()) throw 'Say where the terms say so (a link to the provider’s policy, or the page and date you read)';
      const c = find(id);
      c.dataUseTerms = terms; c.termsSource = source?.trim() || null; c.termsCheckedAt = terms === 'unknown' ? null : Date.now();
      return touch(view(c));
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
        allowedConnections: null as string[] | null, paidRetries: 0, autoMinTier: 'T2' as Tier, mayRunTasks: false, requireNoTraining: false, strictBudget: false,
        tokenViewable: true};
      clients.push(client);
      return touch({client: clientView(client), token});
    },
    revokeClient: async id => { const c = clients.find(x => x.id === id); if (c) { c.revokedAt = Date.now(); c.tokenViewable = false; } await sleep(80); },
    clientToken: async id => {
      const c = clients.find(x => x.id === id);
      if (!c?.tokenViewable || c.revokedAt) throw new Error('This key was made before Pygg kept a copy, or it was revoked, so it can’t be shown.');
      return `${c.tokenPrefix}${'0'.repeat(56)}`;
    },
    deleteClient: async id => { const i = clients.findIndex(x => x.id === id && x.revokedAt); if (i >= 0) clients.splice(i, 1); await sleep(80); },
    deleteRevokedClients: async () => {
      const n = clients.filter(c => c.revokedAt).length;
      for (let i = clients.length - 1; i >= 0; i--) if (clients[i].revokedAt) clients.splice(i, 1);
      return n;
    },
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
      if (c && policy.requireNoTraining !== undefined) c.requireNoTraining = policy.requireNoTraining;
      if (c && policy.strictBudget !== undefined) c.strictBudget = policy.strictBudget;
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
    mcpSetup: () => touch({path: 'C:\\Users\\you\\AppData\\Local\\Pygg\\Pygg.exe', args: ['mcp'], exists: true, url: 'http://127.0.0.1:8741'}),
    ...demoIntegrations(touch),
    usage: async days => {
      const since = Date.now() - days * 86_400_000;
      const list = events.filter(e => e.createdAt >= since);
      const summary: UsageSummary = {since, total: group('all', list),
        byModel: groups(list, e => `${e.provider}/${e.model}`), byClient: groups(list, e => e.client ?? ''),
        recent: [...list].sort((a, b) => b.createdAt - a.createdAt).slice(0, 200)};
      return touch(summary);
    },
    usageSubscriptions: async days => {
      const list = events.filter(e => e.createdAt >= Date.now() - days * 86_400_000 && rowBilling(e) === 'subscription');
      const out: SubscriptionUsage[] = connections.filter(c => c.defaultBilling === 'subscription').map(c => {
        const v = view(c);
        return {providerId: c.id, label: c.label, kind: c.kind, ...tally(list.filter(e => e.provider === c.id)), limits: v.limits ?? [],
          account: v.account ?? null};
      });
      return touch(out.sort((a, b) => b.requests - a.requests));
    },
    usageLocal: async days => {
      const list = events.filter(e => e.createdAt >= Date.now() - days * 86_400_000 && rowBilling(e) === 'local');
      const out: LocalUsage[] = connections.filter(c => c.defaultBilling === 'local').map(c => {
        const mine = list.filter(e => e.provider === c.id);
        const models = [...new Set(mine.map(e => e.model))].map(model => {
          const rows = mine.filter(e => e.model === model);
          return {model, ...tally(rows), medianLatencyMs: median(rows.filter(e => e.outcome === 'ok' && e.latencyMs !== null).map(e => e.latencyMs!))};
        }).sort((a, b) => b.requests - a.requests);
        return {providerId: c.id, label: c.label, kind: c.kind, models};
      });
      return touch(out);
    },
    // Sample alerts for the preview (ADR-014): one client near its limit, and requests without a price.
    // Sample figures (ADR-014): an agent that re-read a few files.
    usageShadow: async () => touch({requests: 48, requestsWithRepeats: 11, repeatedTokens: 38_400, inputTokens: 1_420_000,
      cacheReadTokens: 1_050_000}),
    usageAlerts: async () => touch([{kind: 'near_limit', client: 'Claude Code', value: 8_400_000, against: 10_000_000},
      {kind: 'unknown_costs', client: null, value: 6, against: 0}]),
    exportUsage: async () => { await sleep(200); return 'C:\\Users\\you\\Downloads\\pygg-usage.csv'; },
    retention: () => touch(retention),
    // The preview deletes nothing: it only counts what the setting would remove, as retention.rs decides it.
    retentionPreview: async days => touch(events.filter(e => e.createdAt < Math.min(Date.now() - days * 86_400_000, monthStart())).length),
    setRetention: async days => {
      if (days !== null) {
        const before = Math.min(Date.now() - days * 86_400_000, monthStart());
        const kept = events.filter(e => e.createdAt >= before);
        retention = {days, lastRun: {at: Date.now(), before, usageRows: events.length - kept.length, routeDecisions: 0}};
        events.splice(0, events.length, ...kept);
      } else retention = {...retention, days};
      return touch(retention);
    },
    exportDiagnostics: async () => { await sleep(200); return 'C:\\Users\\you\\Downloads\\pygg-diagnostics.zip'; },
    usageOverTime: async days => {
      const size = days <= 1 ? 3_600_000 : 86_400_000;
      const offset = -new Date().getTimezoneOffset() * 60_000;
      const since = Math.floor((Date.now() - days * 86_400_000 + offset) / size) * size - offset;
      const buckets = new Map<number, UsageBucket>();
      for (const e of events.filter(e => e.createdAt >= since)) {
        const start = Math.floor((e.createdAt + offset) / size) * size - offset;
        const b = buckets.get(start) ?? {start, requests: 0, failed: 0, knownMicroUsd: 0, actualMicroUsd: 0, unknownRequests: 0};
        b.requests += 1;
        b.failed += e.outcome === 'ok' || e.outcome === 'fallback' ? 0 : 1;
        b.knownMicroUsd += e.costMicroUsd ?? 0;
        b.actualMicroUsd += e.costStatus === 'actual' ? e.costMicroUsd ?? 0 : 0;
        b.unknownRequests += e.costStatus === 'unknown' ? 1 : 0;
        buckets.set(start, b);
      }
      return touch([...buckets.values()].sort((a, b) => a.start - b.start));
    },
    // Sample figures for the preview, under its demo banner (ADR-014).
    taskQuality: () => touch({tasks: 6, kept: 4, discarded: 1, retried: 1, cancelled: 1, checkedAttempts: 8, passedAttempts: 5,
      knownMicroUsd: 48_000, unknownEntries: 3, microUsdPerKept: 12_000}),
  };
}

const DEMO_DIFF = 'diff --git a/src/timeout.ts b/src/timeout.ts\n--- a/src/timeout.ts\n+++ b/src/timeout.ts\n@@ -3,3 +3,3 @@\n-  return setTimeout(fn, ms * 100);\n+  return setTimeout(fn, ms);\n';

/** Tasks for the browser preview: the sample tasks, and new ones that "run" for a few seconds. Nothing runs. */
function demoTasks(touch: <T>(value: T) => Promise<T>): Pick<GatewayApi, 'workspaces' | 'addWorkspace' | 'removeWorkspace' | 'taskBudget' | 'setTaskBudget' | 'setWorkspaceBudget' | 'setWorkspacePrivacy' | 'pickFolder' | 'exportPatch'
  | 'detectChecks' | 'workspaceCheck' | 'tasks' | 'task' | 'createTask' | 'cancelTask' | 'decideTask' | 'retryTask' | 'taskDiff'
  | 'finishTask'> {
  let dailyLimit: number | null = 2;
  const workspaces: WorkspaceView[] = SAMPLE_WORKSPACES.map((w, i) => ({...w, tasks: 0, dailyBudgetUsd: null, privacyClass: i === 1 ? 'sensitive' as const : 'standard' as const, checks: i === 0
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
      outcome: a.outcome, detail: a.outcome === 'running' || a.outcome === 'waiting' ? null : a.outcome === 'skipped' ? 'skipped' : 'exit 0',
      escalationReason: a.outcome === 'failed' ? '`pnpm test` exited 1'
        : a.outcome === 'skipped' ? `Skipped: ${a.model} timed out on its last two runs in the past 30 minutes without answering, so Pygg did not wait for it again` : null,
      checks: a.outcome === 'passed' || a.outcome === 'failed' ? checks.map((c, j) => ({command: c.command,
        passed: a.outcome === 'passed' || j < checks.length - 1, detail: a.outcome === 'passed' || j < checks.length - 1 ? 'exited 0' : 'exited 1',
        outputTail: a.outcome === 'failed' && j === checks.length - 1 ? 'FAIL src/timeout.test.ts\n  expected 50 ms, got 5000 ms' : '',
        durationMs: 2400 + j * 900})) : [],
      filesChanged: a.outcome === 'waiting' || a.outcome === 'skipped' ? null : 2, insertions: 6, deletions: 3,
      summary: a.outcome === 'passed' ? 'Fixed the unit conversion in timeout handling and added a regression test.' : null,
      logTail: null, inputTokens: a.outcome === 'waiting' || a.outcome === 'skipped' ? null : 41_200,
      outputTokens: a.outcome === 'waiting' || a.outcome === 'skipped' ? null : 1_830,
      startedAt: s.startedAt, finishedAt: a.outcome === 'running' || a.outcome === 'waiting' ? null : s.updatedAt,
      open: a.outcome === 'passed' && (s.state === 'checks_passed'),
      agentEstimateUsd: a.outcome === 'passed' || a.outcome === 'failed' ? (a.billing === 'paid' ? 0.04 : 0) : null,
      costSource: a.outcome === 'passed' || a.outcome === 'failed' ? (a.billing === 'paid' ? 'pygg' : 'agent') : null,
      unpricedRequests: 0,
      capUsd: a.billing === 'paid' && a.outcome !== 'waiting' ? 0.5 : null,
      handoffSent: i > 0 && s.attempts[i - 1].outcome === 'failed' ? 'A previous attempt at this task, started from the same code, did not pass '
        + 'the workspace\'s checks. You start again from the original code.\nThe check `pnpm test` exited 1 on that attempt\'s changes. '
        + 'The end of its output:\nFAIL src/timeout.test.ts\n  expected 50 ms, got 5000 ms' : null,
      changedPaths: a.outcome === 'failed' ? ['src/timeout.ts'] : [],
    }));
    const waiting = attempts.find(a => a.outcome === 'waiting');
    return {...s, prompt: s.title, strategy: 'free_first', reason: s.state === 'interrupted'
      ? 'The app closed while this task ran. Retry runs the step again from the same code.'
      : s.state === 'waiting_approval' ? `Step 2 (${s.attempts[1]?.model ?? 'Claude Code · default model'}) is marked paid, so it waits for your approval.` : null,
      result: null, resultBranch: null, baseRevision: 'a91e7e7', workspaceName: ws.name, workspacePath: ws.path,
      policy: {steps: s.attempts.map((a, i) => ({agent: attempts[i].agent, model: null, tier: a.tier, billing: a.billing})), checks,
        allowShell: false, timeoutSecs: 900},
      attempts: s.attempts, attemptDetails: attempts,
      approval: s.state === 'waiting_approval' && waiting ? {step: waiting.step, paid: waiting.billing === 'paid', label: waiting.label,
        estimate: {runs: 4, lowMicroUsd: 20_000, highMicroUsd: 60_000}, suggestedCapUsd: 0.1, enforcedByAgent: waiting.agent === 'claude-code',
        remainingUsd: 1.42} : null};
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
        tasks: 0, dailyBudgetUsd: null, privacyClass: 'standard', checks: [{command: 'npm ci', source: 'package.json', approved: false}, {command: 'npm run test', source: 'package.json', approved: false}]};
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
    decideTask: async (id, approve, capUsd) => { await sleep(150);
      if (approve && !(capUsd && capUsd > 0)) throw 'Set a spending cap for this paid step';
      set(id, approve ? {state: 'running', phase: 'generating', reason: null, approval: null, attempts: get(id).attempts.map(a => a.outcome === 'waiting' ? {...a, outcome: 'running'} : a)}
        : {state: 'blocked', reason: 'You declined the step it was waiting on', attempts: get(id).attempts.map(a => a.outcome === 'waiting' ? {...a, outcome: 'canceled'} : a)}); },
    retryTask: async id => { if (get(id).state !== 'interrupted') throw 'Only an interrupted task can be retried';
      set(id, {state: 'running', phase: 'preparing', reason: null}); },
    taskDiff: () => touch(DEMO_DIFF),
    taskBudget: () => touch({recordedUsd: 0.48, reservedUsd: 0.1, limitUsd: dailyLimit, unknownCalls: 1}),
    setTaskBudget: async dailyUsd => { dailyLimit = dailyUsd; return touch({recordedUsd: 0.48, reservedUsd: 0.1, limitUsd: dailyLimit, unknownCalls: 1}); },
    setWorkspacePrivacy: async (id, privacyClass) => { const w = findWorkspace(id); w.privacyClass = privacyClass; return touch(w); },
    pickFolder: async () => null,
    exportPatch: async () => { await sleep(200); return 'C:\\Users\\you\\Downloads\\fix-timeout-handling.patch'; },
    setWorkspaceBudget: async (id, dailyUsd) => { const w = findWorkspace(id); w.dailyBudgetUsd = dailyUsd; return touch(w); },
    finishTask: async (id, keep, title) => { await sleep(250); const d = get(id);
      const branch = keep ? `str/${(title || d.title).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 48)}-${id.replace(/\W/g, '').slice(0, 8)}` : null;
      set(id, {result: keep ? 'kept' : 'discarded', resultBranch: branch, state: d.state === 'needs_review' ? (keep ? 'manually_accepted' : 'canceled') : d.state});
      return branch; },
  };
}
/** The integrations hub on demo data: Claude Code set up, Codex not yet, OpenCode with an entry made by hand, and the
 * two "models through Pygg" rows not set up. */
function demoIntegrations(touch: <T>(value: T) => Promise<T>) {
  const exe = 'C:\\Users\\you\\AppData\\Local\\Pygg\\Pygg.exe';
  const home = 'C:\\Users\\you';
  const list: Integration[] = [
    {id: 'claude-code', name: 'Claude Code', installed: true, state: 'set_up', configPath: `${home}\\.claude.json`,
      detail: 'pygg: · Scope: User config (available in all your projects) · Status: ✓ Connected', clientName: 'Claude Code (MCP)',
      setUpAt: Date.now() - 86_400_000, backup: `${home}\\AppData\\Roaming\\SmartTaskRouter\\integration-backups\\claude-code-1-.claude.json`},
    {id: 'codex', name: 'Codex CLI', installed: true, state: 'not_set_up', configPath: `${home}\\.codex\\config.toml`,
      detail: null, clientName: null, setUpAt: null, backup: null},
    {id: 'opencode', name: 'OpenCode', installed: true, state: 'set_up_elsewhere', configPath: `${home}\\.config\\opencode\\opencode.json`,
      detail: null, clientName: null, setUpAt: null, backup: null},
    {id: 'opencode-gateway', name: 'OpenCode: models through Pygg', kind: 'gateway', installed: true, state: 'not_set_up',
      configPath: `${home}\\.config\\opencode\\opencode.json`, detail: null, clientName: null, setUpAt: null, backup: null},
    {id: 'codex-gateway', name: 'Codex: models through Pygg', kind: 'gateway', installed: true, state: 'not_set_up',
      configPath: `${home}\\.codex\\config.toml`, detail: null, clientName: null, setUpAt: null, backup: null},
  ];
  const modelsClient = (id: string, name: string) =>
    id === 'opencode-gateway' ? 'OpenCode (models)' : id === 'codex-gateway' ? 'Codex (models)' : `${name} (MCP)`;
  const find = (id: string) => { const i = list.find(x => x.id === id); if (!i) throw `Pygg cannot set up ${id}`; return i; };
  return {
    integrations: () => touch(list),
    integrationPlan: (id: string) => {
      const i = find(id);
      const change = id === 'opencode-gateway'
        ? JSON.stringify({provider: {pygg: {npm: '@ai-sdk/anthropic', name: 'Pygg', options: {baseURL: 'http://127.0.0.1:8741/v1',
            apiKey: '<new-token>', headers: {'x-smarttaskrouter-session': '{env:PYGG_ATTEMPT_ID}'}},
            models: {auto: {name: 'auto', tool_call: true}, 'auto/cheap': {name: 'auto/cheap', tool_call: true},
              'anthropic/claude-haiku-5-5': {name: 'anthropic/claude-haiku-5-5', tool_call: true, limit: {context: 200000, output: 64000}}}}}}, null, 2)
        : id === 'codex-gateway'
        ? `# Pygg's gateway, added by Pygg (Agents > From Your IDE). Use it with: codex --profile pygg
[model_providers.pygg]
name = "Pygg"
base_url = "http://127.0.0.1:8741/v1"
wire_api = "responses"
experimental_bearer_token = "<new-token>"
env_http_headers = { x-smarttaskrouter-session = "PYGG_ATTEMPT_ID" }

# A new file next to it: pygg.config.toml
# Codex's models through Pygg's gateway, added by Pygg (Agents > From Your IDE): codex --profile pygg
model_provider = "pygg"
model = "auto"
`
        : id === 'opencode'
        ? JSON.stringify({mcp: {pygg: {type: 'local', command: [exe, 'mcp'], environment: {PYGG_TOKEN: '<new-token>'}, enabled: true}}}, null, 2)
        : id === 'codex' ? `codex mcp add pygg --env PYGG_TOKEN=<new-token> -- ${exe} mcp`
        : `claude mcp add --scope user pygg -e PYGG_TOKEN=<new-token> -- ${exe} mcp`;
      return touch<IntegrationPlan>({id, name: i.name, kind: i.kind ?? 'mcp', configPath: i.configPath, configExists: true, change,
        method: id.startsWith('opencode') || id === 'codex-gateway' ? 'file' : 'command', clientName: modelsClient(id, i.name),
        backupDir: `${home}\\AppData\\Roaming\\SmartTaskRouter\\integration-backups`});
    },
    applyIntegration: async (id: string) => {
      const i = find(id);
      if (i.state === 'set_up') throw `Pygg is already set up in ${i.name}`;
      if (i.state === 'set_up_elsewhere') throw `${i.name} already has an MCP server named pygg, set up outside Pygg. Remove it here first, or keep using it.`;
      Object.assign(i, {state: 'set_up', clientName: modelsClient(id, i.name), setUpAt: Date.now(),
        backup: `${home}\\AppData\\Roaming\\SmartTaskRouter\\integration-backups\\${id}-${Date.now()}`});
      await sleep(600);
      return clone(i);
    },
    removeIntegration: async (id: string) => {
      const i = find(id);
      Object.assign(i, {state: 'not_set_up', clientName: null, setUpAt: null, backup: null, detail: null});
      await sleep(300);
      return clone(i);
    },
  };
}
