import { useMemo, useState } from 'react';
import { CircleAlert, CircleCheck, Circle, TerminalSquare } from 'lucide-react';
import { call, errorText, gatewayChanged, type GatewayStatus, type LatestRequest } from '../../lib/api';
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
 * Nothing is sent to find out.
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
  const latestFor = (clientId: string | undefined | null) => clientId ? latest.data?.find(r => r.clientId === clientId) : undefined;
  const byName = (name: string | null) => active.find(c => c.name === name);
  return <div className="page">
    <p className="page-intro">{t('Point your coding tool at Pygg and choose the model auto, then work as usual. Pygg picks an eligible model for each request from your providers, free and cheap ones first where they qualify, within your limits, and records what each request cost.')}</p>
    <section className="group">
      <h2>{t('One-Click Setup')}</h2>
      <p className="group-intro">{t('Each tool gets a Pygg API key of its own, so you can see what it spends and whether it works. Pygg backs up a tool’s configuration before changing it; Remove puts it back and revokes the key.')}</p>
      <ClaudeLauncher latestFor={latestFor}/>
      <Integrations kind="gateway" status={i => <ToolStatus latest={latestFor(byName(i.clientName)?.id)}/>}/>
    </section>
    <details className="manual-setup">
      <summary>{t('Set up by hand: Claude Code without the launcher, Cline, Continue, SDKs, curl')}</summary>
      <ClientSetup baseUrl={status?.baseUrl ?? `http://127.0.0.1:${status?.port ?? 8741}`} routes={routes}
        hasToken={active.length > 0} onCreateToken={() => navigate('endpoint', 'create')}/>
    </details>
  </div>;
}

/** Configured, waiting for the first request; or working; or the last request failed. */
function ToolStatus({latest}: {latest: LatestRequest | undefined}) {
  if (!latest) return <p className="client-check waiting tool-status" role="status"><Circle size={13} aria-hidden/>
    {t('Configured. Waiting for its first request: start the tool and send one.')}</p>;
  const model = `${latest.provider}/${latest.model}`;
  if (latest.outcome === 'ok') return <p className="client-check ok tool-status" role="status"><CircleCheck size={13} aria-hidden/>
    {t('Working: answered by {model} {ago}.', {model, ago: ago(latest.createdAt)})}</p>;
  return <p className="client-check failed tool-status" role="status"><CircleAlert size={13} aria-hidden/>
    {t('Its last request, {ago}, did not succeed ({outcome}). Usage has the details.', {ago: ago(latest.createdAt), outcome: latest.outcome})}</p>;
}

/** Claude Code: pointing its settings at Pygg would stop it using the user's Claude plan, so a launcher does it per launch. */
function ClaudeLauncher({latestFor}: {latestFor: (clientId: string | null) => LatestRequest | undefined}) {
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
          ? t('Start Claude Code through Pygg with the command below; plain claude still uses your Claude sign-in. The key is in the Windows Credential Manager, not in a file.')
          : t('Claude Code pointed at another endpoint stops using your Claude plan, so Pygg changes none of its settings. Set Up makes a key for it, kept in the Windows Credential Manager, and a command that starts Claude Code through Pygg with auto, one launch at a time.')}</small>
        {l.setUp && <CodeBlock code={command} label={t('Start Claude Code through Pygg')}/>}
        {l.setUp && <small className="secondary">{t('From npm (npm install -g @pygg/cli@dev) it is pygg run claude. Add Claude Code’s own arguments after it; --model before claude picks another model for every slot.')}</small>}
        {l.setUp && <ToolStatus latest={latestFor(l.clientId)}/>}
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

