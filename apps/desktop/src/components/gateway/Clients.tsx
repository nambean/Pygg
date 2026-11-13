import { useEffect, useState } from 'react';
import { KeyRound, TriangleAlert } from 'lucide-react';
import { TIERS, call, errorText, gatewayChanged, type Client, type Tier } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { isMac } from '../../lib/platform';
import { microUsd, parseUsd } from '../../lib/money';
import { ago } from '../../lib/format';
import { useGateway } from '../../hooks/useGateway';
import { Badge, ConfirmSheet, CopyButton, Empty, ErrorNote, FormRow, Sheet, Spinner } from '../ui';

/**
 * API Keys (SCREENS.md §5.7, inside Endpoint): Pygg's own keys, one per app that uses the gateway, each with an optional
 * monthly limit. Not the provider keys, which stay with their connection.
 */
export function Clients({intent, onIntentDone}: {intent?: 'create'; onIntentDone: () => void}) {
  const clients = useGateway(api => api.clients(), [], 15_000);
  const [creating, setCreating] = useState(intent === 'create');
  const [limit, setLimit] = useState<Client | null>(null);
  const [revoking, setRevoking] = useState<Client | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  useEffect(() => { if (intent === 'create') { setCreating(true); onIntentDone(); } }, [intent, onIntentDone]);

  if (clients.loading && !clients.data) return <Spinner label={t('Loading API keys…')}/>;
  const list = clients.data ?? [];
  const active = list.filter(c => !c.revokedAt);
  const revoked = list.filter(c => c.revokedAt);
  return <>
    {clients.error && <ErrorNote>{clients.error}</ErrorNote>}
    {list.length === 0
      ? <section className="group">
        <h2>{t('API Keys')}</h2>
        <Empty icon={<KeyRound size={40} strokeWidth={1.4}/>} title={t('No API Keys Yet')}
          action={<button type="button" className="button prominent" onClick={() => setCreating(true)}>{t('Create API Key…')}</button>}>
          {t('Every app that sends requests through the gateway needs a Pygg API key. Give each one its own, so you can see what each spends and revoke one without touching the others. CLI Tools makes them for Codex and OpenCode.')}
        </Empty></section>
      : <section className="group">
        <h2>{t('API Keys')}</h2>
        <p className="group-intro">{t('Pygg’s own keys for your apps, not your providers’ keys.')}</p>
        <div className="group-box table-box">
          <table className="data-table" aria-label={t('API Keys')}>
            <thead><tr>
              <th scope="col">{t('Client')}</th><th scope="col">{t('Token')}</th><th scope="col">{t('Last used')}</th>
              <th scope="col">{t('This month')}</th><th scope="col" className="narrow"><span className="sr-only">{t('Actions')}</span></th>
            </tr></thead>
            <tbody>
              {[...active, ...revoked].map(c => <tr key={c.id} className={c.revokedAt ? 'disabled-row' : ''}>
                <td><span className="client-name">{c.name}{c.revokedAt && <> <Badge tone="off">{t('Revoked')}</Badge></>}</span>
                  {!c.revokedAt && <small className="secondary">{policySummary(c)}</small>}</td>
                <td className="mono secondary"><TokenCell client={c}/></td>
                <td className="secondary">{c.lastUsedAt ? ago(c.lastUsedAt) : t('Never')}</td>
                <td><Spend client={c}/></td>
                <td className="narrow row-buttons">{!c.revokedAt ? <>
                  <button type="button" className="button small" onClick={() => setLimit(c)}
                    aria-label={t('Limits for {name}', {name: c.name})}>{t('Limits…')}</button>
                  <button type="button" className="button small" onClick={() => setRevoking(c)}
                    aria-label={t('Revoke {name}', {name: c.name})}>{t('Revoke…')}</button>
                </> : <button type="button" className="button small" aria-label={t('Delete {name}', {name: c.name})}
                    onClick={() => call(api => api.deleteClient(c.id)).then(gatewayChanged, e => setDeleteError(errorText(e)))}>{t('Delete')}</button>}</td>
              </tr>)}
            </tbody>
          </table>
        </div>
        <p className="group-footnote">
          {t('Show reads a key back from the Windows Credential Manager; the store keeps only its fingerprint. Keys made before this version can’t be shown: create a new one and revoke the old. A deleted key’s past requests keep its name in Usage.')}
          {revoked.length > 0 && <>{' '}<button type="button" className="link-button"
            onClick={() => call(api => api.deleteRevokedClients()).then(gatewayChanged, e => setDeleteError(errorText(e)))}>
            {plural(revoked.length, 'Delete {n} revoked key', 'Delete {n} revoked keys')}</button></>}
        </p>
        {deleteError && <ErrorNote>{deleteError}</ErrorNote>}
      </section>}

    {creating && <CreateSheet onClose={() => setCreating(false)}/>}
    {limit && <LimitSheet client={limit} onClose={() => setLimit(null)}/>}
    {revoking && <ConfirmSheet title={t('Revoke {name}?', {name: revoking.name})} confirmLabel={t('Revoke Token')}
      message={<p>{t('Requests with this token are refused from now on. Its usage history is kept.')}</p>}
      onClose={() => setRevoking(null)}
      onConfirm={async () => { await call(api => api.revokeClient(revoking.id)); gatewayChanged(); }}/>}
  </>;
}

/** The key's prefix; Show reads the whole key back from the credential store, when a copy was kept. */
function TokenCell({client: c}: {client: Client}) {
  const [token, setToken] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (token) return <span className="token-reveal"><span className="selectable token-full">{token}</span>
    <CopyButton text={token} label={t('Copy')}/>
    <button type="button" className="link-button" onClick={() => setToken(null)}>{t('Hide')}</button></span>;
  return <span className="token-reveal">{c.tokenPrefix}…
    {!c.revokedAt && c.tokenViewable && <button type="button" className="link-button"
      aria-label={t('Show the key for {name}', {name: c.name})}
      onClick={() => call(api => api.clientToken(c.id)).then(setToken, e => setError(errorText(e)))}>{t('Show')}</button>}
    {!c.revokedAt && !c.tokenViewable && <span className="secondary small" title={t('Made before Pygg kept a copy in the Credential Manager, or set in a tool’s own configuration.')}>{t('not viewable')}</span>}
    {error && <small className="warn-text">{error}</small>}</span>;
}

function Spend({client: c}: {client: Client}) {
  const limit = c.monthlyBudgetMicroUsd;
  const counted = c.monthSpendMicroUsd + c.monthAtRiskMicroUsd;
  const share = limit ? Math.min(1, counted / limit) : 0;
  return <span className="spend">
    <span className="spend-text">{microUsd(c.monthSpendMicroUsd)}{limit
      ? <span className="secondary" title={c.strictBudget ? t('Strict: each request is held at the most it could cost.') : t('Estimated: counted as requests finish; one last request can pass it.')}> / {microUsd(limit)} · {c.strictBudget ? t('strict') : t('estimated')}</span>
      : <span className="secondary"> · {t('no limit')}</span>}</span>
    {limit ? <span className={share >= 1 ? 'meter full' : share >= 0.8 ? 'meter high' : 'meter'} role="meter"
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(share * 100)} aria-label={t('Share of the monthly limit')}>
      <span style={{width: `${share * 100}%`}}/></span> : null}
    {limit !== null && counted > limit && <small className="error-text">{t('Over the limit by {amount}', {amount: microUsd(counted - limit)})}</small>}
    {c.monthAtRiskMicroUsd > 0 && <small className="secondary"
      title={t('Requests that failed or were cut off after reaching the provider. Their cost is unknown, so the amount held for them counts against the limit.')}>
      {t('+ {amount} possibly charged', {amount: microUsd(c.monthAtRiskMicroUsd)})}</small>}
    {c.reservedMicroUsd > 0 && <small className="secondary">{t('{amount} held for requests in flight', {amount: microUsd(c.reservedMicroUsd)})}</small>}
    {c.monthUnknownRequests > 0 && <small className="secondary">{plural(c.monthUnknownRequests, '+ {n} request with unknown cost', '+ {n} requests with unknown cost')}</small>}
  </span>;
}

function policySummary(c: Client): string {
  const unknown = c.monthlyBudgetMicroUsd === null ? '' : c.unknownCostPolicy === 'block'
    ? ` · ${t('unknown prices blocked')}` : ` · ${t('unknown prices allowed')}`;
  const scope = c.allowedConnections === null ? ''
    : ` · ${plural(c.allowedConnections.length, 'only {n} connection', 'only {n} connections')}`;
  const retries = c.paidRetries > 0
    ? ` · ${plural(c.paidRetries, 'retries {n} model after a server error', 'retries up to {n} models after a server error')}` : '';
  const tasks = c.mayRunTasks ? ` · ${t('may start smart tasks')}` : '';
  const strictness = c.strictBudget && c.monthlyBudgetMicroUsd !== null ? ` · ${t('strict limit')}` : '';
  const training = c.requireNoTraining ? ` · ${t('only providers that do not train on its data')}` : '';
  return plural(c.maxConcurrency, 'up to {n} request at once', 'up to {n} requests at once') + unknown + scope + retries + tasks + training + strictness;
}

function CreateSheet({onClose}: {onClose: () => void}) {
  const [name, setName] = useState('');
  const [limit, setLimit] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<{client: Client; token: string} | null>(null);
  const amount = parseUsd(limit);
  const valid = name.trim() && (amount === null || (Number.isFinite(amount) && amount > 0));
  const create = () => {
    setBusy(true);
    call(api => api.createClient(name.trim(), amount)).then(r => { setCreated(r); setBusy(false); gatewayChanged(); },
      e => { setError(errorText(e)); setBusy(false); });
  };

  if (created) {
    const persist = isMac
      ? `echo 'export SMARTTASKROUTER_TOKEN="${created.token}"' >> ~/.zshrc`
      : `[Environment]::SetEnvironmentVariable('SMARTTASKROUTER_TOKEN', '${created.token}', 'User')`;
    return <Sheet title={t('Token for {name}', {name: created.client.name})} onClose={onClose} footer={<>
      <span className="spacer"/>
      <button type="button" className="button prominent" onClick={onClose}>{t('Done')}</button>
    </>}>
      {created.client.tokenViewable
        ? <p className="sheet-text">{t('Saved in the Windows Credential Manager: API Keys → Show copies it again later. The store keeps only its fingerprint.')}</p>
        : <p className="warning-note"><TriangleAlert size={15} aria-hidden/>{t('Copy it now. The Credential Manager could not keep a copy, so it can’t be shown again.')}</p>}
      <div className="token-box">
        <code className="mono selectable">{created.token}</code>
        <CopyButton text={created.token} label={t('Copy Token')} prominent/>
      </div>
      <p className="sheet-text">{t('The snippets in CLI Tools → Set Up by Hand read it from SMARTTASKROUTER_TOKEN. To set that for your user:')}</p>
      <div className="token-box">
        <code className="mono selectable small-code">{persist}</code>
        <CopyButton text={persist}/>
      </div>
      <p className="sheet-text secondary">{t('Open a new terminal afterwards so it sees the variable.')}</p>
    </Sheet>;
  }
  return <Sheet title={t('Create Client Token')} subtitle={t('For one app, such as Claude Code on this computer.')} onClose={onClose} busy={busy}
    className="narrow" footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={create} disabled={busy || !valid}>{t('Create Token')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Name')} htmlFor="client-name">
        <input id="client-name" className="text-input" value={name} onChange={e => setName(e.target.value)} placeholder="Claude Code" autoFocus/>
      </FormRow>
      <FormRow label={t('Monthly limit')} htmlFor="client-limit"
        hint={t('Optional. Each request is checked against an estimate of its cost before it is sent, and refused once this month’s spend reaches the limit. The estimate isn’t a guarantee: the provider’s bill can be higher.')}>
        <span className={amount !== null && !Number.isFinite(amount) ? 'money-input invalid' : 'money-input'}>
          <span aria-hidden>$</span>
          <input id="client-limit" inputMode="decimal" value={limit} onChange={e => setLimit(e.target.value)} placeholder={t('No limit')}/>
        </span>
      </FormRow>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

export function LimitSheet({client: c, onClose}: {client: Client; onClose: () => void}) {
  const [limit, setLimit] = useState(c.monthlyBudgetMicroUsd ? String(c.monthlyBudgetMicroUsd / 1_000_000) : '');
  const [unknown, setUnknown] = useState(c.unknownCostPolicy);
  const [concurrency, setConcurrency] = useState(String(c.maxConcurrency));
  const connections = useGateway(api => api.connections());
  const [restricted, setRestricted] = useState(c.allowedConnections !== null);
  const [allowed, setAllowed] = useState<string[]>(c.allowedConnections ?? []);
  const [paidRetries, setPaidRetries] = useState(c.paidRetries);
  const [autoMinTier, setAutoMinTier] = useState<Tier>(c.autoMinTier);
  const [mayRunTasks, setMayRunTasks] = useState(c.mayRunTasks);
  const [noTraining, setNoTraining] = useState(c.requireNoTraining);
  const [strict, setStrict] = useState(c.strictBudget);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const amount = parseUsd(limit);
  const inFlight = Number(concurrency);
  const valid = (amount === null || (Number.isFinite(amount) && amount > 0))
    && /^\d+$/.test(concurrency) && inFlight >= 1 && inFlight <= 256;
  const save = () => {
    setBusy(true);
    call(async api => {
      await api.setClientBudget(c.id, amount);
      await api.setClientPolicy(c.id, {maxConcurrency: inFlight, unknownCostPolicy: unknown,
        restrictConnections: restricted, allowedConnections: allowed, paidRetries, autoMinTier, mayRunTasks,
        requireNoTraining: noTraining, strictBudget: strict});
    }).then(() => { gatewayChanged(); onClose(); }, e => { setError(errorText(e)); setBusy(false); });
  };
  return <Sheet title={t('Limits for {name}', {name: c.name})}
    subtitle={c.monthAtRiskMicroUsd > 0
      ? t('Spent so far this month: {amount} known, {risk} possibly charged.', {amount: microUsd(c.monthSpendMicroUsd), risk: microUsd(c.monthAtRiskMicroUsd)})
      : t('Spent so far this month: {amount} known.', {amount: microUsd(c.monthSpendMicroUsd)})} onClose={onClose} busy={busy} className="narrow"
    footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={save} disabled={busy || !valid}>{t('Save Limits')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Monthly limit')} htmlFor="limit" hint={t('Leave empty for no limit. Resets on the 1st (UTC). Checked against an estimate before each request, so the provider’s bill can end up higher.')}>
        <span className={valid ? 'money-input' : 'money-input invalid'}>
          <span aria-hidden>$</span>
          <input id="limit" inputMode="decimal" value={limit} onChange={e => setLimit(e.target.value)} placeholder={t('No limit')} autoFocus/>
        </span>
      </FormRow>
      <FormRow label={t('Strict limit')} htmlFor="strict-budget"
        hint={amount === null ? t('Applies once the client has a limit.')
          : strict ? t('Each request is held at the most it could cost: its own output limit, or all the model can write. Models whose price or maximum output is unknown are not used. The bill can still differ if a provider changes its price.') + ' ' + t('A strict limit holds each request at the most it could cost (output limit × price): Claude Code on a large repository is held at over $1 a request, so prefer the estimated limit there.')
          : t('Off: a request without an output limit is estimated at 4,096 output tokens, so a long answer can take the month past the limit.')}>
        <label className="check-row"><input id="strict-budget" type="checkbox" checked={strict}
          onChange={e => setStrict(e.target.checked)}/>{t('Hold the most a request could cost')}</label>
      </FormRow>
      <FormRow label={t('Unknown price')} htmlFor="unknown-policy"
        hint={amount === null ? t('Applies once the client has a limit.')
          : unknown === 'block' ? t('Models without a price are skipped, so the limit can’t be passed unseen.')
          : t('Models without a price may answer; their cost isn’t counted against the limit.')}>
        <select id="unknown-policy" className="popup-button" value={unknown} onChange={e => setUnknown(e.target.value as 'block' | 'allow')}>
          <option value="block">{t('Block')}</option>
          <option value="allow">{t('Allow')}</option>
        </select>
      </FormRow>
      <FormRow label={t('At once')} htmlFor="concurrency" hint={t('Requests this client may have in flight. More are refused with 429 until one finishes.')}>
        <input id="concurrency" className="text-input narrow-input" inputMode="numeric" value={concurrency}
          onChange={e => setConcurrency(e.target.value)} aria-invalid={!/^\d+$/.test(concurrency) || inFlight < 1 || inFlight > 256}/>
      </FormRow>
      <FormRow label={t('After a server error')} htmlFor="paid-retries"
        hint={paidRetries === 0
          ? t('Stop and return the error. The provider may have run and billed the request, so trying another model could charge twice.')
          : t('Try the next model of the combo. The first provider may still bill the failed request; each try holds its estimated cost against the limit.')}>
        <select id="paid-retries" className="popup-button" value={paidRetries} onChange={e => setPaidRetries(Number(e.target.value))}>
          <option value={0}>{t('Stop (no retry)')}</option>
          {[1, 2, 3, 4, 5].map(n => <option key={n} value={n}>{plural(n, 'Retry {n} model', 'Retry up to {n} models')}</option>)}
        </select>
      </FormRow>
      <FormRow label={t('Auto minimum tier')} htmlFor="auto-tier"
        hint={t('The lowest tier “auto” may choose for this client. “auto/cheap” ignores it.')}>
        <select id="auto-tier" className="popup-button" value={autoMinTier} onChange={e => setAutoMinTier(e.target.value as Tier)}>
          {TIERS.map(x => <option key={x} value={x}>{x}</option>)}
        </select>
      </FormRow>
      <FormRow label={t('Smart tasks')} htmlFor="may-run-tasks"
        hint={mayRunTasks
          ? t('This client may start tasks through pygg-mcp or the task API. They run here with your agents’ sign-in, in a separate worktree; paid steps still wait for your approval in the app.')
          : t('Off: this client may only send model requests.')}>
        <label className="check-row"><input id="may-run-tasks" type="checkbox" checked={mayRunTasks}
          onChange={e => setMayRunTasks(e.target.checked)}/>{t('May start tasks')}</label>
      </FormRow>
      <FormRow label={t('Data use')} htmlFor="no-training"
        hint={noTraining
          ? t('Requests go only to connections whose terms you recorded as not training on requests, under Providers → Advanced → Data use. Not recorded counts as may train.')
          : t('Any connection may answer, whatever its terms say about training.')}>
        <label className="check-row"><input id="no-training" type="checkbox" checked={noTraining}
          onChange={e => setNoTraining(e.target.checked)}/>{t('Only providers that do not train on requests')}</label>
      </FormRow>
      <FormRow label={t('Connections')} htmlFor="scope"
        hint={restricted
          ? t('Requests from this client go only to the connections ticked here, whatever a combo or a fallback says. Use it to keep sensitive work with providers whose terms you have checked.')
          : t('Any connection of a combo may answer, including fallbacks to other providers.')}>
        <select id="scope" className="popup-button" value={restricted ? 'only' : 'all'} onChange={e => setRestricted(e.target.value === 'only')}>
          <option value="all">{t('All connections')}</option>
          <option value="only">{t('Only selected')}</option>
        </select>
      </FormRow>
      {restricted && <fieldset className="check-list" aria-label={t('Allowed connections')}>
        {(connections.data ?? []).map(conn => <label key={conn.id} className="check-row">
          <input type="checkbox" checked={allowed.includes(conn.id)}
            onChange={e => setAllowed(list => e.target.checked ? [...list, conn.id] : list.filter(id => id !== conn.id))}/>
          <span>{conn.label}</span><span className="secondary mono">{conn.id}/…</span>
        </label>)}
        {allowed.length === 0 && <small className="error-text">{t('With nothing ticked, every request from this client is refused.')}</small>}
      </fieldset>}
    </div>
    {c.monthUnknownRequests > 0 && <p className="sheet-text secondary">
      {plural(c.monthUnknownRequests, '{n} request this month had an unknown cost. Only spend held for it counts against the limit.', '{n} requests this month had an unknown cost. Only spend held for them counts against the limit.')}
    </p>}
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}
