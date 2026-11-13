import { useState } from 'react';
import { ArrowRight, CircleAlert, CircleCheck, Circle, Power, Settings2, TerminalSquare } from 'lucide-react';
import { call, errorText, gatewayChanged, type GatewayStatus, type UsageEvent } from '../../lib/api';
import { t } from '../../lib/i18n';
import { isMac } from '../../lib/platform';
import { costText, microUsd, sourceText, tokens } from '../../lib/money';
import { ago } from '../../lib/format';
import { useGateway } from '../../hooks/useGateway';
import { Badge, CodeBlock, CopyButton, ErrorNote, FormRow, Segmented, Sheet, Spinner, Switch } from '../ui';
import { OutcomeBadge } from './Usage';
import { Savings, useSavings } from './Savings';
import { UsageAlerts } from './UsageAlerts';

/** The gateway's screens, in the sidebar's order: the path a 9router or OmniRoute user knows. */
export type GatewayDestination = 'overview' | 'connections' | 'endpoint' | 'tools' | 'combos' | 'usage';
export type Navigate = (to: GatewayDestination, intent?: 'connect' | 'create' | 'unpriced') => void;

export type ClientKind = 'claude' | 'opencode' | 'editor' | 'sdk' | 'curl';
type Shell = 'powershell' | 'posix';

/**
 * Home: is Pygg working, and what is it costing. Before the first request, two steps (a provider, then a tool);
 * after it, today's traffic. How to point a tool at the gateway lives in CLI Tools, the address and keys in Endpoint.
 */
export function Overview({status, onStatus, navigate}: {status: GatewayStatus | null; onStatus: (s: GatewayStatus) => void;
  navigate: Navigate}) {
  const connections = useGateway(api => api.connections());
  const clients = useGateway(api => api.clients());
  const usage = useGateway(api => api.usage(1), [], 10_000);
  const launcher = useGateway(api => api.launcher());
  const integrations = useGateway(api => api.integrations(), [], undefined, 'integrations');
  const latest = useGateway(api => api.latestRequests(), [], 10_000);
  const savings = useSavings(1);
  // Whether auto has a model for a coding agent's request (tools, a short prompt): the check behind "ready".
  const codingPlan = useGateway(api => api.autoSimulate(null, 'auto', {surfaceAnthropic: false, tools: true, vision: false,
    inputTokens: 2000, maxTokens: 1000, thinking: null, effort: null}));
  const [configuring, setConfiguring] = useState(false);

  const activeClients = clients.data?.filter(c => !c.revokedAt) ?? [];
  // A connection counts once it can serve: on, with its key, and with models listed.
  const usable = (connections.data ?? []).some(c => c.enabled && !(c.needsKey && !c.hasKey) && c.models > 0);
  const eligible = codingPlan.data?.ranked.length ?? null;
  // A tool counts as set up once Pygg set it up, or once a key of the user's own has been used.
  const toolSetUp = !!launcher.data?.setUp || (integrations.data ?? []).some(i => i.kind === 'gateway' && i.state === 'set_up')
    || activeClients.some(c => c.lastUsedAt && c.name !== 'Pygg (model tests)' && !c.name.endsWith('(MCP)'));
  // The last step is done once a tool's request was answered, not merely sent: a refused first request is not "working".
  const own = (name: string | null | undefined) => !!name && name !== 'Pygg (model tests)' && !name.endsWith('(MCP)');
  const used = (latest.data ?? []).some(r => r.outcome === 'ok' && own(activeClients.find(c => c.id === r.clientId)?.name));
  const steps = [
    {done: usable && eligible !== 0, title: t('Connect a provider'),
      detail: (connections.data?.length ?? 0) > 0 && !usable
        ? t('A connection exists, but none is on with a key and models listed yet. Open Providers to check it.')
        : usable && eligible === 0
        ? t('Connected, but auto has no model that qualifies for a coding agent (tools, tier T2 or higher, a price or free billing). See How auto chooses in Combos.')
        : t('An API key or a local runtime such as Ollama. Keys stay in the Windows Credential Manager.'),
      action: (primary: boolean) => <button type="button" className={primary ? 'button prominent' : 'button'} onClick={() => navigate('connections', 'connect')}>{t('Connect Provider…')}</button>},
    {done: toolSetUp, title: t('Set up your coding tool'),
      detail: t('Claude Code with the launcher, Codex and OpenCode in one click, editors and SDKs by hand. Use the model auto.'),
      action: (primary: boolean) => <button type="button" className={primary ? 'button prominent' : 'button'} onClick={() => navigate('tools')}>{t('Open CLI Tools')}</button>},
    {done: used, title: t('First request answered'),
      detail: t('Work in your IDE or terminal. The first request a tool sends through Pygg shows here, with the model that answered and what it cost.'),
      action: () => null},
  ];
  const ready = steps.every(s => s.done);
  const next = steps.findIndex(s => !s.done);

  return <div className="page">
    <GatewayCard status={status} onStatus={onStatus} onConfigure={() => setConfiguring(true)}/>
    <UsageAlerts onOpen={() => navigate('usage')} onUnpriced={() => navigate('connections', 'unpriced')}/>
    {(usage.data?.total.requests ?? 0) > 0 && <Savings savings={savings} period={t('last 24 hours')}/>}
    {status?.accountingError && <ErrorNote>
      {t('The last usage entry couldn’t be written: {error}. The request stays in the journal and is recorded with an unknown cost when the gateway next starts. Requests are refused while nothing can be written.', {error: status.accountingError})}
    </ErrorNote>}

    {!ready && <section className="group">
      <h2>{t('Get Started')}</h2>
      <ol className="group-box steps">
        {steps.map((step, i) => <li key={step.title} className={step.done ? 'step done' : 'step'}>
          <span className="step-check" aria-hidden>{step.done ? <CircleCheck size={18}/> : <Circle size={18}/>}</span>
          <span className="group-label">
            <span>{step.title}</span>
            <small>{step.detail}</small>
          </span>
          {step.done ? <span className="value">{t('Done')}</span> : step.action(i === next)}
        </li>)}
      </ol>
      <p className="group-footnote">
        {t('Then work in your IDE or terminal as usual. auto picks a model from your connections for each request, within your limits; no combo is needed.')}{' '}
        <button type="button" className="link-button" onClick={() => navigate('combos')}>{t('How auto chooses')}<ArrowRight size={12} aria-hidden/></button>
      </p>
    </section>}
    {ready && usage.data && <ClientCheck last={usage.data.recent[0]} onOpen={() => navigate('usage')}/>}

    <section className="group">
      <div className="group-heading">
        <h2>{t('Last 24 Hours')}</h2>
        <button type="button" className="link-button" onClick={() => navigate('usage')}>{t('Open Usage')}<ArrowRight size={13} aria-hidden/></button>
      </div>
      {usage.error && <ErrorNote>{usage.error}</ErrorNote>}
      {usage.data && <>
        <div className="stats">
          <Stat label={t('Requests')} value={String(usage.data.total.requests)}/>
          <Stat label={t('Known cost')} value={microUsd(usage.data.total.knownMicroUsd)}
            note={usage.data.total.knownMicroUsd > usage.data.total.actualMicroUsd ? t('includes estimates') : undefined}/>
          <Stat label={t('Cost unknown')} value={String(usage.data.total.unknownRequests)} note={t('requests, not in the total')}/>
          <Stat label={t('Fallback attempts')} value={String(usage.data.total.fallbacks)} note={t('another model was tried')}/>
          <Stat label={t('Failed')} value={String(usage.data.total.failed)}/>
        </div>
        {usage.data.recent.length > 0 ? <ul className="group-box compact-list">
          {usage.data.recent.slice(0, 6).map(e => <li key={e.id} className="compact-row">
            <span className="compact-time">{ago(e.createdAt)}</span>
            <span className="compact-main"><span className="mono">{e.provider}/{e.model}</span>
              <small>{e.client ?? t('No client')}{e.requestedModel && e.requestedModel !== `${e.provider}/${e.model}` ? ` · ${t('asked for {model}', {model: e.requestedModel})}` : ''}</small></span>
            <span className="compact-tokens">{e.inputTokens != null ? `${tokens(e.inputTokens)} → ${tokens(e.outputTokens ?? 0)}` : '—'}</span>
            <span className={`compact-cost ${e.costStatus}`} title={sourceText(e.costSource)}>{costText(e.costMicroUsd, e.costStatus)}</span>
            <OutcomeBadge outcome={e.outcome} status={e.httpStatus}/>
          </li>)}
        </ul> : <p className="group-footnote">{t('No requests yet. Requests from your clients appear here as they happen.')}</p>}
      </>}
    </section>

    {configuring && status && <ConfigureSheet status={status} onStatus={onStatus} onClose={() => setConfiguring(false)}/>}
  </div>;
}

/**
 * The last step of setting up a client: seeing that it works. Other gateways leave this to the user; here the
 * newest request says so, or says that none has arrived yet. Nothing is sent to check it, so nothing is spent.
 */
export function ClientCheck({last, onOpen}: {last: UsageEvent | undefined; onOpen: () => void}) {
  const open = <button type="button" className="link-button" onClick={onOpen}>{t('See it in Usage')}<ArrowRight size={12} aria-hidden/></button>;
  if (!last) return <p className="client-check waiting" role="status">
    <Circle size={14} aria-hidden/>{t('No request yet. Start the client with these settings; its first request shows here within seconds.')}</p>;
  const who = last.client ?? t('No client');
  if (last.outcome === 'ok') return <p className="client-check ok" role="status"><CircleCheck size={14} aria-hidden/>
    {t('Latest answered request: {client} → {model} · {ago}. Each tool’s own state is in CLI Tools.', {client: who, ago: ago(last.createdAt), model: `${last.provider}/${last.model}`})} {open}</p>;
  return <p className="client-check failed" role="status"><CircleAlert size={14} aria-hidden/>
    {t('The last request, from {client} {ago}, did not succeed.', {client: who, ago: ago(last.createdAt)})} {open}</p>;
}

function Stat({label, value, note}: {label: string; value: string; note?: string}) {
  return <div className="stat">
    <span className="stat-label">{label}</span>
    <span className="stat-value">{value}</span>
    {note && <small>{note}</small>}
  </div>;
}

export function GatewayCard({status, onStatus, onConfigure, copies}: {status: GatewayStatus | null; onStatus: (s: GatewayStatus) => void;
  onConfigure: () => void; /** The copy buttons, instead of one for the root URL. */ copies?: {label: string; text: string}[]}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const act = (f: () => Promise<GatewayStatus>) => {
    setBusy(true);
    setError(null);
    f().then(s => { onStatus(s); gatewayChanged(); }, e => setError(errorText(e))).finally(() => setBusy(false));
  };
  if (!status) return <div className="gateway-card"><Spinner label={t('Checking the gateway…')}/></div>;
  const running = status.state === 'running';
  return <section className={`gateway-card ${status.state}`} aria-label={t('Gateway')}>
    <div className="gateway-card-main">
      <span className="gateway-icon" aria-hidden><TerminalSquare size={22} strokeWidth={1.8}/></span>
      <div className="gateway-card-text">
        <div className="gateway-card-title">
          {running ? <Badge tone="ok">{t('Gateway Running')}</Badge>
            : status.state === 'failed' ? <Badge tone="error">{t('Gateway Couldn’t Start')}</Badge>
            : <Badge tone="off">{t('Gateway Stopped')}</Badge>}
          <span className="secondary">{t('Loopback only · other devices can’t reach it')}</span>
        </div>
        {running && status.baseUrl
          ? <span className="gateway-url mono selectable">{status.baseUrl}</span>
          : <span className="secondary">{status.error ?? (status.finishing
            ? t('Requests that were running are still finishing, so their cost is recorded. Until they do, no other gateway can use this store.')
            : t('Clients can’t connect until the gateway is started.'))}</span>}
      </div>
    </div>
    <div className="gateway-card-actions">
      {running && status.baseUrl && (copies ?? [{label: t('Copy URL'), text: status.baseUrl}]).map(c => <CopyButton key={c.label} text={c.text} label={c.label}/>)}
      <button type="button" className="button" onClick={onConfigure} disabled={busy}><Settings2 size={13} aria-hidden/>{t('Configure…')}</button>
      {running
        ? <button type="button" className="button" disabled={busy} onClick={() => act(() => call(api => api.gatewayStop()))}>
            <Power size={13} aria-hidden/>{t('Stop')}</button>
        : <button type="button" className="button prominent" disabled={busy} onClick={() => act(() => call(api => api.gatewayStart()))}>
            {busy ? <Spinner/> : <Power size={13} aria-hidden/>}{status.state === 'failed' ? t('Try Again') : t('Start Gateway')}</button>}
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </section>;
}

export function ConfigureSheet({status, onStatus, onClose}: {status: GatewayStatus; onStatus: (s: GatewayStatus) => void; onClose: () => void}) {
  const [port, setPort] = useState(String(status.port));
  const [autostart, setAutostart] = useState(status.autostart);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const number = Number(port);
  const valid = /^\d+$/.test(port) && number >= 1024 && number <= 65535;
  const save = () => {
    setBusy(true);
    call(api => api.gatewayConfigure(number, autostart)).then(s => { onStatus(s); gatewayChanged(); onClose(); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  return <Sheet title={t('Gateway Settings')} subtitle={t('Changing the port restarts the gateway; clients need the new URL.')}
    onClose={onClose} busy={busy} className="narrow" footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={save} disabled={busy || !valid}>{t('Save')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Port')} htmlFor="gateway-port" hint={valid ? `http://127.0.0.1:${number}` : t('A number from 1024 to 65535.')}>
        <input id="gateway-port" className="text-input narrow-input" inputMode="numeric" value={port} onChange={e => setPort(e.target.value)} autoFocus/>
      </FormRow>
      <FormRow label={t('Start with the app')} hint={t('Clients work as soon as Pygg is open.')}>
        <Switch checked={autostart} onChange={setAutostart} label={t('Start with the app')}/>
      </FormRow>
      <FormRow label={t('Address')} hint={t('The gateway listens on 127.0.0.1 only. Sharing it with other devices isn’t offered.')}>
        <span className="mono">127.0.0.1</span>
      </FormRow>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

export function ClientSetup({baseUrl, routes, hasToken, onCreateToken}: {
  baseUrl: string; routes: {value: string; label: string}[]; hasToken: boolean; onCreateToken: () => void;
}) {
  const [kind, setKind] = useState<ClientKind>('claude');
  const [shell, setShell] = useState<Shell>(isMac ? 'posix' : 'powershell');
  const [picked, setPicked] = useState<string | null>(null);
  const model = picked && routes.some(r => r.value === picked) ? picked : routes[0]?.value ?? 'coding';
  const snippet = snippetFor(kind, shell, baseUrl, model);
  const notes: Record<ClientKind, string> = {
    claude: t('Claude Code talks the Anthropic Messages API. To an Anthropic connection it passes through unchanged, prompt cache included; other connections get a translation. The session header keeps each terminal on one model.'),
    opencode: t('Add this provider to opencode.json in your project or ~/.config/opencode/. The fixed session header keeps OpenCode on one model of a combo.'),
    editor: t('For Cline, Continue, Roo Code and other editors that call the API from this computer: choose an OpenAI-compatible provider and fill in these fields. Editors that send requests from their own servers can’t reach 127.0.0.1.'),
    sdk: t('Any OpenAI-compatible SDK works the same way: base URL, token, and a route as the model. Send one session header per conversation to keep it on one model.'),
    curl: t('A quick check from a terminal.'),
  };
  return <section className="group">
    <h2>{t('Set Up by Hand')}</h2>
    <div className="group-box client-setup">
      <div className="client-setup-bar">
        <Segmented label={t('Client')} value={kind} onChange={setKind} options={[
          {value: 'claude', label: 'Claude Code'}, {value: 'opencode', label: 'OpenCode'}, {value: 'editor', label: t('Editors')},
          {value: 'sdk', label: t('OpenAI SDK')}, {value: 'curl', label: 'curl'}]}/>
        <span className="spacer"/>
        <label className="inline-field">
          <span>{t('Model')}</span>
          <select className="popup-button" value={model} onChange={e => setPicked(e.target.value)} disabled={!routes.length}>
            {routes.length ? routes.map(r => <option key={r.value} value={r.value}>{r.label}</option>)
              : <option value="coding">{t('No models yet')}</option>}
          </select>
        </label>
        {(kind === 'claude' || kind === 'curl') && <Segmented label={t('Shell')} value={shell} onChange={setShell} options={[
          {value: 'powershell', label: 'PowerShell'}, {value: 'posix', label: 'macOS / Linux'}]}/>}
      </div>
      <CodeBlock code={snippet} label={t('Setup snippet')}/>
      <p className="client-setup-note">
        {notes[kind]}{' '}
        {hasToken
          ? t('Set SMARTTASKROUTER_TOKEN to the Pygg API key you saved when you created it.')
          : <>{t('You’ll need a Pygg API key first.')} <button type="button" className="link-button" onClick={onCreateToken}>{t('Create API Key…')}</button></>}
      </p>
    </div>
  </section>;
}

export function snippetFor(kind: ClientKind, shell: Shell, baseUrl: string, model: string): string {
  const v1 = `${baseUrl}/v1`;
  const json = JSON.stringify({model, messages: [{role: 'user', content: 'Hello'}]});
  if (kind === 'claude') {
    const vars: [string, string][] = [['ANTHROPIC_BASE_URL', baseUrl], ['ANTHROPIC_AUTH_TOKEN', '%TOKEN%'],
      ['ANTHROPIC_MODEL', model], ['ANTHROPIC_DEFAULT_OPUS_MODEL', model], ['ANTHROPIC_DEFAULT_SONNET_MODEL', model],
      ['ANTHROPIC_DEFAULT_HAIKU_MODEL', model]];
    // A fresh session id per terminal keeps each Claude Code session on one model, and so on one prompt cache.
    return shell === 'powershell'
      ? vars.map(([k, v]) => `$env:${k} = ${v === '%TOKEN%' ? '$env:SMARTTASKROUTER_TOKEN' : `"${v}"`}`).join('\n')
        + '\n$env:ANTHROPIC_CUSTOM_HEADERS = "x-smarttaskrouter-session: $([guid]::NewGuid())"\nclaude'
      : vars.map(([k, v]) => `export ${k}=${v === '%TOKEN%' ? '"$SMARTTASKROUTER_TOKEN"' : `"${v}"`}`).join('\n')
        + '\nexport ANTHROPIC_CUSTOM_HEADERS="x-smarttaskrouter-session: $(uuidgen)"\nclaude';
  }
  if (kind === 'editor') {
    return [`Provider:  OpenAI Compatible`, `Base URL:  ${v1}`, `API key:   your Pygg API key (SMARTTASKROUTER_TOKEN)`,
      `Model:     ${model}`].join('\n');
  }
  if (kind === 'opencode') {
    return JSON.stringify({
      $schema: 'https://opencode.ai/config.json',
      provider: {smarttaskrouter: {npm: '@ai-sdk/openai-compatible', name: 'Pygg',
        options: {baseURL: v1, apiKey: '{env:SMARTTASKROUTER_TOKEN}', headers: {'x-smarttaskrouter-session': 'opencode'}},
        models: {[model]: {name: model}}}},
      model: `smarttaskrouter/${model}`,
    }, null, 2);
  }
  if (kind === 'sdk') {
    return [
      'import os',
      'from openai import OpenAI',
      '',
      `client = OpenAI(base_url="${v1}", api_key=os.environ["SMARTTASKROUTER_TOKEN"],`,
      '                default_headers={"x-smarttaskrouter-session": "my-conversation"})',
      `reply = client.chat.completions.create(model="${model}", messages=[{"role": "user", "content": "Hello"}])`,
      'print(reply.choices[0].message.content)',
    ].join('\n');
  }
  return shell === 'powershell'
    ? [`Invoke-RestMethod ${v1}/chat/completions -Method Post \``,
       `  -Headers @{ Authorization = "Bearer $env:SMARTTASKROUTER_TOKEN" } -ContentType 'application/json' \``,
       `  -Body '${json}'`].join('\n')
    : [`curl ${v1}/chat/completions \\`, '  -H "Authorization: Bearer $SMARTTASKROUTER_TOKEN" \\',
       '  -H "Content-Type: application/json" \\', `  -d '${json}'`].join('\n');
}
