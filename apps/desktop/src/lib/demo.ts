/**
 * Browser-preview stand-in for the core (pnpm dev), so the gateway screens can be reviewed without the
 * desktop app. Everything lives in memory, nothing is sent anywhere, and the interface shows a "Demo
 * data" banner whenever this is in use (ADR-014). It follows the core's rules (slugs, validation, honest
 * costs) closely enough for design review, not as a test of them.
 */
import type { AgentInfo, Integration, Launcher, ClientDefaults, ConnectionCheck, HealthStrip, IntegrationPlan, AutoExcluded, AutoPlan, AutoRanked, RoutingSummary, TokenMix, TokenMixGroup, TokenParts, PrefixSession, Billing, Capabilities, Client, Combo, Connection, GatewayApi, GatewayStatus, SavingsPart, SavingsPeriod,
  Health, Model, Preset, Target, Tier, UsageEvent, UsageBucket, UsageGroup, UsageSummary, Account, Wire,
  LocalUsage, SubscriptionUsage, Retention } from './api';
import { TIERS, WEB_WARNING_VERSION, type AutoAlias, type AutoRole, type ConnectionKey, type KeyStrategy } from './api';
import { SAMPLE_WORKSPACES, sampleTasks } from './sample';
import { demoSkills } from './demoSkills';
import type { AttemptView, NewTask, TaskDetail, TaskSummary, WorkspaceView, Brainstorm, BrainCard, BrainQuestion, BrainRole, RoleLine, BoardCard, BoardLane, BrainPick, BrainSettings, FactsStatus } from './tasks';

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
    ['meta', 'Meta (Muse API)', 'https://api.meta.ai/v1', 'paid', 'openai'],
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
  ...([
    // Same list as gateway/presets.rs `web(…)` and websession.rs `RECIPES`.
    ['muse-web', 'Muse (muse.ai)', 'https://muse.ai'],
    ['chatgpt-web', 'ChatGPT (chatgpt.com)', 'https://chatgpt.com'],
    ['perplexity-web', 'Perplexity (perplexity.ai)', 'https://www.perplexity.ai'],
    ['deepseek-web', 'DeepSeek (chat.deepseek.com)', 'https://chat.deepseek.com'],
    ['grok-web', 'Grok (grok.com)', 'https://grok.com'],
    ['gemini-web', 'Gemini (gemini.google.com)', 'https://gemini.google.com'],
  ] as const).map(([id, name, endpoint]): BasePreset => ({id, name, endpoint, needsKey: false, defaultBilling: 'free', wire: 'openai'})),
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
  'muse-web': ['web', 'The Muse models also have an API: Meta (Muse API)'],
  'chatgpt-web': ['web', 'OpenAI\'s models also have an API: OpenAI'],
  'perplexity-web': ['web', 'Answers with web search; its Sonar API is paid'],
  'deepseek-web': ['web', 'DeepSeek\'s models also have an API: DeepSeek'],
  'grok-web': ['web', 'Behind Cloudflare checks; xAI\'s models also have an API: xAI (Grok)'],
  'gemini-web': ['web', 'Google sign-in only; Google\'s models also have an API: Google Gemini'],
};
const PRESETS: Preset[] = BASE_PRESETS.map(p => {
  const offer = OFFERS[p.id];
  const freeModels = ['openrouter', 'zai', 'zai-anthropic'].includes(p.id);
  return {...p, offer: offer?.[0] ?? (p.endpoint === null ? 'custom' : p.defaultBilling === 'local' ? 'local' : 'paid'),
    offerNote: offer?.[1] ?? null, freeModels, defaultBilling: freeModels ? 'free' : p.defaultBilling, web: offer?.[0] === 'web',
    webActs: p.id === 'muse-web'};
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

/** As the core's `plain_http_allowed`: https anywhere, http only to this computer or the local network. */
function plainHttpAllowed(endpoint: string): boolean {
  let url: URL;
  try { url = new URL(endpoint); } catch { return false; }
  if (url.protocol !== 'http:') return true;
  const host = url.hostname.toLowerCase().replace(/\.$/, '');
  if (host.startsWith('[')) return host === '[::1]' || /^\[f[c-d]/.test(host) || /^\[fe[89ab]/.test(host);
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host)) return /^(127\.|10\.|192\.168\.|169\.254\.|172\.(1[6-9]|2\d|3[01])\.)/.test(host);
  return host === 'localhost' || !host.includes('.') || /\.(localhost|local|lan|internal|home\.arpa|test)$/.test(host);
}

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
  // `?gateway=stopped` shows the screens that say so (the board's header).
  if (new URLSearchParams(location.search).get('gateway') === 'stopped') status = {...status, state: 'stopped', baseUrl: null};
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
      tokenViewable: true, sessionBudgetMicroUsd: 5_000_000, dayBudgetMicroUsd: 8_000_000, dayCountedMicroUsd: 2_140_000},
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
  let clientDefaults: ClientDefaults = {autoMinTier: null, monthlyBudgetMicroUsd: null, maxConcurrency: null, unknownCostPolicy: null,
    paidRetries: null, requireNoTraining: null};
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
      // A coding agent re-sends its conversation each turn: most of a Claude Code prompt is read from Gemini's implicit
      // cache (billed at the cached rate), as the CCHT benchmark measured (87–96%); a script's one-off prompt is not.
      const read = client === 'Claude Code' ? Math.round(input * 0.85) : 0;
      events.push({...base, id: `e${i}`, provider: 'gemini', model: 'gemini-3.1-flash-lite', inputTokens: input, outputTokens: output,
        cacheTokens: read, costMicroUsd: Math.round((input - read) * 0.1 + read * 0.025 + output * 0.4), costStatus: 'estimated',
        costSource: 'price: user', outcome: 'ok', httpStatus: 200});
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

  // What schema v30 records with each attempt: the request (a fallback's two attempts share one), the billing it was sent
  // under, how its route was chosen and the cache hints. Claude Code's launches are routing sessions of about eight
  // requests, so Usage → Routing has changes of model to show.
  for (const e of events) {
    const pair = /^e(\d+)([ab])$/.exec(e.id);
    e.requestId = pair ? `r${pair[1]}` : `r-${e.id}`;
    e.attempt = pair?.[2] === 'b' ? 2 : 1;
    e.billing = e.provider === 'ollama' ? 'local' : e.provider === 'qwen-plan' ? 'subscription' : e.costStatus === 'unknown' ? 'unknown' : 'paid';
    e.routeKind = e.requestedModel === 'coding' || e.requestedModel === 'local-first' ? 'combo' : 'named';
    e.cacheHint = e.provider === 'anthropic' ? 'preserved' : 'none';
    if (e.client === 'Claude Code' && /^e\d+/.test(e.id)) e.sessionKey = `launch-${Math.floor(Number(/^e(\d+)/.exec(e.id)![1]) / 8)}`;
  }
  // Why each answer missed the cache, as store::cache_miss reads the ledger: oldest first, per session.
  const seen = new Map<string, {target: string; at: number}>();
  for (const e of [...events].sort((a, b) => a.createdAt - b.createdAt)) {
    if (e.inputTokens == null || !['ok', 'client_disconnected', 'stream_error'].includes(e.outcome)) continue;
    const target = `${e.provider}/${e.model}`, last = e.sessionKey ? seen.get(e.sessionKey) : undefined;
    e.cacheMiss = e.cacheTokens == null ? 'not_reported' : e.cacheTokens > 0 ? null : e.inputTokens < 1024 ? 'below_minimum'
      : !e.sessionKey ? 'no_session' : !last ? 'first_request' : last.target !== target ? 'model_switched'
      : e.createdAt - last.at >= 300_000 ? 'cache_expired' : 'prefix_changed';
    if (e.sessionKey) seen.set(e.sessionKey, {target, at: e.createdAt});
  }

  // `?empty` starts from nothing, to review first-run states.
  if (new URLSearchParams(location.search).has('empty')) {
    connections.length = 0;
    models.length = 0;
    combos.length = 0;
    clients.length = 0;
    events.length = 0;
  }

  // Key rings (ADR-038), by connection: filled for `?stress` below; their functions are with the connections.
  const rings = new Map<string, ConnectionKey[]>();
  const keyStrategies = new Map<string, KeyStrategy>();
  // `?stress` fills the preview with data shaped like a real account (UI review 2026-11-14, after a models table broke on
  // the owner's OpenRouter free models): long model ids, dozens of them, long labels and names, failed tests with long
  // errors. Reviews look at every screen with it as well as with the default data.
  const stress = new URLSearchParams(location.search).has('stress');
  if (stress) {
    const vendors = ['baidu/ernie-5.5-flash-preview', 'cohere/north-mini-code', 'dots-studio/dots-3-note-preview', 'google/gemma-4-26b-a4b-it',
      'meta-llama/llama-4.1-maverick-instruct-long-context', 'mistralai/devstral-medium-2611-experimental', 'moonshotai/kimi-k3-thinking-preview',
      'nousresearch/hermes-5-405b-reasoning', 'nvidia/nemotron-ultra-3-253b-v2-instruct', 'qwen/qwen3.7-coder-480b-a35b-instruct-turbo',
      'thudm/glm-5.1-air-agentic-coding-preview', 'z-ai/glm-4.9-flash-0305-reasoning-distill'];
    const ids: string[] = [];
    for (let round = 0; round < 3; round++) for (const v of vendors) ids.push(round === 0 ? `${v}:free` : `${v}-v${round + 1}:free`);
    addModels('openrouter', ids);
    for (const [i, id] of ids.entries()) {
      const m = models.find(x => x.modelId === id)!;
      m.billing = 'free';
      m.price = {inputMicroPerMtok: 0, outputMicroPerMtok: 0, cachedInputMicroPerMtok: null, source: 'list', observedAt: now};
      m.capabilities = {...UNKNOWN, contextTokens: [1_024_000, 256_000, 512_000, 262_144][i % 4], supportsTools: true, supportsVision: i % 3 === 0,
        source: 'provider: openrouter /models', observedAt: now};
    }
    const or = connections.find(c => c.id === 'openrouter')!;
    or.label = 'OpenRouter — personal account used by every coding agent on this computer';
    clients.push({...clients[0], id: 'k-stress', name: 'Visual Studio Code with the Continue extension (work laptop, second profile)',
      tokenPrefix: 'str-9f1d0c33', spend: 0});
    combos.push({id: 'combo-stress', name: 'free-models-first-then-the-cheapest-paid-ones-for-long-agent-sessions',
      members: ids.slice(0, 6).map(modelId => ({providerId: 'openrouter', modelId}))});
    for (const [i, e] of events.slice(0, 40).entries()) {
      e.model = ids[i % ids.length];
      e.provider = 'openrouter';
      if (i % 5 === 0) {
        e.outcome = 'error';
        e.httpStatus = 429;
        e.error = 'Rate limit exceeded: free-models-per-day. Add 10 credits to unlock 1000 free model requests per day. This message comes straight from the provider and is long on purpose.';
      }
    }
    // OpenRouter with three keys of separate accounts (ADR-038): one rate-limited with a long provider message, one rejected.
    const later = now + 42 * 60_000;
    rings.set('openrouter', [
      {id: 'or-k0', providerId: 'openrouter', label: 'First key', rank: 0, enabled: true, separateAccountAt: null, createdAt: now - 86_400_000,
        primary: true, health: [], requestsToday: 132, lastUsedAt: now - 40_000, room: 0.62},
      {id: 'or-k1', providerId: 'openrouter', label: 'Work account — the team’s shared OpenRouter organization, billed monthly by finance',
        rank: 1, enabled: true, separateAccountAt: now - 3_600_000, createdAt: now - 3_600_000, primary: false, requestsToday: 87,
        lastUsedAt: now - 300_000, room: 0, health: [{keyId: 'or-k1', modelId: ids[0], state: 'rate_limited', httpStatus: 429, untilMs: later, updatedAt: now,
          reason: 'Router answered HTTP 429: Rate limit exceeded: free-models-per-day. Add 10 credits to unlock 1000 free model requests per day.'}]},
      {id: 'or-k2', providerId: 'openrouter', label: 'Old trial account', rank: 2, enabled: true, separateAccountAt: now - 7_200_000,
        createdAt: now - 7_200_000, primary: false, requestsToday: 3, lastUsedAt: now - 7_000_000, room: null,
        health: [{keyId: 'or-k2', modelId: '', state: 'invalid_credentials', httpStatus: 401, untilMs: null, updatedAt: now,
          reason: 'Router answered HTTP 401: User not found.'}]},
    ]);
    keyStrategies.set('openrouter', 'quota_aware');
    // A DeepSeek web session backed off after the site asked to slow down: the web guard's longest notes (ADR-035).
    connections.push({id: 'deepseek-web-stress', kind: 'deepseek-web', label: 'DeepSeek — personal account, signed in on this computer',
      endpoint: 'https://chat.deepseek.com', enabled: true, credentialRef: null, defaultBilling: 'free', dataUseTerms: 'unknown',
      termsSource: null, termsCheckedAt: null, createdAt: now, key: false, wire: 'openai', separateQuota: false, quotaConfirmedAt: null,
      web: true, webAckCurrent: true, webActs: false, webAckAt: now, webState: 'signed_in', webStateAt: now, webStateReason: 'Signed in.',
      webAccount: 'a.very.long.account.name.for.testing@example-company-mail.com'});
    addModels('deepseek-web-stress', ['deepseek']);
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
  // Key rings (ADR-038): keys of separate accounts on one connection, used in turn. The preview never holds a secret.
  const NO_SIGN_INS = 'Only API-key connections have several keys. Sign-ins (web sessions, subscriptions) are never rotated.';
  const keysRefused = (c: Stored) => c.web || PRESETS.find(p => p.id === c.kind)?.offer === 'local' ? NO_SIGN_INS : null;
  const findKey = (keyId: string) => {
    for (const [id, keys] of rings) { const k = keys.find(x => x.id === keyId); if (k) return {id, keys, k}; }
    throw 'No such key';
  };
  function view(c: Stored): Connection {
    const preset = PRESETS.find(p => p.id === c.kind);
    const {key, ...rest} = c;
    return {...rest, keys: rings.get(c.id) ?? [], keyStrategy: keyStrategies.get(c.id) ?? 'in_order', keysRefused: keysRefused(c),
      quotaGroup: quotaGroup(c), presetName: preset?.name ?? c.kind, hasKey: key, needsKey: preset?.needsKey ?? false,
      freeModels: preset?.freeModels ?? false,
      readsAccount: c.kind === 'openrouter' || c.kind === 'deepseek', account: accounts.get(c.id) ?? null,
      models: models.filter(m => m.providerId === c.id).length,
      health: health.filter(h => h.providerId === c.id && (h.untilMs === null || h.untilMs > Date.now())),
      sharesQuotaWith: connections.filter(o => o.id !== c.id && quotaGroup(o) === quotaGroup(c)).map(o => o.label),
      // Illustrative: what Anthropic's API sends with an answer.
      ...demoHealth(c),
      // Illustrative: the web guard after a long prompt and a timeout (ADR-035, migration 0028).
      ...(c.web ? {webDailyLimit: c.kind === 'deepseek-web' ? 30 : 50, webGuard: {hourUnits: 7, hourRequests: 3, hourUnitsCap: 20, hourRequestsCap: 15, todayUnits: 12, todayUnitsCap: 100,
        maxPromptChars: c.kind === 'deepseek-web' ? 4_000 : 12_000, burstRequests: 2, burstRequestsCap: c.kind === 'deepseek-web' ? 3 : 5,
        burstWindowMin: 10, dailyLimitMax: c.kind === 'deepseek-web' ? 30 : 100, restBaseSecs: c.kind === 'deepseek-web' ? 30 : 10,
        siteNote: c.kind === 'deepseek-web' ? "DeepSeek suspended an account for Pygg's pace on 2026-11-13, so its limits are lower than other sites'." : null,
        restingUntil: Date.now() + 70_000, backoffUntil: c.kind === 'grok-web' || c.id === 'deepseek-web-stress' ? Date.now() + 35 * 60_000 : null,
        backoffReason: c.kind === 'grok-web' ? 'No reply appeared in time.' : c.id === 'deepseek-web-stress'
          ? 'The site asked to slow down (a "too many requests" or "server is busy" notice), so Pygg rests this account for at least half an hour.' : null}} : {}),
      limits: c.kind === 'qwen' ? planLimits() : c.kind === 'anthropic' ? [{modelId: 'claude-sonnet-4-5', httpStatus: 200, observedAt: Date.now() - 6 * 60_000, headers: {
        'anthropic-ratelimit-requests-limit': '50', 'anthropic-ratelimit-requests-remaining': '49',
        'anthropic-ratelimit-tokens-remaining': '39000', 'anthropic-ratelimit-tokens-reset': '2026-11-12T02:31:00Z'}}] : []};
  }
  /** Each connection's checks, newest last: one 50 minutes ago for every enabled API connection, Ollama's failed (its
   *  runtime is off), as the 30-minute loop would leave them. Check Now and Check All add to them. */
  const checks = new Map<string, ConnectionCheck[]>();
  function demoChecks(c: Stored): ConnectionCheck[] {
    if (!checks.has(c.id)) {
      const at = Date.now() - 50 * 60_000;
      checks.set(c.id, !c.enabled || c.web ? [] : c.kind === 'ollama'
        ? [{at, ok: false, kind: 'models', detail: 'Could not reach http://localhost:11434/v1: connection refused'}]
        : [{at, ok: true, kind: 'models', detail: 'Reachable · models listed'}]);
    }
    return checks.get(c.id)!;
  }
  function demoCheck(c: Stored) {
    const ok = c.kind !== 'ollama';
    demoChecks(c).push(c.web ? {at: Date.now(), ok: true, kind: 'sign_in', detail: 'Signed in.'}
      : {at: Date.now(), ok, kind: 'models', detail: ok ? `Reachable · ${models.filter(m => m.providerId === c.id).length} models listed`
        : 'Could not reach http://localhost:11434/v1: connection refused'});
    return ok;
  }
  /** The health strip from the demo's own requests and checks, as store::health_strip counts them. */
  function demoHealth(c: Stored): {healthStrip: HealthStrip; lastCheck: ConnectionCheck | null} {
    const hour = 3_600_000;
    const now = Date.now();
    const slot = (at: number) => at <= now && at > now - 24 * hour ? 23 - Math.floor((now - at) / hour) : -1;
    const buckets = Array.from({length: 24}, (_, i) => ({start: now - (24 - i) * hour, ok: 0, failed: 0}));
    for (const e of events.filter(e => e.provider === c.id)) {
      const b = buckets[slot(e.createdAt)];
      if (!b) continue;
      const notSent = (e.error ?? '').includes('Not sent');
      if (e.outcome === 'ok') b.ok += 1;
      else if (['error', 'fallback', 'stream_error'].includes(e.outcome) && ![400, 413, 422].includes(e.httpStatus ?? 0) && !notSent) b.failed += 1;
    }
    // An hour with requests is judged by them alone; an hour without, by its checks.
    const fromChecks = Array.from({length: 24}, () => ({ok: 0, failed: 0}));
    for (const k of demoChecks(c)) {
      const b = fromChecks[slot(k.at)];
      if (b) { if (k.ok) b.ok += 1; else b.failed += 1; }
    }
    buckets.forEach((b, i) => { if (b.ok + b.failed === 0) Object.assign(b, fromChecks[i]); });
    let good = 0, all = 0;
    buckets.forEach((b, i) => { if (b.ok + b.failed === 0) return; const w = 0.25 + 0.75 * i / 23; good += w * b.ok / (b.ok + b.failed); all += w; });
    const list = demoChecks(c);
    return {healthStrip: {buckets, score: all > 0 ? Math.round(100 * good / all) : null}, lastCheck: list[list.length - 1] ?? null};
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
  // The Brain Storming screen (BRAIN.md 4.7b) and the feed's state, as the app keeps them.
  const brainSettings: BrainSettings = {wipRunning: 1, wipChecking: 1, runner: 'engine', rememberPlanner: true, changeScope: 'ask'};
  const brainPicks: BrainPick[] = new URLSearchParams(location.search).has('empty') ? [] : [{scope: 'global', role: 'plan', model: 'gemini/gemini-3.1-pro-preview'}];
  const factsStatus: FactsStatus = {enabled: true, fetchedAt: Date.now() - 2 * 86_400_000, built: '2026-11-18T06:12:00Z', facts: 187, stale: false, lastError: null, sources: [
    {id: 'terminal-bench', name: 'Terminal-Bench 4.x', url: 'https://www.tbench.ai/leaderboard', licence: 'Apache-2.0', attribution: 'Terminal-Bench', measures: 'tasks solved in a terminal by an agent with the model', roles: ['code', 'test'], status: 'ok', observed: '2026-11-18', rows: 71, termsRead: '2026-11-10', error: null},
    {id: 'swe-bench-verified', name: 'SWE-bench Verified', url: 'https://www.swebench.com', licence: 'CC BY-NC 4.0', attribution: 'SWE-bench', measures: 'GitHub issues resolved', roles: ['code'], status: 'ok', observed: '2026-11-17', rows: 110, termsRead: '2026-11-10', error: null},
    {id: 'aider-polyglot', name: 'Aider polyglot', url: 'https://aider.chat/docs/leaderboards/', licence: 'Apache-2.0', attribution: 'Aider', measures: 'exercises passed', roles: ['code'], status: 'ok', observed: '2025-09-01', rows: 42, termsRead: '2026-11-10', error: null},
    {id: 'lmarena', name: 'LMArena (hard prompts)', url: 'https://lmarena.ai', licence: 'CC BY 4.0', attribution: 'LMArena', measures: 'pairwise human preference', roles: ['plan', 'review', 'image'], status: 'carried', observed: '2026-11-11', rows: 405, termsRead: '2026-11-10', error: 'HTTP 503 at the last build'}]};
  function simulate(clientId: string | null, alias: AutoAlias, needs: Parameters<GatewayApi['autoSimulate']>[2]): AutoPlan {
    const client = clients.find(c => c.id === clientId);
    const cheap = alias === 'auto/cheap';
    // A role alias (auto/code, …) ranks by the model facts feed in the app; the demo keeps no feed, so every model
    // ranks by its tier, the higher first, and says so, as auto.rs does with no feed.
    const role: AutoRole | null = alias === 'auto' || cheap ? null : alias.slice('auto/'.length) as AutoRole;
    const minTier: Tier | null = cheap ? null : client?.autoMinTier ?? 'T2';
    const rank = (t: Tier) => TIERS.indexOf(t);
    const billingRank = (b: Billing) => ({local: 0, free: 1, subscription: 2} as Record<string, number>)[b] ?? 3;
    const output = needs.maxTokens ?? 4096;
    const ranked: AutoRanked[] = [], excluded: AutoExcluded[] = [];
    for (const m of models) {
      const conn = connections.find(c => c.id === m.providerId);
      if (!conn?.enabled || !m.enabled) continue;
      const caps = m.capabilities, billing = m.billing === 'unknown' ? conn.defaultBilling : m.billing;
      // Each exclusion says what would bring the model in, as auto.rs's `kind` does.
      const no = (kind: AutoExcluded['kind'], reason: string) => excluded.push({providerId: m.providerId, modelId: m.modelId, reason, kind});
      if (conn.web) { no('web', 'A web session is used only by name or in a combo you build, never chosen by auto.'); continue; }
      if (client?.allowedConnections && !client.allowedConnections.includes(conn.id)) { no('policy', `this client may not use ${conn.label}`); continue; }
      // Both API formats reach both kinds of target (auto.rs, ROADMAP B5).
      const h = health.find(x => x.providerId === conn.id && (x.modelId === '' || x.modelId === m.modelId) && (x.untilMs ?? Infinity) > Date.now());
      if (h) { no('unavailable', `${h.state.replace('_', ' ')} (${h.reason})`); continue; }
      if (minTier) {
        if (!caps.tier) { no('facts', role ? `no fact for ${role} and no tier set` : 'no tier set'); continue; }
        if (rank(caps.tier) < rank(minTier)) { no('policy', `tier ${caps.tier} is below this client's minimum ${minTier}`); continue; }
      }
      if (needs.tools && caps.supportsTools !== true) { caps.supportsTools === false ? no('policy', 'does not take tools') : no('facts', 'tool support unknown'); continue; }
      if (needs.vision && caps.supportsVision !== true) { caps.supportsVision === false ? no('policy', 'does not take images') : no('facts', 'image support unknown'); continue; }
      if (conn.wire === 'anthropic' && needs.thinking) {
        if (caps.thinkingTypes === null) { no('facts', `unknown whether it takes thinking: ${needs.thinking} (refresh its model list, or set it)`); continue; }
        if (!caps.thinkingTypes.includes(needs.thinking)) { no('policy', `does not take thinking: ${needs.thinking}`); continue; }
      }
      if (conn.wire === 'anthropic' && needs.effort) {
        if (caps.effortLevels === null) { no('facts', `unknown whether it takes effort: ${needs.effort} (refresh its model list, or set it)`); continue; }
        if (!caps.effortLevels.includes(needs.effort)) { no('policy', `does not take effort: ${needs.effort}`); continue; }
      }
      if (!caps.contextTokens) { no('facts', 'context size unknown'); continue; }
      if (needs.inputTokens + output > caps.contextTokens) {
        no('policy', `about ${needs.inputTokens} prompt and ${output} output tokens do not fit its ${caps.contextTokens}-token context`); continue;
      }
      if (needs.maxTokens != null) {
        if (caps.maxOutputTokens == null) { no('facts', 'maximum output unknown, and the request sets one'); continue; }
        if (needs.maxTokens > caps.maxOutputTokens) { no('policy', `the request allows ${needs.maxTokens} output tokens; it gives at most ${caps.maxOutputTokens}`); continue; }
      }
      let estimate = 0, cost = `${billing} (as set)`;
      if (billing === 'subscription') cost = "the plan's quota, not money; quota left not tracked";
      if (billing !== 'free' && billing !== 'local' && billing !== 'subscription') {
        if (!m.price) { no('facts', 'price unknown'); continue; }
        const inPrice = Math.max(m.price.inputMicroPerMtok, m.price.cacheWriteMicroPerMtok ?? 0);
        estimate = Math.ceil(needs.inputTokens * inPrice / 1e6) + Math.ceil(output * m.price.outputMicroPerMtok / 1e6);
        cost = `about $${(estimate / 1e6).toFixed(4)} at ${m.price.source === 'user' ? 'your price' : "the provider's list price"}`;
      }
      const reason = cheap ? `${billing}; ${cost}`
        : role ? `no fact for ${role}; tier ${caps.tier}; ${billing}; ${cost}`
        : `tier ${caps.tier} (minimum ${minTier}); ${billing}; ${cost}`;
      ranked.push({providerId: m.providerId, modelId: m.modelId, tier: caps.tier, billing, estimateMicroUsd: estimate, reason});
    }
    ranked.sort((a, b) => cheap
      ? a.estimateMicroUsd - b.estimateMicroUsd || billingRank(a.billing) - billingRank(b.billing) || rank(b.tier ?? 'T0') - rank(a.tier ?? 'T0')
      : role ? rank(b.tier!) - rank(a.tier!) || billingRank(a.billing) - billingRank(b.billing) || a.estimateMicroUsd - b.estimateMicroUsd
      : rank(a.tier!) - rank(b.tier!) || billingRank(a.billing) - billingRank(b.billing) || a.estimateMicroUsd - b.estimateMicroUsd);
    // As auto::route: at most two from a connection while others are eligible, the free slots filled in rank order.
    const chosen: number[] = [], per = new Map<string, number>();
    ranked.forEach((r, i) => {
      if (chosen.length < 5 && (per.get(r.providerId) ?? 0) < 2) { chosen.push(i); per.set(r.providerId, (per.get(r.providerId) ?? 0) + 1); }
    });
    ranked.forEach((_, i) => { if (chosen.length < 5 && !chosen.includes(i)) chosen.push(i); });
    const route = chosen.sort((a, b) => a - b).map(i => ranked[i]);
    return {alias, strategy: cheap ? 'cheap' : role ? {role} : 'tier', minTier, ranked, route, excluded};
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
      // A web session (ADR-035): no key, off until signed in; the preview has no window, so it never signs in.
      if (preset.web) {
        if (connection.webAck !== WEB_WARNING_VERSION) throw 'Read and acknowledge the account-risk warning first';
        const label = connection.label.trim() || preset.name.split(' (')[0];
        let id = slug(label), n = 2;
        while (connections.some(c => c.id === id)) id = `${slug(label)}-${n++}`;
        connections.push({id, kind: preset.id, label, endpoint: preset.endpoint!, enabled: false, credentialRef: null,
          defaultBilling: 'free', dataUseTerms: 'unknown', termsSource: null, termsCheckedAt: null, createdAt: Date.now(),
          key: false, wire: 'openai', separateQuota: false, quotaConfirmedAt: null, web: true, webAckCurrent: true, webActs: preset.webActs,
          webAckAt: Date.now(), webState: 'signed_out', webStateAt: Date.now(),
          webStateReason: 'Not signed in yet. Open the sign-in window and sign in on the site.'});
        addModels(id, [preset.id.replace(/-web$/, '')]);
        models[models.length - 1].capabilities = {...UNKNOWN, supportsTools: false, supportsVision: false,
          source: 'web session: text only, no tools or images (ADR-035)'};
        return view(connections[connections.length - 1]);
      }
      const endpoint = (connection.endpoint?.trim() || preset.endpoint || '').replace(/\/+$/, '');
      if (!endpoint) throw 'Enter the endpoint URL';
      if (!/^https?:\/\//.test(endpoint)) throw 'The endpoint must start with https:// or http://';
      if (!plainHttpAllowed(endpoint)) throw 'Use https:// for a provider on the internet: over http:// its key and every prompt would travel unencrypted. http:// is for this computer and the local network.';
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
      const baseline = savingsBaseline;
      const priceAt = (e: UsageEvent) => (e.inputTokens ?? 0) * 2 + (e.outputTokens ?? 0) * 10;
      const comparable = (e: UsageEvent) => !!baseline && rowBilling(e) !== 'subscription' && e.costStatus !== 'unknown'
        && e.inputTokens !== null && e.outputTokens !== null;
      const failedRows = shown.filter(e => e.outcome !== 'ok' && e.outcome !== 'client_disconnected');
      const failed = failedRows.filter(e => e.costStatus !== 'unknown').reduce((n, e) => n + (e.costMicroUsd ?? 0), 0);
      const routeKind = (m: string | null) => !m || m === 'auto' ? 'auto' : m === 'auto/cheap' ? 'auto/cheap'
        : m.startsWith('auto/') ? 'auto' : m.includes('/') ? 'named' : 'combo';
      const slice = (keyOf: (e: UsageEvent) => string): SavingsPart[] => {
        const parts = new Map<string, SavingsPart>();
        for (const e of ok) {
          const key = keyOf(e);
          const p = parts.get(key) ?? {key, answered: 0, compared: 0, routedMicroUsd: 0, baselineMicroUsd: 0};
          p.answered += 1;
          if (comparable(e)) { p.compared += 1; p.routedMicroUsd += e.costMicroUsd ?? 0; p.baselineMicroUsd += priceAt(e); }
          parts.set(key, p);
        }
        return [...parts.values()].sort((a, b) => b.answered - a.answered);
      };
      const order = ['free', 'local', 'subscription', 'paid', 'unknown'];
      const count = (b: string) => ok.filter(e => (rowBilling(e) as string) === b).length;
      const size = days <= 1 ? 3_600_000 : 86_400_000;
      const offset = -new Date().getTimezoneOffset() * 60_000;
      const periods = new Map<number, SavingsPeriod>();
      const compared = ok.filter(comparable);
      for (const e of compared) {
        const start = Math.floor((e.createdAt + offset) / size) * size - offset;
        const p = periods.get(start) ?? {start, compared: 0, routedMicroUsd: 0, baselineMicroUsd: 0};
        p.compared += 1;
        p.routedMicroUsd += e.costMicroUsd ?? 0;
        p.baselineMicroUsd += priceAt(e);
        periods.set(start, p);
      }
      return touch({
        baseline, baselinePriced: baseline !== null,
        answered: {free: count('free'), local: count('local'), subscription: count('subscription'), paid: count('paid'), unknown: count('unknown')},
        compared: compared.length,
        routedMicroUsd: baseline ? compared.reduce((n, e) => n + (e.costMicroUsd ?? 0), 0) + failed : 0,
        baselineMicroUsd: compared.reduce((n, e) => n + priceAt(e), 0),
        failedAttemptsMicroUsd: baseline ? failed : 0,
        leftOutUnknownCost: baseline ? ok.filter(e => e.costStatus === 'unknown' && rowBilling(e) !== 'subscription').length : 0,
        leftOutNoTokens: 0,
        failedUnknownCost: failedRows.filter(e => e.costStatus === 'unknown').length,
        suggested: baseline ? null : 'anthropic/claude-sonnet-4-5',
        byBilling: slice(e => rowBilling(e) as string).sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key)),
        byClient: slice(e => e.client ?? ''),
        byRoute: slice(e => routeKind(e.requestedModel)),
        periods: [...periods.values()].sort((a, b) => a.start - b.start),
        fellBack: shown.filter(e => e.outcome === 'fallback').length,
      });
    },
    setSavingsBaseline: async baseline => { savingsBaseline = baseline; },
    clientDefaults: async () => touch({...clientDefaults}),
    setClientDefaults: async d => { await sleep(150); clientDefaults = {...d}; },
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
    setComboStrategy: async (id, strategy) => { const c = combos.find(x => x.id === id); if (c) c.strategy = strategy; await sleep(80); },
    testCombo: async name => {
      await sleep(900);
      const combo = combos.find(c => c.name === name);
      const first = combo?.members[0];
      return first ? {ok: true, text: 'ok', latencyMs: 910, inputTokens: 14, outputTokens: 2, route: 'native',
        answeredBy: `${first.providerId}/${first.modelId}`, attempts: 1} : {ok: false, text: 'No such combo', latencyMs: 1, inputTokens: null,
        outputTokens: null, route: '', answeredBy: '', attempts: 0};
    },
    testModel: async (_provider, modelId) => {
      await sleep(600);
      // Stress data: every other model fails the way a real free model does, with the provider's long message.
      if (stress && modelId.length % 2 === 0) return {ok: false, text: 'The model answered with HTTP 429 Too Many Requests: Rate limit exceeded: free-models-per-day. Add 10 credits to unlock 1000 free model requests per day.', latencyMs: 412, inputTokens: null, outputTokens: null, route: 'native'};
      return {ok: true, text: 'ok', latencyMs: 640, inputTokens: 14, outputTokens: 2, route: 'native'};
    },
    setConnectionPool: async (id, pool) => {
      const c = find(id);
      if (pool && !c.separateQuota) throw 'Confirm first that this connection has its own provider quota (Independent quota confirmed): a pool is for separate accounts, not several keys of one account';
      c.pool = pool;
      c.poolRank = pool ? connections.filter(x => x.pool === pool).length : null;
      return touch(view(c));
    },
    addConnectionKey: async (id, label, apiKey, separateAccount) => {
      const c = find(id);
      if (keysRefused(c)) throw NO_SIGN_INS;
      if (!c.key) throw 'This connection has no key yet: set its key first, then add others.';
      if (!separateAccount) throw 'Confirm that this key belongs to another account, with its own quota. Several keys of one account share its limits, so Pygg does not rotate them.';
      if (!apiKey.trim()) throw 'Enter the API key';
      const name = label.trim();
      if (!name || name.length > 60) throw 'Give the key a name of 1 to 60 characters, such as the account it belongs to.';
      const keys = rings.get(id) ?? [];
      const now = Date.now();
      if (!keys.length) keys.push({id: `${id}-k0`, providerId: id, label: 'First key', rank: 0, enabled: true, separateAccountAt: null,
        createdAt: c.createdAt, primary: true, health: [], requestsToday: 0, lastUsedAt: null, room: null});
      if (keys.some(k => k.label.toLowerCase() === name.toLowerCase())) throw `This connection already has a key named "${name}".`;
      keys.push({id: `${id}-k${now}`, providerId: id, label: name, rank: keys.length, enabled: true, separateAccountAt: now, createdAt: now,
        primary: false, health: [], requestsToday: 0, lastUsedAt: null, room: null});
      rings.set(id, keys);
      return touch(view(c));
    },
    removeConnectionKey: async keyId => {
      const {keys, k} = findKey(keyId);
      if (keys.length === 1) throw 'This is the connection’s only key. Replace it, or remove the connection.';
      keys.splice(keys.indexOf(k), 1);
      if (k.primary) keys[0].primary = true;
      if (!keys.some(x => x.enabled)) keys[0].enabled = true;
      keys.forEach((x, i) => { x.rank = i; });
      await touch(undefined);
    },
    moveConnectionKey: async (keyId, up) => {
      const {keys, k} = findKey(keyId);
      const at = keys.indexOf(k), to = up ? at - 1 : at + 1;
      if (to >= 0 && to < keys.length) [keys[at], keys[to]] = [keys[to], keys[at]];
      keys.forEach((x, i) => { x.rank = i; });
      await touch(undefined);
    },
    enableConnectionKey: async (keyId, enabled) => {
      const {keys, k} = findKey(keyId);
      if (!enabled && !keys.some(x => x.enabled && x !== k)) throw 'This is the only key on; turn another one on first, or turn the connection off.';
      k.enabled = enabled;
      await touch(undefined);
    },
    setKeyStrategy: async (id, strategy) => {
      keyStrategies.set(id, strategy);
      return touch(view(find(id)));
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
    // The preview has no window to open or drive.
    openWebSession: async () => { await sleep(150); throw 'The browser preview cannot open a sign-in window; run the desktop app.'; },
    setWebDailyLimit: async () => { await sleep(100); },
    checkConnection: async id => { await sleep(400); const c = connections.find(c => c.id === id)!; demoCheck(c); return view(c); },
    checkAllConnections: async () => { await sleep(800);
      const api = connections.filter(c => c.enabled && !c.web);
      const failed = api.filter(c => !demoCheck(c)).length;
      return {checked: api.length, failed, webSkipped: connections.filter(c => c.enabled && c.web).length}; },
    resumeWebSession: async () => { await sleep(100); },
    checkWebSession: async id => { await sleep(400); return touch(view(find(id))); },
    refreshModels: async id => {
      await sleep(700);
      const c = find(id);
      if (c.web) return {listed: 1, added: 0};
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
    brainstormCandidates: async (_id, role) => touch(simulate(null, `auto/${role ?? 'plan'}`, {surfaceAnthropic: false, tools: role != null && role !== 'plan', vision: false, inputTokens: role && role !== 'plan' ? 8000 : 3000, maxTokens: role && role !== 'plan' ? null : 6000, thinking: null, effort: null})),
    modelFactsStatus: async () => touch(factsStatus),
    modelFactsRefresh: async () => { await sleep(600); factsStatus.fetchedAt = Date.now(); factsStatus.stale = false; factsStatus.lastError = null; return touch(factsStatus); },
    modelFactsSetEnabled: async on => { factsStatus.enabled = on; return touch(factsStatus); },
    brainSettings: async () => touch(brainSettings),
    setBrainSettings: async s => { Object.assign(brainSettings, s); return touch(brainSettings); },
    brainPicks: async () => touch(brainPicks),
    setBrainPick: async (scope, role, model) => {
      const i = brainPicks.findIndex(p => p.scope === scope && p.role === role);
      if (i >= 0) brainPicks.splice(i, 1);
      if (model) brainPicks.push({scope, role, model});
      return touch(brainPicks);
    },
    modelFactsRole: async role => touch(models.filter(m => m.enabled).slice(0, 40).map((m, i): RoleLine => ({
      model: `${m.providerId}/${m.modelId}`, providerId: m.providerId, modelId: m.modelId, billing: m.billing, tier: m.capabilities.tier, tierSource: m.capabilities.tierSource ?? null,
      layer: i === 1 ? 'own' : i % 3 === 0 ? 'feed' : m.capabilities.tier ? 'tier' : 'none', own: i === 1 ? {attempts: 14, checksPassed: 11, kept: 9, ranks: true, knownMicroUsd: 640_000, unknownRequests: 0} : i === 4 ? {attempts: 3, checksPassed: 2, kept: 2, ranks: false, knownMicroUsd: 90_000, unknownRequests: 1} : null, others: [], outputMicroPerMtok: m.price?.outputMicroPerMtok ?? null,
      turnMicroUsd: m.billing === 'free' || m.billing === 'local' || m.billing === 'subscription' ? 0 : m.price ? Math.round((1600 * m.price.inputMicroPerMtok + 6400 * (m.price.cachedInputMicroPerMtok ?? m.price.inputMicroPerMtok) + 1000 * m.price.outputMicroPerMtok) / 1e6) : null,
      fact: i % 3 === 0 ? {source: 'terminal-bench', sourceName: 'Terminal-Bench 4.x', licence: 'Apache-2.0', url: null, name: m.modelId, place: 3 + i, of: 71, rank: 0.9, score: 64.85, unit: '% resolved',
        ci: 3.11, date: '2026-09-22', derived: role !== 'code', agent: 'claude-code', effort: 'max', votes: null, ranks: true, measures: 'tasks solved in a terminal by an agent with the model'} : null}))),
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
    setClientLimits: async (id, sessionUsd, dayUsd) => {
      const c = clients.find(x => x.id === id);
      if (c) {
        c.sessionBudgetMicroUsd = sessionUsd == null ? null : Math.round(sessionUsd * 1_000_000);
        c.dayBudgetMicroUsd = dayUsd == null ? null : Math.round(dayUsd * 1_000_000);
      }
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
    usageRequestsMatching: async (days, filter, offset, limit) => {
      const since = Date.now() - days * 86_400_000;
      const rows = events.filter(e => e.createdAt >= since
        && (filter.client === null || (e.client ?? '') === filter.client)
        && (filter.model === null || `${e.provider}/${e.model}` === filter.model)
        && (filter.show === null || (filter.show === 'unknown' ? e.costStatus === 'unknown' : e.outcome !== 'ok')))
        .sort((a, b) => b.createdAt - a.createdAt);
      return touch({rows: rows.slice(offset, offset + limit), total: rows.length});
    },
    usageRequests: async (days, offset, limit) => {
      const since = Date.now() - days * 86_400_000;
      return touch(events.filter(e => e.createdAt >= since).sort((a, b) => b.createdAt - a.createdAt).slice(offset, offset + limit));
    },
    usageRouting: async days => {
      // As store::routing_summary.
      const list = events.filter(e => e.createdAt >= Date.now() - days * 86_400_000 && e.requestId)
        .sort((a, b) => a.createdAt - b.createdAt || (a.attempt ?? 1) - (b.attempt ?? 1));
      const out: RoutingSummary = {requests: new Set(list.map(e => e.requestId)).size, attempts: list.length, answered: 0,
        answeredAfterFallback: 0, withSession: list.filter(e => e.sessionKey).length, sessions: 0, changes: []};
      const failures = new Map<string, string[]>(), last = new Map<string, string>();
      for (const e of list) {
        if (e.outcome !== 'ok') {
          failures.set(e.requestId!, [...failures.get(e.requestId!) ?? [], e.httpStatus && e.httpStatus !== 200 ? `HTTP ${e.httpStatus}` : e.outcome]);
          continue;
        }
        out.answered += 1;
        const failed = failures.get(e.requestId!) ?? [];
        failures.delete(e.requestId!);
        if (failed.length) out.answeredAfterFallback += 1;
        if (!e.sessionKey) continue;
        const target = `${e.provider}/${e.model}`, before = last.get(e.sessionKey);
        last.set(e.sessionKey, target);
        if (before && before !== target) out.changes.push({at: e.createdAt, client: e.client, from: before, to: target, reason: failed.join(', '),
          cacheWritesAfter: e.cacheWriteTokens ?? 0, knownMicroUsd: e.costStatus === 'unknown' ? null : e.costMicroUsd ?? 0});
      }
      out.sessions = last.size;
      out.changes.reverse();
      return touch(out);
    },
    usageTokenMix: async days => {
      // As store::token_mix: each answer's tokens by kind, and its known cost split by its model's current prices.
      const zero = (): TokenParts => ({newInput: 0, cacheRead: 0, cacheWrite: 0, output: 0});
      const add = (into: TokenParts, p: TokenParts) => { into.newInput += p.newInput; into.cacheRead += p.cacheRead; into.cacheWrite += p.cacheWrite; into.output += p.output; };
      const list = events.filter(e => e.createdAt >= Date.now() - days * 86_400_000);
      const mix: TokenMix = {tokens: zero(), cost: zero(), requests: list.length, withTokens: 0, unsplitMicroUsd: 0, unknownCostRequests: 0, byClient: [], byModel: [], misses: [],
        prefix: {sessions: 0, requests: 0, firstToolsTokens: 0, firstSystemTokens: 0, changes: 0, resent: 0, resentTokens: 0, top: []},
        classes: {requests: 0, sessions: 0, groups: [], agentTypes: [], agents: 0, mostAgentsOnePrompt: 0, peakParallelAgents: 0}};
      // As gateway::hints::request_classes for Claude Code's answers: made-up shares, shaped like sessions with subagents.
      const claude = list.filter(e => e.client === 'Claude Code' && e.outcome === 'ok');
      if (claude.length > 0) {
        const share = (f: number) => Math.round(claude.length * f);
        const money = (f: number) => Math.round(claude.reduce((n, e) => n + (e.costMicroUsd ?? 0), 0) * f);
        const group = (key: string, f: number, agents: number, read: number) => ({key, requests: share(f), knownMicroUsd: money(f),
          unknownCost: 0, promptTokens: share(f) * 42_000, cacheReadTokens: Math.round(share(f) * 42_000 * read),
          cacheWriteTokens: Math.round(share(f) * 42_000 * (1 - read) * 0.6), agents});
        mix.classes = {requests: claude.length, sessions: new Set(claude.map(e => e.sessionKey)).size,
          groups: [group('main', 0.58, 0, 0.94), group('subagent', 0.31, 7, 0.71), group('compaction', 0.02, 0, 0.12),
            group('auxiliary', 0.09, 0, 0.35)],
          agentTypes: [group('Explore', 0.22, 5, 0.69), group('general-purpose', 0.09, 2, 0.76)],
          agents: 7, mostAgentsOnePrompt: 4, peakParallelAgents: 3};
      }
      // As store::prefix_meter: each session's tools and system prompt at its first answer, then changes and uncached resends.
      // Claude Code carries far more tool definitions than the others; the sizes are made up, shaped like real ones.
      const sessions = new Map<string, PrefixSession>();
      for (const e of [...list].filter(e => e.sessionKey && e.outcome === 'ok').sort((a, b) => a.createdAt - b.createdAt)) {
        const big = e.client === 'Claude Code', toolsTokens = big ? 16_840 : 2_310, systemTokens = big ? 3_420 : 860;
        const s = sessions.get(e.sessionKey!);
        if (!s) {
          sessions.set(e.sessionKey!, {client: e.client ?? '', session: e.sessionKey!, toolsTokens, systemTokens, requests: 1, changes: 0, resent: 0, resentTokens: 0});
          continue;
        }
        s.requests += 1;
        if (e.cacheMiss === 'prefix_changed') s.changes += 1;
        if (e.cacheMiss && e.cacheMiss !== 'not_reported' && e.cacheMiss !== 'below_minimum') { s.resent += 1; s.resentTokens += toolsTokens + systemTokens; }
      }
      for (const s of sessions.values()) {
        const p = mix.prefix;
        p.sessions += 1; p.requests += s.requests; p.firstToolsTokens += s.toolsTokens; p.firstSystemTokens += s.systemTokens;
        p.changes += s.changes; p.resent += s.resent; p.resentTokens += s.resentTokens;
      }
      mix.prefix.top = [...sessions.values()].sort((a, b) => b.resentTokens - a.resentTokens
        || (b.toolsTokens + b.systemTokens) - (a.toolsTokens + a.systemTokens) || a.session.localeCompare(b.session)).slice(0, 5);
      for (const e of list) {
        if (!e.cacheMiss) continue;
        const g = mix.misses.find(m => m.cause === e.cacheMiss) ?? (mix.misses[mix.misses.length] = {cause: e.cacheMiss, requests: 0, inputTokens: 0});
        g.requests += 1; g.inputTokens += e.inputTokens ?? 0;
      }
      mix.misses.sort((a, b) => b.inputTokens - a.inputTokens || b.requests - a.requests);
      const groups = {client: new Map<string, TokenMixGroup>(), model: new Map<string, TokenMixGroup>()};
      for (const e of list.filter(e => e.inputTokens != null || e.outputTokens != null)) {
        const input = e.inputTokens ?? 0, read = Math.min(e.cacheTokens ?? 0, input), write = e.cacheWriteTokens ?? 0;
        const parts = {newInput: Math.max(0, input - read - write), cacheRead: read, cacheWrite: write, output: e.outputTokens ?? 0};
        mix.withTokens += 1;
        add(mix.tokens, parts);
        const known = e.costStatus === 'unknown' ? null : e.costMicroUsd ?? 0;
        if (known === null) mix.unknownCostRequests += 1;
        else if (known > 0) {
          const price = models.find(m => m.providerId === e.provider && m.modelId === e.model)?.price;
          if (!price) mix.unsplitMicroUsd += known;
          else {
            const w = [parts.newInput * price.inputMicroPerMtok, parts.cacheRead * (price.cachedInputMicroPerMtok ?? price.inputMicroPerMtok),
              parts.cacheWrite * (price.cacheWriteMicroPerMtok ?? price.inputMicroPerMtok), parts.output * price.outputMicroPerMtok];
            const all = w.reduce((a, b) => a + b, 0) || 1;
            const s = w.map(x => Math.floor(known * x / all));
            s[w.indexOf(Math.max(...w))] += known - s.reduce((a, b) => a + b, 0);
            add(mix.cost, {newInput: s[0], cacheRead: s[1], cacheWrite: s[2], output: s[3]});
          }
        }
        for (const [map, key] of [[groups.client, e.client ?? ''], [groups.model, `${e.provider}/${e.model}`]] as const) {
          const g = map.get(key) ?? {key, tokens: zero(), requests: 0, knownMicroUsd: 0};
          add(g.tokens, parts); g.requests += 1; g.knownMicroUsd += known ?? 0; map.set(key, g);
        }
      }
      const total = (g: TokenMixGroup) => g.tokens.newInput + g.tokens.cacheRead + g.tokens.cacheWrite + g.tokens.output;
      mix.byClient = [...groups.client.values()].sort((a, b) => total(b) - total(a));
      mix.byModel = [...groups.model.values()].sort((a, b) => total(b) - total(a));
      return touch(mix);
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
    // Clearing: shaped like a long Codex session on Claude Haiku, where the rewrites outweigh what clearing saves.
    usageShadow: async () => touch({requests: 48, requestsWithRepeats: 11, repeatedTokens: 38_400, inputTokens: 1_420_000,
      cacheReadTokens: 1_050_000, clearing: {requests: 31, overTrigger: 6, clearedTokens: 412_000, rewrittenTokens: 236_000,
        savedMicroUsd: 4_120, extraMicroUsd: 27_140, netMicroUsd: -23_020, unpriced: stress ? 2 : 0},
      // Lifetime: Claude Code sessions with a few coffee breaks; the one-hour lifetime pays, the keep-alive does not.
      lifetime: {sessions: 9, requests: 214, idleGaps: 7, expiredMisses: 6, recoveredTokens: 486_000,
        savedMicroUsd: 559_000, hourExtraMicroUsd: 402_000, hourNetMicroUsd: 157_000, keepAliveRequests: stress ? 1_204 : 61,
        keepAliveExtraMicroUsd: 612_000, keepAliveNetMicroUsd: -53_000, unpriced: stress ? 3 : 0}}),
    usageAlerts: async () => {
      const day = Date.now() - 86_400_000;
      const unknown = events.filter(e => e.createdAt >= day && e.costStatus === 'unknown').length;
      const near = clients.flatMap(c => {
        const view = clientView(c);
        return !c.revokedAt && c.monthlyBudgetMicroUsd && view.monthSpendMicroUsd >= 0.8 * c.monthlyBudgetMicroUsd
          ? [{kind: 'near_limit' as const, client: c.name, value: view.monthSpendMicroUsd, against: c.monthlyBudgetMicroUsd}] : [];
      });
      return touch([...near, ...(unknown ? [{kind: 'unknown_costs' as const, client: null, value: unknown, against: 0}] : [])]);
    },
    exportUsage: async () => { await sleep(200); return 'C:\\Users\\you\\Downloads\\pygg-usage.csv'; },
    ...demoSkills(),
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
        const b = buckets.get(start) ?? {start, requests: 0, failed: 0, knownMicroUsd: 0, actualMicroUsd: 0, unknownRequests: 0,
          inputTokens: 0, outputTokens: 0};
        b.requests += 1;
        b.inputTokens += e.inputTokens ?? 0;
        b.outputTokens += e.outputTokens ?? 0;
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

const DEMO_ANSWER = 'The app starts in src/main.tsx:1, which mounts <App/>. The gateway is a Rust server started from src-tauri/src/lib.rs:140 on port 8741; the database is opened in src-tauri/src/db.rs:58 (SQLite, WAL mode).\n\nRequests go: client → gateway/mod.rs (routing) → a provider connection → the ledger (usage_events).';
const DEMO_ACTIVITY = ['OpenCode started', 'read src/timeout.ts', 'grep setTimeout', '» The delay is multiplied by 100; ms is already milliseconds.',
  'edit src/timeout.ts', '$ npm test', '» All 12 tests pass.'];
const DEMO_DIFF = 'diff --git a/src/timeout.ts b/src/timeout.ts\n--- a/src/timeout.ts\n+++ b/src/timeout.ts\n@@ -3,3 +3,3 @@\n-  return setTimeout(fn, ms * 100);\n+  return setTimeout(fn, ms);\n';

/** Tasks for the browser preview: the sample tasks, and new ones that "run" for a few seconds. Nothing runs. */
function demoTasks(touch: <T>(value: T) => Promise<T>): Pick<GatewayApi, 'boards' | 'board' | 'boardLimits' | 'boardAct' | 'boardSetLimits' | 'brainstorms' | 'brainstorm' | 'brainstormPickPlanner' | 'brainstormRetry' | 'brainstormSetModel' | 'brainstormSetCard' | 'brainstormDecline' | 'brainstormApprove' | 'workspaces' | 'addWorkspace' | 'removeWorkspace' | 'taskBudget' | 'setTaskBudget' | 'setWorkspaceBudget' | 'setWorkspacePrivacy' | 'pickFolder' | 'exportPatch'
  | 'detectChecks' | 'workspaceCheck' | 'tasks' | 'task' | 'createTask' | 'cancelTask' | 'decideTask' | 'retryTask' | 'taskDiff' | 'taskActivity' | 'brainstormDiff' | 'plansProgress' | 'brainstormStart' | 'brainstormRefine'
  | 'finishTask'> {
  let dailyLimit: number | null = 2;
  const workspaces: WorkspaceView[] = SAMPLE_WORKSPACES.map((w, i) => ({...w, tasks: 0, dailyBudgetUsd: null, privacyClass: i === 1 ? 'sensitive' as const : 'standard' as const, checks: i === 0
    ? [{command: 'pnpm install --frozen-lockfile', source: 'package.json', approved: true},
       {command: 'pnpm typecheck', source: 'package.json', approved: true}, {command: 'pnpm test', source: 'package.json', approved: true},
       {command: 'cargo test', source: 'Cargo.toml', approved: false}]
    : [{command: 'npm ci', source: 'package.json', approved: false}, {command: 'npm run build', source: 'package.json', approved: false}]}));
  // pygg/brain (ADR-041): a brainstorm waiting for a planner pick, one proposed (with `?stress`: 40 cards, 12 questions
  // and long ids), and one whose planner failed. Models and reasons are shaped like the gateway's.
  const stress = new URLSearchParams(location.search).has('stress');
  const card = (id: string, title: string, role: BrainRole, done: string, dependsOn: string[], model: string | null, reason: string | null, wave: number, dropped = false): BrainCard =>
    ({id, title, role, done, dependsOn, model, reason, estimateMicroUsd: model === 'anthropic/claude-opus-5-5' ? null : model === 'ollama/qwen3:8b' || model?.endsWith(':free') ? 0 : role === 'plan' ? 85_400 : 46_000, fallbacks: model && role !== 'plan' ? [{model: 'google-gemini/gemini-3.1-flash', reason: "no fact for code; tier T2 from Pygg's table; paid; about $0.0090 at the provider's list price"}, {model: 'siliconflow/zai-org/GLM-5', reason: 'SWE-bench Verified #16 of 110 (72.8%) dated 2026-02-17, run with mini-SWE-agent at high for code; free; free (as set)'}] : [], runner: 'engine', class: 'standard', dropped, wave});
  const sonnet = 'openrouter/anthropic/claude-sonnet-5';
  const codeReason = "Terminal-Bench 4.x #3 of 71 (64.85% +/-3.11) dated 2026-09-22, run with claude-code at max for code; paid; about $0.0460 at the provider's list price";
  const proposedCards = (): BrainCard[] => {
    const cards = [
      card('c1', 'Information architecture and data model', 'plan', 'A written model (entities, relations, pages) that the code cards cite', [], 'gemini/gemini-3.1-pro-preview', "LMArena hard prompts #9 of 405 (1525.3 Arena rating) dated 2026-11-18 at high, derived for plan; paid; about $0.0120 at the provider's list price", 1),
      card('c2', 'Product catalogue: model, migration, list and detail pages', 'code', '`pnpm test` passes and /products renders the seed data', ['c1'], sonnet, codeReason, 2),
      card('c3', 'Seed data and fixtures', 'code', '`pnpm seed` runs clean twice', ['c1'], 'ollama/qwen3:8b', "no fact for code; tier T0 from Pygg's table; local; free (as set)", 2),
      card('c4', 'Cart: add, remove, totals', 'code', '`pnpm test` passes; cart totals match the fixture table', ['c2'], sonnet, codeReason, 3),
      card('c5', 'Checkout with Stripe and cash on delivery', 'code', 'Test-mode Stripe charge succeeds; a COD order lands in the orders table', ['c4', 'q3', 'q4'], sonnet, codeReason, 4),
      card('c6', 'End-to-end: browse, add to cart, pay', 'test', 'Playwright run green on CI', ['c5'], 'gemini/gemini-3.1-flash', "Terminal-Bench 4.x #12 of 71 (56.36% +/-2.97) dated 2026-11-12, run with antigravity-cli at high for test, derived; paid; about $0.0090 at the provider's list price", 5),
      card('c7', 'Product variants (size, colour)', 'code', 'Variants selectable on the product page; the cart keeps the variant', ['q4', 'c2'], sonnet, codeReason, 3, true),
      card('c8', 'Security review of checkout and admin', 'review', 'A findings list; every High fixed or accepted by you', ['c5'], 'anthropic/claude-opus-5-5', 'LMArena hard prompts #4 of 405 (1530.8 Arena rating) dated 2026-11-18 at high, derived for review; paid; price unknown', 5),
    ];
    if (stress) for (let i = 9; i <= 40; i++) cards.push(card(`c${i}`, `Card ${i}: ${'a long title that wraps across the row and keeps going '.repeat(3)}`, (['code', 'test', 'review', 'plan'] as BrainRole[])[i % 4],
      `Done criterion ${i}: ${'a command that passes and a page that renders with the seed data, '.repeat(4)}`, [`c${i - 1}`], 'openrouter/qwen/qwen3-coder-480b-a35b-instruct-2507-with-a-very-long-variant-name:free', codeReason, 2 + (i % 4)));
    return cards;
  };
  const brainQuestions = (n: number): BrainQuestion[] => [
    {id: 'q1', text: 'Which payment providers?', answered: true}, {id: 'q2', text: 'Is there an admin area?', answered: true},
    {id: 'q3', text: 'Which language for the storefront?', answered: false}, {id: 'q4', text: 'Do products have variants (size, colour)?', answered: false},
    ...Array.from({length: Math.max(0, n - 4)}, (_, i) => ({id: `q${i + 5}`, text: `Question ${i + 5}: ${'a long clarifying question the planner asked about the requirement, '.repeat(3)}`, answered: i % 2 === 0}))];
  const empty = new URLSearchParams(location.search).has('empty');
  const brainstorms: Brainstorm[] = empty ? [] : [
    {id: 'bs-pick', clientId: 'k-1', session: '3f2a', workspaceId: null, state: 'waiting_pick', requirement: 'Build a shop website', planner: null, promptVersion: 'brain-v1', revision: 0,
      proposal: {questions: [], cards: [], droppedCards: 0}, rawReply: null, note: 'the rule would pick openrouter/anthropic/claude-sonnet-5, which costs money; pick a planner in Pygg', error: null,
      createdAt: Date.now() - 120_000, updatedAt: Date.now() - 120_000},
    {id: 'bs-proposed', clientId: 'k-1', session: '9b1c', workspaceId: workspaces[0]?.id ?? null, state: 'proposed', requirement: 'Add CSV export to the orders report', planner: sonnet, promptVersion: 'brain-v1', revision: 2,
      proposal: {questions: brainQuestions(stress ? 12 : 4), cards: proposedCards(), droppedCards: stress ? 2 : 0}, rawReply: null,
      note: stress ? '2 card(s) the planner wrote did not fit the schema and were left out' : null, error: null, createdAt: Date.now() - 600_000, updatedAt: Date.now() - 60_000},
    {id: 'bs-running', clientId: 'k-app', session: 'app-5d1e', workspaceId: workspaces[0]?.id ?? null, state: 'approved', requirement: 'Product catalogue and checkout for the shop', planner: sonnet, promptVersion: 'brain-v3', revision: 1,
      proposal: {questions: brainQuestions(2).map(q => ({...q, answered: true})), cards: proposedCards(), droppedCards: 0}, rawReply: null, note: null, error: null,
      createdAt: Date.now() - 7_300_000, updatedAt: Date.now() - 60_000, branch: 'str/brain-product-catalogue-and-checkout-bs5d1e00'},
    {id: 'bs-failed', clientId: 'k-1', session: '77aa', workspaceId: null, state: 'failed', requirement: 'Migrate the reports to the new schema', planner: 'ollama/qwen3:8b', promptVersion: 'brain-v1', revision: 0,
      proposal: {questions: [], cards: [], droppedCards: 0}, rawReply: 'Sure! Here is my plan:\n1. Look at the schema\n2. Write the migration', note: null,
      error: 'the planner did not answer in the board schema: no JSON object in the reply', createdAt: Date.now() - 3_600_000, updatedAt: Date.now() - 3_500_000},
  ];
  const brainstorm = (id: string): Brainstorm => { const b = brainstorms.find(x => x.id === id); if (!b) throw 'Brainstorm not found'; return b; };
  const planned = async (b: Brainstorm) => { b.state = 'thinking'; b.note = null; b.error = null; await sleep(600);
    b.state = 'proposed'; b.revision += 1; b.proposal = {questions: brainQuestions(4), cards: proposedCards(), droppedCards: 0}; return clone(b); };
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

  // The board (board.rs): the proposed brainstorm's cards as if approved into the first workspace, in every lane.
  const boardLimits = new Map<string, {wipRunning: number; wipChecking: number}>();
  const boardCards: BoardCard[] = empty ? [] : (() => {
    const ws = workspaces[0]?.id ?? '';
    const tasks = [...details.values()].filter(d => d.workspaceId === ws);
    const byState = (state: string) => tasks.find(d => d.state === state) ?? tasks[0] ?? null;
    const lanes: Record<string, [BoardLane, string | null, string | null]> = {
      c1: ['done', 'done', null], c2: ['review', 'done', null], c3: ['running', 'running', null], c4: ['ready', null, null],
      c5: ['blocked', null, 'Needs q3, q4'], c6: ['discovery', null, null], c7: ['discarded', null, null], c8: ['discovery', null, null]};
    const now = Date.now();
    return proposedCards().map((c, i): BoardCard => {
      const [lane, taskState, reason] = lanes[c.id] ?? [(['discovery', 'ready', 'blocked', 'done'] as BoardLane[])[i % 4], null, i % 4 === 2 ? 'Waiting for your agent' : null];
      const task = taskState ? byState(taskState) : null;
      return {id: `bc-${c.id}`, workspaceId: ws, brainstormId: 'bs-running', cardId: c.id, title: c.title, role: c.role, done: c.done, prompt: c.done, dependsOn: c.dependsOn,
        ladder: [c.model ?? 'auto/code', ...c.fallbacks.map(f => f.model)], agent: 'opencode', runner: c.id === 'c8' ? 'ide' : 'engine', class: c.id === 'c4' ? 'expedite' : 'standard', lane,
        held: c.id === 'c6', taskId: task?.id ?? null, blockedReason: lane === 'blocked' ? reason : null, blockedSince: lane === 'blocked' ? now - 5_400_000 : null, blockedMs: lane === 'blocked' ? 1_200_000 : 0,
        readyAt: now - 7_200_000, startedAt: ['running', 'checking', 'review', 'done'].includes(lane) ? now - 3_600_000 : null, doneAt: lane === 'done' ? now - 1_800_000 : null,
        position: c.wave * 1000 + i, createdAt: now - 7_200_000, updatedAt: now - 60_000, task: task ? summary(task) : null};
    });
  })();
  const boardOf = (workspaceId: string) => {
    const w = findWorkspace(workspaceId);
    const own = boardLimits.get(workspaceId);
    const cards = boardCards.filter(c => c.workspaceId === workspaceId).map(c => ({...c, task: c.taskId && details.has(c.taskId) ? summary(get(c.taskId)) : null}));
    const done = cards.filter(c => c.lane === 'done').length;
    return {workspaceId, workspaceName: w.name, workspacePath: w.path, limits: {wipRunning: own?.wipRunning ?? 1, wipChecking: own?.wipChecking ?? 1, own: !!own}, cards,
      measures: {measured: done >= 5, done, doneToday: done, cycleTimeMedianMs: done ? 1_800_000 : null, cycleTimed: done, leadTimeMedianMs: done ? 5_400_000 : null, blockedTodayMs: 1_200_000,
        money: done ? [{role: 'plan' as BrainRole, kept: 1, knownUsd: 0.172, unknownCalls: 0, usdPerKept: 0.172}, {role: 'code' as BrainRole, kept: done, knownUsd: 0.91 * done, unknownCalls: 2, usdPerKept: 0.91}] : []}};
  };

  return {
    boards: () => touch(workspaces.filter(w => boardCards.some(c => c.workspaceId === w.id)).map(w => ({workspaceId: w.id, workspaceName: w.name,
      cards: boardCards.filter(c => c.workspaceId === w.id && c.lane !== 'discarded').length, attention: boardCards.filter(c => c.workspaceId === w.id && (c.lane === 'blocked' || c.lane === 'review')).length,
      limits: boardLimits.get(w.id) ?? null}))),
    board: async id => touch(boardOf(id)),
    boardLimits: async id => touch(boardOf(id).limits),
    boardAct: async (cardId, action) => {
      const c = boardCards.find(x => x.id === cardId);
      if (!c) throw 'That card no longer exists';
      if (action === 'expedite') { if (boardCards.some(x => x.class === 'expedite' && x.lane !== 'done' && x.lane !== 'discarded' && x.id !== c.id)) throw 'One card at a time can be expedited'; c.class = 'expedite'; }
      else if (action === 'standard') c.class = 'standard';
      else if (action === 'hold') { c.held = true; c.lane = 'discovery'; }
      else if (action === 'release') { c.held = false; c.lane = 'ready'; }
      else if (action === 'drop') c.lane = 'discarded';
      else if (action === 'engine') { c.runner = 'engine'; if (c.lane === 'blocked') { c.lane = 'ready'; c.blockedReason = null; c.blockedSince = null; } }
      else if (action === 'rerun') { if (c.lane !== 'blocked' || !c.taskId) throw 'Only a blocked card whose task failed or was stopped can run again'; c.taskId = null; c.task = null; c.lane = 'ready'; c.blockedReason = null; c.blockedSince = null; }
      c.updatedAt = Date.now();
      await sleep(80);
      return clone(c);
    },
    boardSetLimits: async (workspaceId, running, checking) => {
      if (running == null) boardLimits.delete(workspaceId); else boardLimits.set(workspaceId, {wipRunning: Math.max(1, running), wipChecking: Math.max(1, checking ?? 1)});
      return touch(boardOf(workspaceId).limits);
    },
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
    brainstorms: () => touch(brainstorms),
    brainstorm: async id => touch(brainstorm(id)),
    brainstormPickPlanner: async (id, model) => { const b = brainstorm(id); b.planner = model; return planned(b); },
    brainstormRetry: async id => planned(brainstorm(id)),
    brainstormStart: async (workspaceId, requirement) => {
      if (!requirement.trim()) throw 'Describe what to build';
      const b: Brainstorm = {id: `bs-${Date.now()}`, clientId: 'k-app', session: `app-${Date.now()}`, workspaceId, state: 'thinking', requirement: requirement.trim(),
        planner: sonnet, promptVersion: 'brain-v3', revision: 0, proposal: {questions: [], cards: [], droppedCards: 0}, rawReply: null, note: null, error: null,
        createdAt: Date.now(), updatedAt: Date.now()};
      brainstorms.unshift(b);
      void planned(b);
      return touch(clone(b));
    },
    brainstormRefine: async id => {
      const b = brainstorm(id);
      void planned(b).then(() => { b.proposal.questions = b.proposal.questions.map(q => ({...q, answered: true})); });
      return touch(clone(b));
    },
    brainstormSetModel: async (id, cardId, model, allOfRole) => {
      const b = brainstorm(id);
      const role = b.proposal.cards.find(c => c.id === cardId)?.role;
      for (const c of b.proposal.cards) if (c.id === cardId || (allOfRole && c.role === role)) { c.model = model; c.reason = 'your pick'; }
      return touch(b);
    },
    brainstormSetCard: async (id, cardId, change) => {
      const b = brainstorm(id);
      const c = b.proposal.cards.find(x => x.id === cardId);
      if (!c) throw 'Card not found';
      if (change.dropped != null) c.dropped = change.dropped;
      if (change.runner) c.runner = change.runner;
      if (change.class) c.class = change.class;
      if (change.fallbacks) c.fallbacks = change.fallbacks.filter(m => m !== c.model).map(m => ({model: m, reason: c.fallbacks.find(f => f.model === m)?.reason ?? 'your pick'}));
      return touch(b);
    },
    brainstormDecline: async id => { const b = brainstorm(id); b.state = 'declined'; return touch(b); },
    brainstormApprove: async (id, workspaceId) => {
      const b = brainstorm(id); b.state = 'approved'; b.workspaceId = workspaceId; await sleep(300);
      return [...details.keys()].slice(0, b.proposal.cards.filter(c => !c.dropped).length);
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
      if (task.answer) {
        setTimeout(() => {
          const d = details.get(id);
          if (d?.state !== 'running') return;
          set(id, {state: 'answered', phase: undefined, reason: 'Answered. Nothing was changed; the answer is below.',
            attempts: [{...d.attempts[0], outcome: 'done'}],
            attemptDetails: d.attemptDetails.map((a, i) => i === d.attemptDetails.length - 1 ? {...a, outcome: 'done', summary: DEMO_ANSWER} : a)});
        }, 3000);
        return id;
      }
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
    brainstormDiff: () => touch(DEMO_DIFF),
    plansProgress: async () => {
      const by = new Map<string, {brainstormId: string; total: number; done: number; running: number; waiting: number}>();
      for (const c of boardCards.filter(c => c.brainstormId && c.lane !== 'discarded')) {
        const p = by.get(c.brainstormId!) ?? {brainstormId: c.brainstormId!, total: 0, done: 0, running: 0, waiting: 0};
        p.total += 1;
        if (c.lane === 'done') p.done += 1;
        else if (c.lane === 'review' || c.lane === 'blocked' || c.task?.state === 'waiting_approval') p.waiting += 1;
        else if (c.lane === 'running' || c.lane === 'checking') p.running += 1;
        by.set(c.brainstormId!, p);
      }
      return touch([...by.values()]);
    },
    taskActivity: async (_attempt, after) => {
      const lines = DEMO_ACTIVITY.map((text, i) => ({seq: i + 1, atMs: Date.now() - (DEMO_ACTIVITY.length - i) * 4000, text})).filter(l => l.seq > after);
      return touch({known: true, lines, dropped: 0, done: false});
    },
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
      setUpAt: Date.now() - 86_400_000, backup: `${home}\\AppData\\Roaming\\app.smarttaskrouter.desktop\\integration-backups\\claude-code-1-.claude.json`},
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
            apiKey: '<new-token>', headers: {'x-pygg-session': '{env:PYGG_ATTEMPT_ID}'}},
            models: {auto: {name: 'auto', tool_call: true}, 'auto/cheap': {name: 'auto/cheap', tool_call: true},
              'anthropic/claude-haiku-5-5': {name: 'anthropic/claude-haiku-5-5', tool_call: true, limit: {context: 200000, output: 64000}}}}}}, null, 2)
        : id === 'codex-gateway'
        ? `# Pygg's gateway, added by Pygg (Agents > From Your IDE). Use it with: codex --profile pygg
[model_providers.pygg]
name = "Pygg"
base_url = "http://127.0.0.1:8741/v1"
wire_api = "responses"
experimental_bearer_token = "<new-token>"
env_http_headers = { x-pygg-session = "PYGG_ATTEMPT_ID" }

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
        backupDir: `${home}\\AppData\\Roaming\\app.smarttaskrouter.desktop\\integration-backups`});
    },
    applyIntegration: async (id: string) => {
      const i = find(id);
      if (i.state === 'set_up') throw `Pygg is already set up in ${i.name}`;
      if (i.state === 'set_up_elsewhere') throw `${i.name} already has an MCP server named pygg, set up outside Pygg. Remove it here first, or keep using it.`;
      Object.assign(i, {state: 'set_up', clientName: modelsClient(id, i.name), setUpAt: Date.now(),
        backup: `${home}\\AppData\\Roaming\\app.smarttaskrouter.desktop\\integration-backups\\${id}-${Date.now()}`});
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

/** `?fail` makes every change the preview is asked to make fail with a long error, so each sheet's and screen's error
 *  state can be reviewed (scripts/ui-overflow-check.mjs opens them so). Reads still answer, so the screens fill. */
export function withFailures(api: GatewayApi): GatewayApi {
  if (!new URLSearchParams(location.search).has('fail')) return api;
  const change = /^(gateway(Start|Stop|Configure)|add|signIn|update|set|move|remove|enable|save|delete|create|revoke|launcher(SetUp|Remove)|apply|refreshModels|resume|clear|install|uninstall|import)/;
  const failure = 'The preview was asked to fail: the provider refused the change with HTTP 503 Service Unavailable '
    + '(upstream_connect_error_or_disconnect_or_reset_before_headers, reset reason: connection termination). Try again later.';
  return new Proxy(api, {
    get(target, name, receiver) {
      const value = Reflect.get(target, name, receiver);
      if (typeof name !== 'string' || typeof value !== 'function' || !change.test(name)) return value;
      return async () => { await sleep(200); throw failure; };
    },
  });
}
