import { useMemo, useState, type ReactNode } from 'react';
import { CircleAlert, CircleCheck, Circle, TerminalSquare } from 'lucide-react';
import { CLAUDE_CODE_NEEDS, call, errorText, gatewayChanged, type AutoPlan, type GatewayStatus, type LatestRequest } from '../../lib/api';
import { t } from '../../lib/i18n';
import { ago } from '../../lib/format';
import { useGateway } from '../../hooks/useGateway';
import { Badge, CodeBlock, ConfirmSheet, ErrorNote, Spinner } from '../ui';
import { Integrations } from './Agents';
import { ClientSetup, type Navigate } from './Overview';

/**
 * CLI Tools (SCREENS.md §2.2): point a coding tool at Pygg, as 9router's and OmniRoute's CLI Tools pages do. Claude
 * Code through a launcher (launcher.rs: a key of its own in the credential store, handed over at launch), Codex and
 * OpenCode in one click (integrations.rs: backup, a key of their own, the change, a check; Remove puts it back), the
 * others by hand. Each tool's own key says whether it works: configured, then working once a request has come.
 * Nothing is sent to find out. Before a tool is set up, the same simulation Home uses says whether auto has a model for
 * that tool's requests (Claude Code and OpenCode speak the Anthropic format, Codex the OpenAI one), so set-up does not
 * end in "no eligible model".
 */
export function CliTools({status, navigate}: {status: GatewayStatus | null; navigate: Navigate}) {
  const combos = useGateway(api => api.combos());
  const models = useGateway(api => api.models());
  const clients = useGateway(api => api.clients());
  const latest = useGateway(api => api.latestRequests(), [], 5_000);
  const routes = useMemo(() => [
    {value: 'auto', label: t('auto (automatic)')}, {value: 'auto/cheap', label: t('auto/cheap (automatic, lowest cost)')},
    ...(combos.data ?? []).map(c => ({value: c.name, label: t('{name} (combo)', {name: c.name})})),
    ...(models.data ?? []).filter(m => m.enabled).map(m => ({value: `${m.providerId}/${m.modelId}`, label: `${m.providerId}/${m.modelId}`})),
  ], [combos.data, models.data]);
  const active = (clients.data ?? []).filter(c => !c.revokedAt);
  // A coding agent's request on each surface: tools, a short prompt, no image. New keys get the Policies defaults.
  const needs = {tools: true, vision: false, inputTokens: 2000, maxTokens: 1000, thinking: null, effort: null};
  const anthropicPlan = useGateway(api => api.autoSimulate(null, 'auto', {surfaceAnthropic: true, ...needs}));
  // Claude Code also asks Claude models for adaptive thinking: its check includes that (Codex review 7).
  const claudePlan = useGateway(api => api.autoSimulate(null, 'auto', CLAUDE_CODE_NEEDS));
  const openaiPlan = useGateway(api => api.autoSimulate(null, 'auto', {surfaceAnthropic: false, ...needs}));
  const latestFor = (clientId: string | undefined | null) => clientId ? latest.data?.find(r => r.clientId === clientId) : undefined;
  const byName = (name: string | null) => active.find(c => c.name === name);
  // No provider at all: one warning above the list, not the same one under each tool.
  const noProvider = !!anthropicPlan.data && !!openaiPlan.data && [anthropicPlan.data, openaiPlan.data].every(p => p.ranked.length === 0 && p.excluded.length === 0);
  return <div className="page">
    <p className="page-intro">{t('Point your coding tool at Pygg and choose the model auto, then work as usual. Pygg picks an eligible model for each request from your providers, free and cheap ones first where they qualify, within your limits, and records what each request cost.')}</p>
    <section className="group">
      <h2>{t('One-Click Setup')}</h2>
      <p className="group-intro">{t('Each tool gets a Pygg API key of its own, so you can see what it spends and whether it works. Pygg backs up a tool’s configuration before changing it; Remove puts it back and revokes the key.')}</p>
      {noProvider && <p className="client-check failed tool-status no-provider" role="status"><CircleAlert size={13} aria-hidden/>
        <span>{t('Connect a provider before sending requests through Pygg: until one is connected, every tool’s requests would be refused.')}{' '}
          <button type="button" className="link-button" onClick={() => navigate('connections', 'connect')}>{t('Connect Provider…')}</button></span></p>}
      <ClaudeLauncher latestFor={latestFor} onFailed={client => navigate('usage', `failed:${client}`)} note={noProvider ? null : <Eligibility plan={claudePlan.data} tool={t('Claude Code')} navigate={navigate}/>}/>
      <Integrations kind="gateway" status={i => <ToolStatus latest={latestFor(byName(i.clientName)?.id)}
        onFailed={() => navigate('usage', `failed:${i.clientName ?? ''}`)}/>}
        note={i => noProvider ? null : <Eligibility plan={i.id === 'codex-gateway' ? openaiPlan.data : anthropicPlan.data} tool={i.id === 'codex-gateway' ? t('Codex') : t('OpenCode')} navigate={navigate}/>}/>
    </section>
    <details className="manual-setup">
      <summary>{t('Set up by hand: Claude Code without the launcher, Cline, Continue, SDKs, curl')}</summary>
      <ClientSetup baseUrl={status?.baseUrl ?? `http://127.0.0.1:${status?.port ?? 8741}`} routes={routes}
        hasToken={active.length > 0} onCreateToken={() => navigate('endpoint', 'create')}/>
    </details>
  </div>;
}

/**
 * Before a tool is set up: whether auto has a model for its requests (ROADMAP, Codex review r2 #1). Nothing when it has;
 * otherwise what is missing and where to fix it. Set Up still works, so the warning never blocks.
 */
function Eligibility({plan, tool, navigate}: {plan: AutoPlan | null | undefined; tool: string; navigate: Navigate}) {
  if (!plan || plan.ranked.length > 0) return null;
  const connected = plan.excluded.length > 0;
  return <p className="client-check failed tool-status" role="status"><CircleAlert size={13} aria-hidden/>
    <span>{connected
      ? t('auto has no model yet that qualifies for {tool}’s requests (tools, the minimum tier, a price or free billing, and a provider that takes its format). Its requests would be refused until one does.', {tool})
      : t('No provider is connected yet, so {tool}’s requests through Pygg would be refused. Connect a provider first.', {tool})}{' '}
      <button type="button" className="link-button" onClick={() => navigate(connected ? 'combos' : 'connections', connected ? 'auto' : 'connect')}>
        {connected ? t('How auto chooses') : t('Connect Provider…')}</button></span></p>;
}

/** Configured, waiting for the first request; or working; or the last request failed. */
function ToolStatus({latest, onFailed}: {latest: LatestRequest | undefined; onFailed: () => void}) {
  if (!latest) return <p className="client-check waiting tool-status" role="status"><Circle size={13} aria-hidden/>
    {t('Configured. Waiting for its first request: start the tool and send one.')}</p>;
  const model = `${latest.provider}/${latest.model}`;
  if (latest.outcome === 'ok') return <p className="client-check ok tool-status" role="status"><CircleCheck size={13} aria-hidden/>
    {t('Working: answered by {model} {ago}.', {model, ago: ago(latest.createdAt)})}</p>;
  return <p className="client-check failed tool-status" role="status"><CircleAlert size={13} aria-hidden/>
    <span>{t('Its last request, {ago}, did not succeed ({outcome}).', {ago: ago(latest.createdAt), outcome: latest.outcome})}{' '}
      <button type="button" className="link-button" onClick={onFailed}>{t('View Failed Requests')}</button></span></p>;
}

/** Claude Code: pointing its settings at Pygg would stop it using the user's Claude plan, so a launcher does it per launch. */
function ClaudeLauncher({latestFor, note, onFailed}: {latestFor: (clientId: string | null) => LatestRequest | undefined; note: ReactNode;
  onFailed: (client: string) => void}) {
  const launcher = useGateway(api => api.launcher());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);
  if (launcher.error) return <ErrorNote>{launcher.error}</ErrorNote>;
  if (!launcher.data) return <Spinner label={t('Loading…')}/>;
  const l = launcher.data;
  const setUp = () => {
    setBusy(true);
    setError(null);
    call(api => api.launcherSetUp()).then(() => { launcher.reload(); gatewayChanged(); }, e => setError(errorText(e))).finally(() => setBusy(false));
  };
  const command = l.cliPath ? `& "${l.cliPath}" run claude` : 'pygg run claude';
  return <><ul className="group-box agent-list integrations launcher-row">
    <li className="agent-row">
      <span className="provider-logo" aria-hidden><TerminalSquare size={15}/></span>
      <span className="agent-text">
        <span className="agent-title">{t('Claude Code: models through Pygg')}
          <Badge tone={l.setUp ? 'ok' : 'off'}>{l.setUp ? t('Set up') : l.installed ? t('Not set up') : t('Not found')}</Badge></span>
        <small className="secondary">{l.setUp
          ? t('Start Claude Code with the command below to use your connected providers; plain claude keeps your Claude sign-in.')
          : t('Set Up gives you a command that starts Claude Code through Pygg with auto, one launch at a time; plain claude keeps your Claude sign-in.')}</small>
        <details className="setup-details"><summary>{t('What Set Up changes')}</summary>
          <small className="secondary">{t('Nothing in Claude Code’s own settings: pointed at another endpoint, it would stop using your Claude plan. Pygg makes an API key for the launcher, kept in the Windows Credential Manager and never in a file, and hands it to Claude Code at each launch. Remove revokes the key.')}</small></details>
        {l.setUp && <CodeBlock code={command} label={t('Start Claude Code through Pygg')}/>}
        {l.setUp && <small className="secondary">{t('From npm (npm install -g @pygg/cli@dev) it is pygg run claude. Add Claude Code’s own arguments after it; --model before claude picks another model for every slot.')}</small>}
        {l.setUp && <ToolStatus latest={latestFor(l.clientId)} onFailed={() => onFailed(l.clientName ?? '')}/>}
        {!l.setUp && l.installed && note}
        {error && <ErrorNote>{error}</ErrorNote>}
      </span>
      <span className="row-actions">
        {!l.setUp && <button type="button" className="button small" disabled={busy || !l.installed} onClick={setUp}>
          {busy ? t('Setting Up…') : t('Set Up')}</button>}
        {l.setUp && <button type="button" className="button small" onClick={() => setRemoving(true)}>{t('Remove…')}</button>}
      </span>
    </li>
  </ul>
    {removing && <ConfirmSheet title={t('Remove the Claude Code launcher?')} confirmLabel={t('Remove')}
      message={t('This revokes the launcher’s key and deletes it from the Credential Manager. The command stops working; Claude Code itself is not changed.')}
      onConfirm={async () => { await call(api => api.launcherRemove()); launcher.reload(); gatewayChanged(); }}
      onClose={() => setRemoving(false)}/>}
  </>;
}

