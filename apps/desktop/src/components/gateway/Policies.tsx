import { useEffect, useState } from 'react';
import { ShieldCheck } from 'lucide-react';
import { TIERS, call, errorText, gatewayChanged, type Client, type ClientDefaults, type Tier } from '../../lib/api';
import { t } from '../../lib/i18n';
import { microUsd, parseUsd } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { Badge, ErrorNote, FormRow, Segmented, Spinner, Switch } from '../ui';
import { LimitSheet } from './Clients';

type Go = (to: 'connections' | 'endpoint') => void;

/**
 * Policies (SCREENS §2.2): every rule the gateway applies, in one place. Defaults for new API keys, each key's own
 * policy (edited with the same Limits sheet as Endpoint), each connection's terms and quota, the guarantees that are not
 * settings, and a summary of the smart-task rules, which are on hold.
 */
export function Policies({go, onSettings, onWorkspaces}: {go: Go; onSettings: () => void; onWorkspaces: () => void}) {
  const clients = useGateway(api => api.clients());
  const connections = useGateway(api => api.connections());
  const budget = useGateway(api => api.taskBudget());
  const [editing, setEditing] = useState<Client | null>(null);
  const active = (clients.data ?? []).filter(c => !c.revokedAt);
  return <div className="page">
    <Defaults/>

    <section className="group">
      <div className="group-heading">
        <h2>{t('API Key Policies')}</h2>
        <button type="button" className="link-button" onClick={() => go('endpoint')}>{t('Open Endpoint')}</button>
      </div>
      {clients.error && <ErrorNote>{clients.error}</ErrorNote>}
      {!clients.data ? <Spinner/> : active.length === 0 ? <p className="group-footnote">{t('No API keys yet. Create one in Endpoint or set up a tool in CLI Tools.')}</p>
        : <div className="group-box table-box"><table className="data-table" aria-label={t('API Key Policies')}>
          <thead><tr>
            <th scope="col">{t('Key')}</th><th scope="col">{t('Monthly limit')}</th><th scope="col">{t('Min. tier')}</th>
            <th scope="col">{t('At once')}</th><th scope="col">{t('Unknown prices')}</th><th scope="col">{t('Paid retries')}</th>
            <th scope="col">{t('Providers')}</th><th scope="col" className="narrow"><span className="sr-only">{t('Actions')}</span></th>
          </tr></thead>
          <tbody>{active.map(c => <tr key={c.id}>
            <td>{c.name}{c.mayRunTasks && <> <Badge tone="info">{t('tasks')}</Badge></>}</td>
            <td>{c.monthlyBudgetMicroUsd === null ? t('none') : `${microUsd(c.monthlyBudgetMicroUsd)} · ${c.strictBudget ? t('strict') : t('estimated')}`}</td>
            <td className="mono">{c.autoMinTier}</td>
            <td>{c.maxConcurrency}</td>
            <td>{c.unknownCostPolicy === 'block' ? t('blocked') : t('allowed')}</td>
            <td>{c.paidRetries}</td>
            <td>{[c.allowedConnections === null ? t('all') : t('{n} only', {n: String(c.allowedConnections.length)}),
              c.requireNoTraining ? t('no training') : null].filter(Boolean).join(' · ')}</td>
            <td className="narrow"><button type="button" className="button small" onClick={() => setEditing(c)}
              aria-label={t('Limits for {name}', {name: c.name})}>{t('Limits…')}</button></td>
          </tr>)}</tbody>
        </table></div>}
      <p className="group-footnote">{t('Min. tier: the lowest tier auto may choose for that key; auto/cheap ignores it. Unknown prices: whether a model without a price may be used while the key has a monthly limit.')}</p>
    </section>

    <section className="group">
      <div className="group-heading">
        <h2>{t('Connection Policies')}</h2>
        <button type="button" className="link-button" onClick={() => go('connections')}>{t('Open Providers')}</button>
      </div>
      {connections.error && !connections.data ? <ErrorNote>{t('Couldn’t load the connections: {error}', {error: connections.error})}{' '}
        <button type="button" className="link-button" onClick={connections.reload}>{t('Try Again')}</button></ErrorNote>
      : !connections.data ? <Spinner label={t('Loading connections…')}/>
      : connections.data.length > 0 ? <div className="group-box table-box"><table className="data-table" aria-label={t('Connection Policies')}>
        <thead><tr><th scope="col">{t('Connection')}</th><th scope="col">{t('Data use')}</th><th scope="col">{t('Quota')}</th>
          <th scope="col">{t('Pool')}</th><th scope="col">{t('Billing')}</th></tr></thead>
        <tbody>{connections.data.map(c => <tr key={c.id} className={c.enabled ? '' : 'disabled-row'}>
          <td>{c.label}{!c.enabled && <> <Badge tone="off">{t('Off')}</Badge></>}</td>
          <td>{c.dataUseTerms === 'no_training' ? t('does not train on requests') : c.dataUseTerms === 'may_train' ? t('may train') : t('not recorded')}
            {c.termsSource && <small className="secondary"> · {c.termsSource}</small>}</td>
          <td>{c.separateQuota ? t('own quota, confirmed') : c.sharesQuotaWith.length ? t('shared with {names}', {names: c.sharesQuotaWith.join(', ')}) : t('assumed shared')}</td>
          <td>{c.pool ?? '—'}</td>
          <td>{c.defaultBilling}</td>
        </tr>)}</tbody>
      </table></div> : <p className="group-footnote">{t('No connections yet.')}</p>}
      <p className="group-footnote">{t('Data-use terms are what you recorded from the provider’s terms, not something Pygg can check. Not recorded counts as may train for keys that require no training.')}</p>
    </section>

    <section className="group">
      <h2>{t('Always On')}</h2>
      <ul className="group-box guarantee-list">
        {[t('The gateway listens on 127.0.0.1 only: other devices cannot reach it, and that is not a setting.'),
          t('Provider keys and API-key copies stay in the Windows Credential Manager; the store keeps only fingerprints.'),
          t('A cost Pygg cannot know is unknown, never $0, and is left out of totals.'),
          t('A paid task step always waits for your approval in the app; no API can approve one.'),
          t('No telemetry: Pygg reports nothing about your use to anyone. A request goes only to the provider it is routed to.'),
        ].map(text => <li key={text}><ShieldCheck size={14} aria-hidden/>{text}</li>)}
      </ul>
    </section>

    <section className="group">
      <h2>{t('Smart Tasks')} <Badge tone="off">{t('on hold')}</Badge></h2>
      <div className="group-box">
        <div className="group-row">
          <span className="group-label">{t('Daily spending limit for paid task steps')}<small>{t('All tasks together; resets at 00:00 UTC.')}</small></span>
          <span className="group-control value">{budget.data ? (budget.data.limitUsd === null ? t('none') : `$${budget.data.limitUsd}`) : '…'}{' '}
            <button type="button" className="link-button" onClick={onSettings}>{t('Change in Settings')}</button></span>
        </div>
        <div className="group-row">
          <span className="group-label">{t('Checks, privacy and budget per workspace')}</span>
          <span className="group-control"><button type="button" className="link-button" onClick={onWorkspaces}>{t('Open Workspaces')}</button></span>
        </div>
      </div>
    </section>
    {editing && <LimitSheet client={editing} onClose={() => setEditing(null)}/>}
  </div>;
}

/** Defaults for new API keys: applied at creation, by the app, the CLI and a tool's set-up alike. */
function Defaults() {
  const saved = useGateway(api => api.clientDefaults());
  const [d, setD] = useState<ClientDefaults | null>(null);
  const [limit, setLimit] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ok: boolean; text: string} | null>(null);
  useEffect(() => {
    if (saved.data && !d) { setD(saved.data); setLimit(saved.data.monthlyBudgetMicroUsd === null ? '' : String(saved.data.monthlyBudgetMicroUsd / 1_000_000)); }
  }, [saved.data, d]);
  if (saved.error && !d) return <ErrorNote>{t('Couldn’t load the defaults: {error}', {error: saved.error})}{' '}
    <button type="button" className="link-button" onClick={saved.reload}>{t('Try Again')}</button></ErrorNote>;
  if (!d) return <Spinner/>;
  const set = (patch: Partial<ClientDefaults>) => { setD({...d, ...patch}); setMessage(null); };
  const parsedLimit = limit.trim() === '' ? null : parseUsd(limit);
  const limitValid = limit.trim() === '' || (parsedLimit !== null && parsedLimit > 0);
  const save = () => {
    setBusy(true);
    const next = {...d, monthlyBudgetMicroUsd: parsedLimit === null ? null : Math.round(parsedLimit * 1_000_000)};
    call(api => api.setClientDefaults(next)).then(() => { setMessage({ok: true, text: t('Saved. Keys created from now on get these.')}); saved.reload(); gatewayChanged(); },
      e => setMessage({ok: false, text: errorText(e)})).finally(() => setBusy(false));
  };
  return <section className="group">
    <h2>{t('Defaults for New API Keys')}</h2>
    <p className="group-intro">{t('Applied when a key is created: in Endpoint, by pygg gateway create-client, or by a tool’s set-up in CLI Tools. Existing keys keep their saved settings; change them below.')}</p>
    <div className="group-box form defaults-form">
      <FormRow label={t('Minimum model tier')} hint={t('The lowest tier auto may choose. T2 suits coding agents; lower lets cheaper, weaker models answer. auto/cheap and combos ignore it.')}>
        <select className="popup-button" value={d.autoMinTier ?? 'T2'} onChange={e => set({autoMinTier: e.target.value as Tier})} aria-label={t('Minimum model tier')}>
          {TIERS.map(x => <option key={x} value={x}>{x}</option>)}
        </select>
      </FormRow>
      <FormRow label={t('Monthly limit')} htmlFor="default-limit" hint={limitValid ? t('Empty: no limit. A limit counts known spend and spend that may have been charged.') : t('Enter an amount above 0, or leave it empty.')}>
        <span className={limitValid ? 'money-input' : 'money-input invalid'}><span aria-hidden>$</span>
          <input id="default-limit" inputMode="decimal" value={limit} placeholder={t('No limit')} onChange={e => { setLimit(e.target.value); setMessage(null); }}
            aria-invalid={!limitValid}/></span>
      </FormRow>
      <FormRow label={t('Limit mode')} hint={t('Strict reserves the most a request could cost before sending it; a model without a price or maximum output is not used. Estimated counts requests as they finish, so one last request can pass the limit. Provider price changes or unreported charges can make the bill differ either way.')}>
        <Segmented<'estimated' | 'strict'> label={t('Limit mode')} value={d.strictBudget ? 'strict' : 'estimated'}
          onChange={v => set({strictBudget: v === 'strict'})}
          options={[{value: 'estimated', label: t('Estimated')}, {value: 'strict', label: t('Strict')}]}/>
      </FormRow>
      <details className="form-disclosure"><summary>{t('When to choose which')}</summary>
        <p className="secondary">{t('A strict limit holds each request at the most it could cost (output limit × price): Claude Code on a large repository is held at over $1 a request, so prefer the estimated limit there.')}</p></details>
      <FormRow label={t('Unknown prices')} hint={t('While a key has a limit: block keeps it from models without a price, whose cost could not be counted.')}>
        <Segmented<'block' | 'allow'> label={t('Unknown prices')} value={d.unknownCostPolicy ?? 'block'} onChange={v => set({unknownCostPolicy: v})}
          options={[{value: 'block', label: t('Block')}, {value: 'allow', label: t('Allow')}]}/>
      </FormRow>
      <FormRow label={t('Paid retries')} hint={t('Further models tried after a timeout or server error the provider may have billed. 0 stops there.')}>
        <select className="popup-button" value={d.paidRetries ?? 0} onChange={e => set({paidRetries: Number(e.target.value)})} aria-label={t('Paid retries')}>
          {[0, 1, 2, 3, 4, 5].map(n => <option key={n} value={n}>{n}</option>)}
        </select>
      </FormRow>
      <FormRow label={t('Requests at once')}>
        <select className="popup-button" value={d.maxConcurrency ?? 8} onChange={e => set({maxConcurrency: Number(e.target.value)})} aria-label={t('Requests at once')}>
          {[1, 2, 4, 8, 16, 32].map(n => <option key={n} value={n}>{n}</option>)}
        </select>
      </FormRow>
      <FormRow label={t('Only providers that do not train')} hint={t('Uses only connections whose recorded terms say they do not train on requests.')}>
        <Switch checked={d.requireNoTraining ?? false} onChange={v => set({requireNoTraining: v})} label={t('Only providers that do not train')}/>
      </FormRow>
      <div className="defaults-actions">
        <button type="button" className="button prominent" onClick={save} disabled={busy || !limitValid}>{busy ? t('Saving…') : t('Save Defaults')}</button>
        {message && (message.ok ? <span className="ok-note" role="status">{message.text}</span> : <ErrorNote>{message.text}</ErrorNote>)}
      </div>
    </div>
  </section>;
}
