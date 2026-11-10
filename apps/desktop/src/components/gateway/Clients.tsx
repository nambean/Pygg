import { useEffect, useState } from 'react';
import { KeyRound, TriangleAlert } from 'lucide-react';
import { call, errorText, gatewayChanged, type Client } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { isMac } from '../../lib/platform';
import { microUsd, parseUsd } from '../../lib/money';
import { ago } from '../../lib/format';
import { useGateway } from '../../hooks/useGateway';
import { Badge, ConfirmSheet, CopyButton, Empty, ErrorNote, FormRow, Sheet, Spinner } from '../ui';

/** Clients (SCREENS.md §5.7): one token per app that uses the gateway, each with an optional monthly limit. */
export function Clients({intent, onIntentDone}: {intent?: 'create'; onIntentDone: () => void}) {
  const clients = useGateway(api => api.clients(), [], 15_000);
  const [creating, setCreating] = useState(intent === 'create');
  const [limit, setLimit] = useState<Client | null>(null);
  const [revoking, setRevoking] = useState<Client | null>(null);
  useEffect(() => { if (intent === 'create') { setCreating(true); onIntentDone(); } }, [intent, onIntentDone]);

  if (clients.loading && !clients.data) return <div className="page"><Spinner label={t('Loading clients…')}/></div>;
  const list = clients.data ?? [];
  const active = list.filter(c => !c.revokedAt);
  const revoked = list.filter(c => c.revokedAt);
  return <div className="page">
    {clients.error && <ErrorNote>{clients.error}</ErrorNote>}
    {list.length === 0
      ? <Empty icon={<KeyRound size={40} strokeWidth={1.4}/>} title={t('No Client Tokens Yet')}
          action={<button type="button" className="button prominent" onClick={() => setCreating(true)}>{t('Create Client Token…')}</button>}>
          {t('Every app that sends requests through the gateway needs a token. Give each one its own, so you can see what each spends and revoke one without touching the others.')}
        </Empty>
      : <section className="group">
        <h2>{t('Client Tokens')}</h2>
        <div className="group-box table-box">
          <table className="data-table">
            <thead><tr>
              <th scope="col">{t('Client')}</th><th scope="col">{t('Token')}</th><th scope="col">{t('Last used')}</th>
              <th scope="col">{t('This month')}</th><th scope="col" className="narrow"><span className="sr-only">{t('Actions')}</span></th>
            </tr></thead>
            <tbody>
              {[...active, ...revoked].map(c => <tr key={c.id} className={c.revokedAt ? 'disabled-row' : ''}>
                <td><span className="client-name">{c.name}{c.revokedAt && <> <Badge tone="off">{t('Revoked')}</Badge></>}</span>
                  {!c.revokedAt && <small className="secondary">{policySummary(c)}</small>}</td>
                <td className="mono secondary">{c.tokenPrefix}…</td>
                <td className="secondary">{c.lastUsedAt ? ago(c.lastUsedAt) : t('Never')}</td>
                <td><Spend client={c}/></td>
                <td className="narrow row-buttons">{!c.revokedAt && <>
                  <button type="button" className="button small" onClick={() => setLimit(c)}>{t('Limits…')}</button>
                  <button type="button" className="button small" onClick={() => setRevoking(c)}>{t('Revoke…')}</button>
                </>}</td>
              </tr>)}
            </tbody>
          </table>
        </div>
        <p className="group-footnote">{t('Tokens are stored only as a fingerprint; a lost token can’t be shown again. Create a new one and revoke the old.')}</p>
      </section>}

    {creating && <CreateSheet onClose={() => setCreating(false)}/>}
    {limit && <LimitSheet client={limit} onClose={() => setLimit(null)}/>}
    {revoking && <ConfirmSheet title={t('Revoke {name}?', {name: revoking.name})} confirmLabel={t('Revoke Token')}
      message={<p>{t('Requests with this token are refused from now on. Its usage history is kept.')}</p>}
      onClose={() => setRevoking(null)}
      onConfirm={async () => { await call(api => api.revokeClient(revoking.id)); gatewayChanged(); }}/>}
  </div>;
}

function Spend({client: c}: {client: Client}) {
  const limit = c.monthlyBudgetMicroUsd;
  const share = limit ? Math.min(1, c.monthSpendMicroUsd / limit) : 0;
  return <span className="spend">
    <span className="spend-text">{microUsd(c.monthSpendMicroUsd)}{limit ? <span className="secondary"> / {microUsd(limit)}</span> : <span className="secondary"> · {t('no limit')}</span>}</span>
    {limit ? <span className={share >= 1 ? 'meter full' : share >= 0.8 ? 'meter high' : 'meter'} role="meter"
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(share * 100)} aria-label={t('Share of the monthly limit')}>
      <span style={{width: `${share * 100}%`}}/></span> : null}
    {c.reservedMicroUsd > 0 && <small className="secondary">{t('{amount} held for requests in flight', {amount: microUsd(c.reservedMicroUsd)})}</small>}
    {c.monthUnknownRequests > 0 && <small className="secondary">{plural(c.monthUnknownRequests, '+ {n} request with unknown cost', '+ {n} requests with unknown cost')}</small>}
  </span>;
}

function policySummary(c: Client): string {
  const unknown = c.monthlyBudgetMicroUsd === null ? '' : c.unknownCostPolicy === 'block'
    ? ` · ${t('unknown prices blocked')}` : ` · ${t('unknown prices allowed')}`;
  return plural(c.maxConcurrency, 'up to {n} request at once', 'up to {n} requests at once') + unknown;
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
      <p className="warning-note"><TriangleAlert size={15} aria-hidden/>{t('Copy it now. It isn’t stored and can’t be shown again.')}</p>
      <div className="token-box">
        <code className="mono selectable">{created.token}</code>
        <CopyButton text={created.token} label={t('Copy Token')} prominent/>
      </div>
      <p className="sheet-text">{t('The setup snippets in Overview read it from SMARTTASKROUTER_TOKEN. To set that for your user:')}</p>
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
        hint={t('Optional. Requests are refused once this month’s known spend reaches it. Unknown costs can’t be counted.')}>
        <span className={amount !== null && !Number.isFinite(amount) ? 'money-input invalid' : 'money-input'}>
          <span aria-hidden>$</span>
          <input id="client-limit" inputMode="decimal" value={limit} onChange={e => setLimit(e.target.value)} placeholder={t('No limit')}/>
        </span>
      </FormRow>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

function LimitSheet({client: c, onClose}: {client: Client; onClose: () => void}) {
  const [limit, setLimit] = useState(c.monthlyBudgetMicroUsd ? String(c.monthlyBudgetMicroUsd / 1_000_000) : '');
  const [unknown, setUnknown] = useState(c.unknownCostPolicy);
  const [concurrency, setConcurrency] = useState(String(c.maxConcurrency));
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
      await api.setClientPolicy(c.id, {maxConcurrency: inFlight, unknownCostPolicy: unknown});
    }).then(() => { gatewayChanged(); onClose(); }, e => { setError(errorText(e)); setBusy(false); });
  };
  return <Sheet title={t('Limits for {name}', {name: c.name})}
    subtitle={t('Spent so far this month: {amount} known.', {amount: microUsd(c.monthSpendMicroUsd)})} onClose={onClose} busy={busy} className="narrow"
    footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={save} disabled={busy || !valid}>{t('Save Limits')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Monthly limit')} htmlFor="limit" hint={t('Leave empty for no limit. Resets on the 1st (UTC).')}>
        <span className={valid ? 'money-input' : 'money-input invalid'}>
          <span aria-hidden>$</span>
          <input id="limit" inputMode="decimal" value={limit} onChange={e => setLimit(e.target.value)} placeholder={t('No limit')} autoFocus/>
        </span>
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
    </div>
    {c.monthUnknownRequests > 0 && <p className="sheet-text secondary">
      {plural(c.monthUnknownRequests, '{n} request this month had an unknown cost and isn’t counted.', '{n} requests this month had an unknown cost and aren’t counted.')}
    </p>}
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}
