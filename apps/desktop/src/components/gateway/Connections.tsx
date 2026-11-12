import { useEffect, useMemo, useState } from 'react';
import { KeyRound, Plug, Plus, RefreshCw, Search, Trash2 } from 'lucide-react';
import { BILLINGS, TIERS, call, errorText, gatewayChanged, type Billing, type Connection, type Health, type Model, type Preset,
  type ModelTest, type Reliability, type Tier } from '../../lib/api';
import { ago, windowText } from '../../lib/format';
import { plural, t } from '../../lib/i18n';
import { billingLabel, parseUsd, priceText } from '../../lib/money';
import { isMac } from '../../lib/platform';
import { useGateway } from '../../hooks/useGateway';
import { Badge, ConfirmSheet, Empty, ErrorNote, FormRow, Segmented, Sheet, Spinner, Switch } from '../ui';
import { ProviderLogo } from './ProviderLogo';

const STORE = isMac ? 'Keychain' : 'Windows Credential Manager';

/** Outcome of the last model refresh per connection, kept while the app is open (not stored). */
const checks = new Map<string, {at: number; ok: boolean; text: string}>();

function billingHint(billing: Billing, freeModels = false): string {
  if (billing === 'free' && freeModels) {
    return t('Only the models it lists at $0 are on. A model with a price, or none, starts off; turn one on to use it.');
  }
  return {
    free: t('Costs are recorded as $0. Use this for free tiers you’re sure about.'),
    local: t('Runs on this computer; API cost is $0.'),
    subscription: t('Counted against a plan’s quota, never shown as money.'),
    paid: t('Billed per token. Enter prices per model to see estimates; without one the cost is Unknown.'),
    unknown: t('Costs stay Unknown unless the provider reports them or you enter a price.'),
  }[billing];
}

/** Connections (SCREENS.md §5.4, gateway first): provider accounts, their models and prices. */
export function Connections({intent, onIntentDone}: {intent?: 'connect'; onIntentDone: () => void}) {
  const list = useGateway(api => api.connections());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(intent === 'connect');
  useEffect(() => { if (intent === 'connect') { setConnecting(true); onIntentDone(); } }, [intent, onIntentDone]);

  const [query, setQuery] = useState('');
  const [show, setShow] = useState<'all' | 'attention' | 'local' | 'anthropic'>('all');
  const connections = list.data ?? [];
  const q = query.trim().toLowerCase();
  const visible = connections.filter(c =>
    (!q || c.label.toLowerCase().includes(q) || c.id.includes(q) || c.presetName.toLowerCase().includes(q))
    && (show === 'all' || (show === 'attention' ? !c.enabled || ['warn', 'error'].includes(connectionState(c).tone)
      : show === 'local' ? c.defaultBilling === 'local' : c.wire === 'anthropic')));
  // Only a connection the list shows can be open below it, so a search never leaves another one's settings in view.
  const selected = visible.find(c => c.id === selectedId) ?? visible[0];

  if (list.loading && !list.data) return <div className="page"><Spinner label={t('Loading connections…')}/></div>;
  return <div className="page">
    {list.error && <ErrorNote>{list.error}</ErrorNote>}
    {connections.length === 0
      ? <Empty icon={<Plug size={40} strokeWidth={1.4}/>} title={t('No Connections Yet')}
          action={<button type="button" className="button prominent" onClick={() => setConnecting(true)}>{t('Connect Provider…')}</button>}>
          {t('Connect an API key from Gemini, OpenRouter, OpenAI and others, or a local runtime such as Ollama. Every client of the gateway can then use its models.')}
        </Empty>
      : <>
        <section className="group">
          <div className="group-heading">
            <h2>{t('Providers')}</h2>
            {connections.length > 3 && <span className="heading-actions">
              <span className="search-field small">
                <Search size={12} aria-hidden/>
                <input type="search" placeholder={t('Search connections')} value={query} onChange={e => setQuery(e.target.value)}
                  aria-label={t('Search connections')}/>
              </span>
              <select className="popup-button" value={show} onChange={e => setShow(e.target.value as typeof show)} aria-label={t('Show')}>
                <option value="all">{t('All connections')}</option>
                <option value="attention">{t('Needs attention')}</option>
                <option value="local">{t('Local')}</option>
                <option value="anthropic">{t('Anthropic API')}</option>
              </select>
            </span>}
          </div>
          <ul className="group-box select-list" aria-label={t('Providers')}>
            {visible.map(c => <ConnectionRow key={c.id} connection={c} selected={c.id === selected?.id}
              onSelect={() => setSelectedId(c.id)}/>)}
            {visible.length === 0 && <li className="table-empty">{t('No connection matches.')}</li>}
          </ul>
        </section>
        {selected && <ConnectionDetail key={selected.id} connection={selected} onRemoved={() => setSelectedId(null)}/>}
      </>}
    {connecting && <ConnectSheet onClose={() => setConnecting(false)} onConnected={id => { setSelectedId(id); setConnecting(false); }}/>}
  </div>;
}

export function connectionState(c: Connection): {tone: 'ok' | 'warn' | 'off' | 'error'; text: string} {
  if (!c.enabled) return {tone: 'off', text: t('Disabled')};
  if (c.needsKey && !c.hasKey) return {tone: 'error', text: t('Key missing')};
  const whole = c.health.find(h => h.modelId === '');
  if (whole?.state === 'invalid_credentials') return {tone: 'error', text: t('Key rejected')};
  if (whole) return {tone: 'warn', text: healthLabel(whole)};
  const check = checks.get(c.id);
  if (check && !check.ok) return {tone: 'error', text: t('Check failed')};
  if (c.models === 0) return {tone: 'warn', text: t('No models yet')};
  if (c.health.length) return {tone: 'warn', text: plural(c.health.length, '{n} model cooling down', '{n} models cooling down')};
  // Green only once a check answered: without one, no problem has been seen, which is not the same as working.
  return check?.ok ? {tone: 'ok', text: t('Checked')} : {tone: 'off', text: t('No problem seen')};
}

export function healthLabel(h: Health): string {
  return {
    invalid_credentials: t('Key rejected'), forbidden: t('Not allowed'), payment_required: t('Payment required'),
    rate_limited: t('Rate-limited'), model_unavailable: t('Model unavailable'), server_error: t('Provider error'),
    unreachable: t('Unreachable'),
  }[h.state];
}

/** "for 1 min 35 s", "until you replace the key". */
function healthWhen(h: Health, now = Date.now()): string {
  if (h.untilMs === null) return h.state === 'invalid_credentials' ? t('until you replace the key') : t('until you clear it');
  const seconds = Math.max(1, Math.round((h.untilMs - now) / 1000));
  return seconds < 90 ? t('for {n} s', {n: seconds}) : t('for {n} min', {n: Math.round(seconds / 60)});
}

/** The connection's cooldowns and blocks, with what each means for routing. */
function HealthList({connection: c}: {connection: Connection}) {
  const [busy, setBusy] = useState(false);
  if (!c.health.length) return null;
  const clear = () => {
    setBusy(true);
    call(api => api.clearConnectionHealth(c.id)).then(gatewayChanged).finally(() => setBusy(false));
  };
  return <div className="health-list" role="status">
    <ul>
      {c.health.map(h => <li key={`${h.modelId}-${h.state}`}>
        <Badge tone={h.state === 'invalid_credentials' || h.state === 'forbidden' ? 'error' : 'warn'}>{healthLabel(h)}</Badge>
        <span className="health-scope mono">{h.modelId || t('whole connection')}</span>
        <span className="secondary">{t('skipped {when}', {when: healthWhen(h)})}</span>
        <small className="health-reason selectable" title={h.reason}>{h.reason}</small>
      </li>)}
    </ul>
    <div className="health-actions">
      <span className="secondary">{t('Skipped targets are not called, so they cost nothing and don’t make the limit worse.')}</span>
      <button type="button" className="button small" onClick={clear} disabled={busy}>{t('Try Again Now')}</button>
    </div>
  </div>;
}

function ConnectionRow({connection: c, selected, onSelect}: {connection: Connection; selected: boolean; onSelect: () => void}) {
  const state = connectionState(c);
  const toggle = (enabled: boolean) => call(api => api.updateConnection(c.id, {enabled})).then(gatewayChanged);
  return <li className={selected ? 'select-row selected' : 'select-row'}>
    <button type="button" className="select-row-main" onClick={onSelect} aria-current={selected ? 'true' : undefined}>
      <ProviderLogo kind={c.kind} name={c.label} size={17}/>
      <span className="select-row-text">
        <span className="select-row-title">{c.label} <span className="mono secondary">{c.id}/…</span></span>
        <small>{c.presetName} · {billingLabel(c.defaultBilling)} · {plural(c.models, '{n} model', '{n} models')}</small>
      </span>
      <Badge tone={state.tone}>{state.text}</Badge>
    </button>
    <Switch checked={c.enabled} onChange={toggle} label={c.enabled ? t('Disable {name}', {name: c.label}) : t('Enable {name}', {name: c.label})}/>
  </li>;
}

function ConnectionDetail({connection: c, onRemoved}: {connection: Connection; onRemoved: () => void}) {
  const models = useGateway(api => api.models(c.id), [c.id]);
  const reliability = useGateway(api => api.reliability(c.id), [c.id], 30_000);
  const [refreshing, setRefreshing] = useState(false);
  const [message, setMessage] = useState<{ok: boolean; text: string} | null>(null);
  const [sheet, setSheet] = useState<'key' | 'remove' | 'add-model' | 'quota' | {price: Model} | {caps: Model} | null>(null);
  const [query, setQuery] = useState('');
  const [show, setShow] = useState<'all' | 'enabled' | 'free' | 'unpriced'>('all');
  const check = checks.get(c.id);

  const refresh = () => {
    setRefreshing(true);
    setMessage(null);
    call(api => api.refreshModels(c.id)).then(r => {
      const text = r.added ? t('{listed} models listed, {added} new.', r) : t('{listed} models listed, nothing new.', r);
      checks.set(c.id, {at: Date.now(), ok: true, text: t('Reachable · {n} models listed', {n: r.listed})});
      setMessage({ok: true, text});
      gatewayChanged();
    }, e => {
      checks.set(c.id, {at: Date.now(), ok: false, text: errorText(e)});
      setMessage({ok: false, text: errorText(e)});
      gatewayChanged();
    }).finally(() => setRefreshing(false));
  };
  const setBilling = (billing: Billing) => call(api => api.updateConnection(c.id, {billing})).then(gatewayChanged);

  const shown = useMemo(() => (models.data ?? []).filter(m =>
    m.modelId.toLowerCase().includes(query.trim().toLowerCase())
    && (show === 'all' || (show === 'enabled' ? m.enabled : show === 'free' ? m.billing === 'free' || m.billing === 'local'
      : !m.price && (m.billing === 'paid' || m.billing === 'unknown')))),
  [models.data, query, show]);

  return <section className="group" aria-label={c.label}>
    <div className="group-heading">
      <h2>{c.label}</h2>
      <span className="heading-actions">
        <button type="button" className="button" onClick={refresh} disabled={refreshing}>
          {refreshing ? <Spinner/> : <RefreshCw size={13} aria-hidden/>}{t('Check & Refresh Models')}</button>
        <button type="button" className="button" onClick={() => setSheet('key')}><KeyRound size={13} aria-hidden/>
          {c.hasKey ? t('Replace Key…') : t('Add Key…')}</button>
        <button type="button" className="button icon-only" onClick={() => setSheet('remove')} aria-label={t('Remove Connection…')}
          title={t('Remove Connection…')}><Trash2 size={14} aria-hidden/></button>
      </span>
    </div>
    <div className="group-box">
      <div className="group-row">
        <span className="group-label">{t('Authentication')}</span>
        <span className="group-control value">{c.hasKey ? t('API key · stored in {store}', {store: STORE})
          : c.needsKey ? <Badge tone="error">{t('Key missing')}</Badge> : t('No key needed')}</span>
      </div>
      <div className="group-row">
        <span className="group-label">{t('Endpoint')}</span>
        <span className="group-control value mono selectable">{c.endpoint}</span>
      </div>
      <div className="group-row">
        <span className="group-label">{t('API')}<small>{c.wire === 'anthropic'
          ? t('Anthropic requests (Claude Code) pass through unchanged, prompt cache included. OpenAI-format clients can’t use it yet.')
          : t('OpenAI-format requests pass through unchanged. Anthropic requests are translated, and prompt-cache hints survive only where the provider accepts them.')}</small></span>
        <span className="group-control value">{c.wire === 'anthropic' ? t('Anthropic Messages') : t('OpenAI Chat Completions')}</span>
      </div>
      <div className="group-row">
        <span className="group-label">{t('Health')}</span>
        <span className="group-control value">{check
          ? <span title={check.text}>{check.ok ? check.text : <Badge tone="error">{check.text}</Badge>} · {t('checked {time}', {time: new Date(check.at).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit'})})}</span>
          : c.health.length ? t('See below') : t('No problems seen')}</span>
      </div>
      {c.health.length > 0 && <div className="group-row stacked"><HealthList connection={c}/></div>}
      {c.readsAccount && <AccountRow connection={c}/>}
      {(c.limits?.length ?? 0) > 0 && <div className="group-row stacked">
        <span className="group-label">{t('Rate limits, as the provider last said')}
          <small>{t('Read from the headers of its last answer for each model. While a limit is used up, auto skips that model until it resets; a provider that sends none shows nothing here.')}</small></span>
        <ul className="limit-list">
          {c.limits!.map(l => <li key={l.modelId}>
            <span className="mono">{l.modelId}</span>
            <small className="secondary">{ago(l.observedAt)}{l.httpStatus && l.httpStatus !== 200 ? ` · HTTP ${l.httpStatus}` : ''}</small>
            <span className="limit-values" title={Object.entries(l.headers).map(([k, v]) => `${k}: ${v}`).join('\n')}>
              {(l.windows?.length ? l.windows.map(windowText) : Object.entries(l.headers).map(([k, v]) => `${k}: ${v}`)).join(' · ')}</span>
          </li>)}
        </ul>
      </div>}
      <div className="group-row">
        <span className="group-label"><label htmlFor={`quota-${c.id}`}>{t('Independent quota confirmed')}</label>
          <small>{c.separateQuota
            ? t('You confirmed on {date} that this connection has its own provider quota. When another connection is rate-limited or refused, this one may still answer. The provider’s own limits still apply.',
                {date: new Date(c.quotaConfirmedAt ?? c.createdAt).toLocaleDateString()})
            : c.sharesQuotaWith.length
              ? t('Shares limits with {names}. While one is rate-limited or refused, the others aren’t used for the same model; using a second key to get around a limit can break the provider’s terms.', {names: c.sharesQuotaWith.join(', ')})
              : t('Assumed to share limits with other {provider} connections you add. Turn on only if the provider gives it a separate quota, for example a different organization.', {provider: c.presetName})}</small></span>
        <span className="group-control"><Switch checked={c.separateQuota} label={t('Independent quota confirmed')}
          onChange={separateQuota => separateQuota ? setSheet('quota')
            : call(api => api.updateConnection(c.id, {separateQuota})).then(gatewayChanged)}/></span>
      </div>
      <PoolRow connection={c}/>
      <div className="group-row">
        <span className="group-label"><label htmlFor={`billing-${c.id}`}>{t('Billing')}</label><small>{billingHint(c.defaultBilling, c.freeModels)}</small></span>
        <span className="group-control">
          <select id={`billing-${c.id}`} className="popup-button" value={c.defaultBilling} onChange={e => setBilling(e.target.value as Billing)}>
            {BILLINGS.map(b => <option key={b} value={b}>{billingLabel(b)}</option>)}
          </select>
        </span>
      </div>
      <TermsRow connection={c}/>
      <div className="group-row">
        <span className="group-label">{t('Model names')}<small>{t('Clients ask for a model of this connection by this name.')}</small></span>
        <span className="group-control value mono selectable">{c.id}/&lt;model&gt;</span>
      </div>
    </div>
    {message && (message.ok ? <p className="group-footnote ok-note" role="status">{message.text}</p> : <ErrorNote>{message.text}</ErrorNote>)}

    <div className="group-heading models-heading">
      <h2>{t('Models')}</h2>
      <span className="heading-actions">
        <span className="search-field small">
          <Search size={13} aria-hidden/>
          <input type="search" placeholder={t('Search models')} value={query} onChange={e => setQuery(e.target.value)} aria-label={t('Search models')}/>
        </span>
        <select className="popup-button" value={show} onChange={e => setShow(e.target.value as typeof show)} aria-label={t('Show')}>
          <option value="all">{t('All models')}</option>
          <option value="enabled">{t('Enabled')}</option>
          <option value="free">{t('Free')}</option>
          <option value="unpriced">{t('Paid or unknown, no price')}</option>
        </select>
        <button type="button" className="button" onClick={() => setSheet('add-model')}><Plus size={13} aria-hidden/>{t('Add Model…')}</button>
      </span>
    </div>
    {models.error && <ErrorNote>{models.error}</ErrorNote>}
    {models.data && models.data.length === 0
      ? <p className="group-footnote">{t('No models yet. Check & Refresh Models asks the provider for its list; Add Model… adds one by name.')}</p>
      : <div className="group-box table-box">
        <table className="data-table" aria-label={t('Models')}>
          <thead><tr>
            <th scope="col">{t('Model')}</th><th scope="col">{t('Billing')}</th><th scope="col">{t('Price')}</th>
            <th scope="col">{t('Auto routing')}</th>
            <th scope="col" className="narrow">{t('Enabled')}</th><th scope="col" className="narrow"><span className="sr-only">{t('Actions')}</span></th>
          </tr></thead>
          <tbody>
            {shown.map(m => <ModelRow key={m.modelId} model={m} onPrice={() => setSheet({price: m})} onCaps={() => setSheet({caps: m})}
              health={c.health.find(h => h.modelId === m.modelId)} reliability={reliability.data?.find(r => r.modelId === m.modelId)}
              onChange={() => { models.reload(); gatewayChanged(); }}/>)}
            {shown.length === 0 && <tr><td colSpan={6} className="table-empty">{t('No models match.')}</td></tr>}
          </tbody>
        </table>
      </div>}

    {sheet === 'key' && <KeySheet connection={c} onClose={() => setSheet(null)}/>}
    {sheet === 'add-model' && <AddModelSheet connection={c} onClose={() => setSheet(null)} onAdded={() => { models.reload(); gatewayChanged(); }}/>}
    {sheet === 'quota' && <ConfirmSheet title={t('Confirm an independent quota for {name}?', {name: c.label})}
      confirmLabel={t('Confirm Independent Quota')} destructive={false}
      message={<>
        <p>{t('A different key is not proof of a different quota. Keys of one organization or project usually share its rate limits and credit.')}</p>
        <p>{t('Confirm only if {provider} gives this connection its own quota, for example a separate organization you are allowed to use. The gateway can’t check this, and the provider’s terms still apply.', {provider: c.presetName})}</p>
      </>}
      onConfirm={() => call(api => api.updateConnection(c.id, {separateQuota: true})).then(gatewayChanged)}
      onClose={() => setSheet(null)}/>}
    {sheet === 'remove' && <ConfirmSheet title={t('Remove {name}?', {name: c.label})} confirmLabel={t('Remove Connection')}
      message={<>
        <p>{t('Its key is deleted from {store}, its models leave every combo, and requests for {id}/… stop working.', {store: STORE, id: c.id})}</p>
        <p>{t('Usage already recorded is kept.')}</p>
      </>}
      onClose={() => setSheet(null)}
      onConfirm={async () => { await call(api => api.removeConnection(c.id)); onRemoved(); gatewayChanged(); }}/>}
    {sheet && typeof sheet === 'object' && 'caps' in sheet && <CapsSheet model={sheet.caps} onClose={() => setSheet(null)}
      onSaved={() => { models.reload(); gatewayChanged(); }}/>}
    {sheet && typeof sheet === 'object' && 'price' in sheet && <PriceSheet model={sheet.price} onClose={() => setSheet(null)}
      onSaved={() => { models.reload(); gatewayChanged(); }}/>}
  </section>;
}

/**
 * The connection's pool (ADR-034): connections of one provider, each a separate account with its own confirmed quota,
 * reached together as `<pool>/<model>` and tried in order, the next one when one is rate-limited, out of credit or
 * refuses its key.
 */
function PoolRow({connection: c}: {connection: Connection}) {
  const all = useGateway(api => api.connections());
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const members = (all.data ?? []).filter(x => c.pool && x.pool === c.pool).sort((a, b) => (a.poolRank ?? 0) - (b.poolRank ?? 0));
  const pools = [...new Set((all.data ?? []).filter(x => x.pool && x.kind === c.kind && x.id !== c.id).map(x => x.pool as string))].sort();
  const act = (f: () => Promise<unknown>) => { setError(null); f().then(() => { all.reload(); gatewayChanged(); }, e => setError(errorText(e))); };
  const join = (pool: string) => act(() => call(api => api.setConnectionPool(c.id, pool)));
  return <div className="group-row stacked">
    <span className="group-label">{t('Pool')}
      <small>{c.pool
        ? t('Requests for {pool}/<model> try these connections in this order, and move to the next when one is rate-limited, out of credit or refuses its key. Each is a separate account you confirmed.', {pool: c.pool})
        : c.separateQuota
          ? t('Group this connection with other {provider} accounts you hold, to reach them as one name. Only accounts with their own quota can join; several keys of one account cannot.', {provider: c.presetName})
          : t('To pool this connection with other {provider} accounts, first confirm that it has its own provider quota (below).', {provider: c.presetName})}</small>
      {error && <small className="warn-text">{error}</small>}</span>
    {c.pool ? <span className="pool-members">
      <ol>{members.map((m, i) => <li key={m.id} className={m.id === c.id ? 'current' : ''}>
        <span>{m.label}</span><small className="secondary mono">{m.id}</small>
        {m.id === c.id && <>
          <button type="button" className="button small" disabled={i === 0} aria-label={t('Earlier')}
            onClick={() => act(() => call(api => api.moveInPool(c.id, true)))}>↑</button>
          <button type="button" className="button small" disabled={i === members.length - 1} aria-label={t('Later')}
            onClick={() => act(() => call(api => api.moveInPool(c.id, false)))}>↓</button>
        </>}
      </li>)}</ol>
      <button type="button" className="button small" onClick={() => act(() => call(api => api.setConnectionPool(c.id, null)))}>{t('Leave Pool')}</button>
    </span> : <span className="group-control pool-join">
      {pools.map(p => <button key={p} type="button" className="button small" disabled={!c.separateQuota} onClick={() => join(p)}>
        {t('Join {pool}', {pool: p})}</button>)}
      <input className="text-input mono" value={name} placeholder={t('new-pool-name')} aria-label={t('New pool name')}
        disabled={!c.separateQuota} onChange={e => setName(e.target.value.toLowerCase())} spellCheck={false}/>
      <button type="button" className="button small" disabled={!c.separateQuota || !name.trim()} onClick={() => join(name.trim())}>
        {t('Create Pool')}</button>
    </span>}
  </div>;
}

/** What the account has left, from the provider's own account endpoint (OpenRouter /key, DeepSeek /user/balance). */
function AccountRow({connection: c}: {connection: Connection}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const a = c.account;
  const check = () => {
    setBusy(true);
    setError(null);
    call(api => api.checkAccount(c.id)).then(gatewayChanged, e => setError(errorText(e))).finally(() => setBusy(false));
  };
  const parts = a ? [
    a.balance !== null ? t('{amount} left', {amount: `${a.balance.toFixed(2)} ${a.currency ?? ''}`.trim()}) : null,
    a.available === false ? t('the provider says paid calls are not possible now') : null,
    a.usedToday !== null ? t('{amount} used today', {amount: `$${a.usedToday.toFixed(2)}`}) : null,
    a.freeRequests ? windowText(a.freeRequests) : null,
  ].filter(Boolean) : [];
  return <div className="group-row">
    <span className="group-label">{t('Account, as the provider says')}
      <small>{a ? t('Read from {source} {when}. Pygg asks again at start-up and when you check the models.', {source: a.source, when: ago(a.observedAt)})
        : t('The provider can say what the account has left. Nothing has been read yet.')}</small>
      {error && <small className="warn-text">{error}</small>}</span>
    <span className="group-control value">{parts.length ? parts.join(' · ') : a ? t('Nothing stated') : ''}
      <button type="button" className="button small" disabled={busy} onClick={check}>{t('Check Now')}</button></span>
  </div>;
}

/** Test Model (SCREENS §6.2): one short request through the gateway; a paid model asks first, as it costs a little. */
function ModelTestButton({model: m}: {model: Model}) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ModelTest | null>(null);
  const [error, setError] = useState<string | null>(null);
  const paid = m.billing === 'paid' || m.billing === 'unknown';
  const run = () => {
    if (paid && !window.confirm(t('Send one short request to {model}? It asks for one word, so it costs a fraction of a cent at its price, recorded in Usage.', {model: m.modelId}))) return;
    setBusy(true);
    setError(null);
    setResult(null);
    call(api => api.testModel(m.providerId, m.modelId)).then(setResult, e => setError(errorText(e))).finally(() => setBusy(false));
  };
  const summary = result && (result.ok
    ? t('Answered “{text}” in {ms} ms', {text: result.text.trim().slice(0, 40), ms: result.latencyMs})
    : result.text);
  return <>
    <button type="button" className="button small" disabled={busy} onClick={run}
      title={summary ?? error ?? t('Send one short request to this model')}>{busy ? t('Testing…') : t('Test')}</button>
    {(summary || error) && <small className={result?.ok ? 'source' : 'warn-text'} role="status">{summary ?? error}</small>}
  </>;
}

function ModelRow({model: m, health, reliability: r, onPrice, onCaps, onChange}: {model: Model; health?: Health; reliability?: Reliability;
  onPrice: () => void; onCaps: () => void; onChange: () => void}) {
  const needsPrice = m.billing === 'paid' || m.billing === 'unknown';
  const price = m.price ? priceText(m.price)
    : needsPrice ? t('No price · cost Unknown') : m.billing === 'subscription' ? t('Quota, not money') : t('Not needed');
  return <tr className={m.enabled ? '' : 'disabled-row'}>
    <td className="mono selectable" title={health ? `${m.modelId} · ${health.reason}` : m.modelId}>
      {m.modelId}{health && <> <Badge tone="warn">{healthLabel(health)}</Badge></>}</td>
    <td>{billingLabel(m.billing)}{!m.enabled && m.enabledSource === 'media-model' && <small className="source"
      title={t('The model catalog says this model makes audio, video or images, priced per item rather than per token, so Pygg turned it off. Turn it on if you mean to use it.')}>
      <br/>{t('Off: makes media')}</small>}{!m.enabled && m.enabledSource === 'free-connection'
      && <small className="source" title={t('This connection is set as free and this model is not listed at $0, so Pygg turned it off. Turn it on to use it; its cost follows its price, or is Unknown without one.')}>
        <br/>{t('Off: not $0')}</small>}</td>
    <td className={m.price ? '' : 'secondary'}>{price}{m.price && <small className="source"> · {m.price.source === 'user' ? t('your price') : t('list price')}</small>}</td>
    <td className={m.capabilities.tier ? '' : 'secondary'}>{capsText(m)}
      {r && r.requests > 0 && <small className="source" title={t('From the ledger over the last 30 days; requests the client left are not counted.')}>
        <br/>{t('{rate}% of {n} answered', {rate: Math.round((r.ok / r.requests) * 100), n: r.requests})}
        {r.medianLatencyMs !== null ? ` · ${t('median {s} s', {s: (r.medianLatencyMs / 1000).toFixed(1)})}` : ''}</small>}</td>
    <td className="narrow"><Switch checked={m.enabled} label={t('Use {model}', {model: m.modelId})}
      onChange={enabled => call(api => api.updateModel(m.providerId, m.modelId, {enabled})).then(onChange)}/></td>
    <td className="narrow row-buttons">
      <button type="button" className="button small" onClick={onPrice}>{m.price ? t('Edit Price…') : t('Set Price…')}</button>
      <button type="button" className="button small" onClick={onCaps}>{t('Auto…')}</button>
      <ModelTestButton model={m}/>
      <button type="button" className="button small icon-only" aria-label={t('Remove {model}', {model: m.modelId})} title={t('Remove')}
        onClick={() => call(api => api.removeModel(m.providerId, m.modelId)).then(onChange)}><Trash2 size={13} aria-hidden/></button>
    </td>
  </tr>;
}

type PresetFilter = 'all' | 'free' | 'local' | 'anthropic';

/** The order and headings of preset groups in Connect: what it takes to start using each. */
const OFFER_GROUPS: {offer: Preset['offer']; title: () => string; hint: () => string}[] = [
  {offer: 'free', title: () => t('Free tier'), hint: () => t('An ongoing free tier or free models, per the provider’s docs. Limits apply.')},
  {offer: 'credits', title: () => t('Trial credits'), hint: () => t('Starting credit or a granted balance, then paid.')},
  {offer: 'paid', title: () => t('Paid'), hint: () => t('Billed per token from the start.')},
  {offer: 'local', title: () => t('On this computer'), hint: () => t('Runs locally: no key, no per-token cost.')},
  {offer: 'custom', title: () => t('Custom'), hint: () => t('Any other OpenAI- or Anthropic-compatible endpoint.')},
];

function presetName(p: Preset): string {
  return p.id === 'custom' ? t('Custom (OpenAI)') : p.id === 'custom-anthropic' ? t('Custom (Anthropic)') : p.name;
}

/** Providers grouped by how to start (free tier, credits, paid, local, custom), with search and filters. */
function PresetPicker({presets, selected, onSelect}: {presets: Preset[]; selected: string; onSelect: (id: string) => void}) {
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<PresetFilter>('all');
  const q = query.trim().toLowerCase();
  const shown = presets.filter(p =>
    (!q || presetName(p).toLowerCase().includes(q) || p.id.includes(q))
    && (filter === 'all' || (filter === 'free' ? p.offer === 'free' || p.offer === 'credits'
      : filter === 'local' ? p.offer === 'local' : p.wire === 'anthropic')));
  return <div className="preset-picker">
    <div className="preset-tools">
      <span className="search-field">
        <Search size={13} aria-hidden/>
        <input type="search" placeholder={t('Search providers')} value={query} onChange={e => setQuery(e.target.value)}
          aria-label={t('Search providers')} autoFocus/>
      </span>
      <Segmented<PresetFilter> label={t('Show')} value={filter} onChange={setFilter} options={[
        {value: 'all', label: t('All')}, {value: 'free', label: t('Free or credits')},
        {value: 'local', label: t('Local')}, {value: 'anthropic', label: t('Anthropic API')}]}/>
    </div>
    {shown.length === 0 && <p className="secondary preset-empty">{t('No provider matches. Custom (OpenAI) connects any compatible endpoint.')}</p>}
    {OFFER_GROUPS.map(group => {
      const members = shown.filter(p => p.offer === group.offer);
      if (members.length === 0) return null;
      return <section key={group.offer} className="preset-group" aria-label={group.title()}>
        <h3>{group.title()}<span className="secondary"> · {group.hint()}</span></h3>
        <div className="preset-grid" role="radiogroup" aria-label={group.title()}>
          {members.map(p => <button key={p.id} type="button" role="radio" aria-checked={p.id === selected}
            className={p.id === selected ? 'preset selected' : 'preset'} onClick={() => onSelect(p.id)}
            title={p.offerNote ?? undefined}>
            <ProviderLogo kind={p.id} name={p.name}/>
            <span className="preset-text">
              <span className="preset-name">{presetName(p)}</span>
              {(p.offerNote || p.wire === 'anthropic') && <small className="secondary">
                {[p.wire === 'anthropic' ? t('Anthropic API') : null, p.offerNote].filter(Boolean).join(' · ')}</small>}
            </span>
          </button>)}
        </div>
      </section>;
    })}
  </div>;
}

function ConnectSheet({onClose, onConnected}: {onClose: () => void; onConnected: (id: string) => void}) {
  const presets = useGateway(api => api.presets());
  const [presetId, setPresetId] = useState('gemini');
  const preset: Preset | undefined = presets.data?.find(p => p.id === presetId);
  const [label, setLabel] = useState('');
  const [endpoint, setEndpoint] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [billing, setBilling] = useState<Billing | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const effectiveBilling = billing ?? preset?.defaultBilling ?? 'unknown';
  const custom = presetId === 'custom' || presetId === 'custom-anthropic';
  const keyRequired = preset?.needsKey ?? false;
  const canConnect = !!preset && (!keyRequired || apiKey.trim().length > 0) && (!custom || endpoint.trim().length > 0);

  const canSignIn = preset?.id === 'openrouter';
  const connect = async (signIn = false) => {
    if (!preset) return;
    setBusy(true);
    setError(null);
    try {
      const connection = {preset: preset.id, label: label.trim() || preset.name, endpoint: endpoint.trim() || undefined,
        apiKey: signIn ? undefined : apiKey.trim() || undefined, billing: effectiveBilling};
      const added = await call(api => signIn ? api.signInConnection(connection) : api.addConnection(connection));
      // Connecting checks the key by reading the model list; a failure still keeps the connection.
      try {
        const r = await call(api => api.refreshModels(added.id));
        checks.set(added.id, {at: Date.now(), ok: true, text: t('Reachable · {n} models listed', {n: r.listed})});
      } catch (e) {
        checks.set(added.id, {at: Date.now(), ok: false, text: errorText(e)});
      }
      gatewayChanged();
      onConnected(added.id);
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return <Sheet title={t('Connect Provider')} subtitle={t('Connecting checks the key by reading the model list. No request is billed.')}
    onClose={onClose} busy={busy} footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={() => connect()} disabled={busy || !canConnect}>
        {busy && <Spinner/>}{busy ? t('Connecting…') : t('Connect Provider')}</button>
    </>}>
    <PresetPicker presets={presets.data ?? []} selected={presetId}
      onSelect={id => { setPresetId(id); setBilling(null); setEndpoint(''); }}/>
    {preset && <div className="form">
      <FormRow label={t('Name')} htmlFor="connection-label" hint={t('Also the first part of model names, e.g. {id}/model.',
        {id: (label.trim() || preset.name).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'connection'})}>
        <input id="connection-label" className="text-input" value={label} placeholder={preset.id === 'custom' ? t('My Provider') : preset.name}
          onChange={e => setLabel(e.target.value)}/>
      </FormRow>
      <FormRow label={t('Endpoint')} htmlFor="connection-endpoint"
        hint={presetId === 'custom-anthropic' ? t('The base URL before /messages, e.g. https://api.example.com/v1')
          : custom ? t('The base URL before /chat/completions, e.g. https://api.example.com/v1')
          : preset.defaultBilling === 'local' ? t('Change it if the server runs on another address or port.')
          : t('Leave empty for the provider’s standard endpoint. Change it if your plan has its own, such as a token plan or a regional endpoint.')}>
        <input id="connection-endpoint" className="text-input mono" value={endpoint} placeholder={preset.endpoint ?? 'https://…/v1'}
          onChange={e => setEndpoint(e.target.value)} spellCheck={false}/>
      </FormRow>
      {preset.id !== 'ollama' && <FormRow label={t('API Key')} htmlFor="connection-key"
        hint={t('Stored in {store}. Pygg never shows it again.', {store: STORE})}>
        <input id="connection-key" className="text-input mono" type="password" value={apiKey} autoComplete="off" spellCheck={false}
          placeholder={keyRequired ? t('Required') : t('Optional')} onChange={e => setApiKey(e.target.value)}/>
        {canSignIn && <div className="sign-in-row">
          <button type="button" className="button" disabled={busy} onClick={() => connect(true)}>{t('Sign In with OpenRouter…')}</button>
          <small className="secondary">{t('Opens openrouter.ai in your browser. After you sign in and allow it, OpenRouter issues a key for Pygg, kept in {store}; you don’t need to copy one.', {store: STORE})}</small>
        </div>}
      </FormRow>}
      <FormRow label={t('Billing')} htmlFor="connection-billing" hint={billingHint(effectiveBilling, preset?.freeModels)}>
        <select id="connection-billing" className="popup-button" value={effectiveBilling} onChange={e => setBilling(e.target.value as Billing)}>
          {BILLINGS.map(b => <option key={b} value={b}>{billingLabel(b)}</option>)}
        </select>
      </FormRow>
    </div>}
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

function KeySheet({connection: c, onClose}: {connection: Connection; onClose: () => void}) {
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const save = () => {
    setBusy(true);
    // The last check was of the old key; until the new one is checked, nothing is known about it.
    call(api => api.setConnectionKey(c.id, key)).then(() => { checks.delete(c.id); gatewayChanged(); onClose(); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  return <Sheet title={c.hasKey ? t('Replace Key for {name}', {name: c.label}) : t('Add Key for {name}', {name: c.label})}
    subtitle={t('The new key replaces the old one in {store} right away.', {store: STORE})} onClose={onClose} busy={busy} className="narrow"
    footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={save} disabled={busy || !key.trim()}>{t('Save Key')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('API Key')} htmlFor="replace-key">
        <input id="replace-key" className="text-input mono" type="password" value={key} autoComplete="off" spellCheck={false}
          onChange={e => setKey(e.target.value)} autoFocus/>
      </FormRow>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

function AddModelSheet({connection: c, onClose, onAdded}: {connection: Connection; onClose: () => void; onAdded: () => void}) {
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const add = () => {
    setBusy(true);
    call(api => api.addModel(c.id, name)).then(() => { onAdded(); onClose(); }, e => { setError(errorText(e)); setBusy(false); });
  };
  return <Sheet title={t('Add Model to {name}', {name: c.label})}
    subtitle={t('For models the provider doesn’t list. Use the exact name the provider expects.')} onClose={onClose} busy={busy} className="narrow"
    footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={add} disabled={busy || !name.trim()}>{t('Add Model')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Model')} htmlFor="model-name" hint={name.trim() ? t('Clients will ask for {route}', {route: `${c.id}/${name.trim()}`}) : undefined}>
        <input id="model-name" className="text-input mono" value={name} onChange={e => setName(e.target.value)} spellCheck={false} autoFocus
          onKeyDown={e => { if (e.key === 'Enter' && name.trim()) add(); }}/>
      </FormRow>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

function PriceSheet({model: m, onClose, onSaved}: {model: Model; onClose: () => void; onSaved: () => void}) {
  const dollars = (micro: number | null | undefined) => micro == null ? '' : String(micro / 1_000_000);
  const [input, setInput] = useState(dollars(m.price?.inputMicroPerMtok));
  const [output, setOutput] = useState(dollars(m.price?.outputMicroPerMtok));
  const [cached, setCached] = useState(dollars(m.price?.cachedInputMicroPerMtok));
  const [written, setWritten] = useState(dollars(m.price?.cacheWriteMicroPerMtok));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const values = [parseUsd(input), parseUsd(output), parseUsd(cached), parseUsd(written)];
  const valid = values[0] !== null && values[1] !== null && values.every(v => v === null || Number.isFinite(v));
  const save = (clear: boolean) => {
    setBusy(true);
    const change = clear ? {clearPrice: true}
      : {price: {inputUsdPerMtok: values[0]!, outputUsdPerMtok: values[1]!, cachedInputUsdPerMtok: values[2],
          cacheWriteUsdPerMtok: values[3]}};
    call(api => api.updateModel(m.providerId, m.modelId, change)).then(() => { onSaved(); onClose(); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  return <Sheet title={t('Price for {model}', {model: m.modelId})}
    subtitle={t('In US dollars per million tokens, from the provider’s price list. Costs are then estimated from the tokens each response reports.')}
    onClose={onClose} busy={busy} className="narrow" footer={<>
      {m.price && <button type="button" className="button" onClick={() => save(true)} disabled={busy}>{t('Clear Price')}</button>}
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={() => save(false)} disabled={busy || !valid}>{t('Save Price')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Input')} htmlFor="price-in"><MoneyInput id="price-in" value={input} onChange={setInput} autoFocus/></FormRow>
      <FormRow label={t('Output')} htmlFor="price-out"><MoneyInput id="price-out" value={output} onChange={setOutput}/></FormRow>
      <FormRow label={t('Cached input')} htmlFor="price-cached" hint={t('Optional. Without it, cached tokens are priced as input.')}>
        <MoneyInput id="price-cached" value={cached} onChange={setCached}/>
      </FormRow>
      <FormRow label={t('Cache write')} htmlFor="price-write" hint={t('Optional. Anthropic charges extra to write the cache; without it, writes are priced as input and the ledger says so.')}>
        <MoneyInput id="price-write" value={written} onChange={setWritten}/>
      </FormRow>
    </div>
    <p className="sheet-text secondary">{t('Providers that report the charge themselves, such as OpenRouter, are recorded as Actual and don’t need a price.')}</p>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

/** A short summary of what auto routing knows about a model. */
function capsText(m: Model): string {
  const c = m.capabilities;
  if (!c.tier && !c.contextTokens) return t('Not set');
  const parts = [c.tier ? (c.tierSource?.startsWith('curated:') ? t('{tier} (Pygg' + String.fromCharCode(0x2019) + 's table)', {tier: c.tier}) : c.tier) : t('no tier')];
  if (c.contextTokens) parts.push(t('{n}k context', {n: Math.round(c.contextTokens / 1000)}));
  if (c.supportsTools) parts.push(t('tools'));
  if (c.supportsVision) parts.push(t('images'));
  if (c.thinkingTypes?.includes('adaptive')) parts.push(t('adaptive thinking'));
  return parts.join(' · ');
}

type TriState = 'unknown' | 'yes' | 'no';
const tri = (v: boolean | null): TriState => v === null ? 'unknown' : v ? 'yes' : 'no';
const untri = (v: TriState): boolean | null => v === 'unknown' ? null : v === 'yes';

/** What auto routing needs to know about a model (ROADMAP P0-5). Unknown counts as missing. */
function CapsSheet({model: m, onClose, onSaved}: {model: Model; onClose: () => void; onSaved: () => void}) {
  const c = m.capabilities;
  const [tier, setTier] = useState<Tier | ''>(c.tier ?? '');
  const [context, setContext] = useState(c.contextTokens ? String(c.contextTokens) : '');
  const [maxOutput, setMaxOutput] = useState(c.maxOutputTokens ? String(c.maxOutputTokens) : '');
  const [tools, setTools] = useState<TriState>(tri(c.supportsTools));
  const [vision, setVision] = useState<TriState>(tri(c.supportsVision));
  const [adaptive, setAdaptive] = useState<TriState>(c.thinkingTypes === null ? 'unknown' : c.thinkingTypes.includes('adaptive') ? 'yes' : 'no');
  const [effort, setEffort] = useState(c.effortLevels === null ? '' : c.effortLevels.length === 0 ? 'none' : c.effortLevels.join(', '));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const count = (v: string) => v.trim() === '' ? null : /^\d+$/.test(v.trim()) && Number(v) > 0 ? Number(v) : NaN;
  const valid = [count(context), count(maxOutput)].every(v => v === null || Number.isFinite(v));
  const save = () => {
    setBusy(true);
    call(api => api.setModelCapabilities(m.providerId, m.modelId, {tier: tier || null, contextTokens: count(context),
      maxOutputTokens: count(maxOutput), supportsTools: untri(tools), supportsVision: untri(vision),
      // `enabled` (a thinking budget) is kept as the provider said; only adaptive is set here.
      thinkingTypes: adaptive === 'unknown' ? null : [...(c.thinkingTypes ?? []).filter(x => x !== 'adaptive'), ...(adaptive === 'yes' ? ['adaptive'] : [])],
      effortLevels: effort.trim() === '' ? null : effort.trim() === 'none' ? [] : effort.split(/[\s,]+/).filter(Boolean),
      tierSource: 'user', tierNote: null, source: 'user', observedAt: null}))
      .then(() => { onSaved(); onClose(); }, e => { setError(errorText(e)); setBusy(false); });
  };
  const sourceText = c.source === null ? null : c.source === 'user'
    ? t('Set by you {when}.', {when: c.observedAt ? ago(c.observedAt) : ''})
    : t('Read from the provider’s model list ({source}) {when}. Saving here makes them yours, and a refresh no longer changes them.',
      {source: c.source.replace(/^provider: /, ''), when: c.observedAt ? ago(c.observedAt) : ''});
  return <Sheet title={t('Auto Routing for {model}', {model: m.modelId})}
    subtitle={t('auto chooses a model only for what it is known to support. Leave a field empty when you don’t know it.')}
    onClose={onClose} busy={busy} className="narrow" footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={save} disabled={busy || !valid}>{t('Save')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Tier')} htmlFor="caps-tier"
        hint={c.tierSource?.startsWith('curated:') && c.tierNote
          ? t('From Pygg' + String.fromCharCode(0x2019) + 's table: {why}. Saving here makes the tier yours, and the table no longer changes it.', {why: c.tierNote})
          : t('T0 local, T1 small and fast, T2 capable at low cost, T3 strong coding and reasoning, T4 frontier. Your judgment; Pygg' + String.fromCharCode(0x2019) + 's table only fills tiers nobody set.')}>
        <select id="caps-tier" className="popup-button" value={tier} onChange={e => setTier(e.target.value as Tier | '')}>
          <option value="">{t('Not set (auto skips it)')}</option>
          {TIERS.map(x => <option key={x} value={x}>{x}</option>)}
        </select>
      </FormRow>
      <FormRow label={t('Context')} htmlFor="caps-context" hint={t('Prompt and answer together, in tokens.')}>
        <input id="caps-context" className="text-input narrow-input" inputMode="numeric" value={context} placeholder={t('Unknown')}
          onChange={e => setContext(e.target.value)} aria-invalid={!Number.isFinite(count(context) ?? 0)}/>
      </FormRow>
      <FormRow label={t('Maximum output')} htmlFor="caps-output" hint={t('Needed when a request sets its own output limit.')}>
        <input id="caps-output" className="text-input narrow-input" inputMode="numeric" value={maxOutput} placeholder={t('Unknown')}
          onChange={e => setMaxOutput(e.target.value)} aria-invalid={!Number.isFinite(count(maxOutput) ?? 0)}/>
      </FormRow>
      <FormRow label={t('Tool calls')} htmlFor="caps-tools">
        <select id="caps-tools" className="popup-button" value={tools} onChange={e => setTools(e.target.value as TriState)}>
          <option value="unknown">{t('Unknown')}</option><option value="yes">{t('Supported')}</option><option value="no">{t('Not supported')}</option>
        </select>
      </FormRow>
      <FormRow label={t('Images')} htmlFor="caps-vision">
        <select id="caps-vision" className="popup-button" value={vision} onChange={e => setVision(e.target.value as TriState)}>
          <option value="unknown">{t('Unknown')}</option><option value="yes">{t('Supported')}</option><option value="no">{t('Not supported')}</option>
        </select>
      </FormRow>
      <FormRow label={t('Adaptive thinking')} htmlFor="caps-adaptive"
        hint={t('Anthropic API only. Claude Code asks for it; a model that does not take it answers 400, so auto skips it unless this says Supported.')}>
        <select id="caps-adaptive" className="popup-button" value={adaptive} onChange={e => setAdaptive(e.target.value as TriState)}>
          <option value="unknown">{t('Unknown')}</option><option value="yes">{t('Supported')}</option><option value="no">{t('Not supported')}</option>
        </select>
      </FormRow>
      <FormRow label={t('Effort levels')} htmlFor="caps-effort"
        hint={t('Anthropic API only: the levels it takes, e.g. low, medium, high, xhigh, max; “none” when it takes no effort setting. Empty is unknown.')}>
        <input id="caps-effort" className="text-input" value={effort} placeholder={t('Unknown')} onChange={e => setEffort(e.target.value)}/>
      </FormRow>
    </div>
    {sourceText && <p className="sheet-text secondary">{sourceText}</p>}
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

function MoneyInput({id, value, onChange, autoFocus}: {id: string; value: string; onChange: (v: string) => void; autoFocus?: boolean}) {
  const invalid = value.trim() !== '' && !Number.isFinite(parseUsd(value) ?? NaN);
  return <span className={invalid ? 'money-input invalid' : 'money-input'}>
    <span aria-hidden>$</span>
    <input id={id} inputMode="decimal" value={value} onChange={e => onChange(e.target.value)} autoFocus={autoFocus}
      aria-invalid={invalid} placeholder="0.00"/>
    <span className="secondary" aria-hidden>{t('/ 1M tokens')}</span>
  </span>;
}

type Terms = 'no_training' | 'may_train' | 'unknown';

/**
 * What the provider's terms say about training on what is sent (review GW-10): the user's record, with where they
 * read it. A client limited to providers that do not train on its data reaches only connections recorded so.
 */
function TermsRow({connection: c}: {connection: Connection}) {
  const [terms, setTerms] = useState<Terms>((c.dataUseTerms as Terms) ?? 'unknown');
  const [source, setSource] = useState(c.termsSource ?? '');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const changed = terms !== c.dataUseTerms || source.trim() !== (c.termsSource ?? '');
  const save = () => {
    setBusy(true);
    setError(null);
    call(api => api.setConnectionTerms(c.id, terms, source.trim() || null))
      .then(() => { setBusy(false); gatewayChanged(); }, e => { setError(errorText(e)); setBusy(false); });
  };
  return <div className="group-row stacked terms-row">
    <span className="group-label">{t('Data use')}
      <small>{c.dataUseTerms === 'unknown'
        ? t('Not recorded: clients that require providers not to train on their data skip this connection. Check the provider’s terms and record them here.')
        : t('Recorded by you {when}. Pygg does not check it; a provider can change its terms.', {when: c.termsCheckedAt ? ago(c.termsCheckedAt) : ''})}</small></span>
    <span className="group-control terms-control">
      <select className="popup-button" value={terms} onChange={e => setTerms(e.target.value as Terms)} aria-label={t('Data use')} disabled={busy}>
        <option value="unknown">{t('Not recorded')}</option>
        <option value="no_training">{t('Does not train on requests')}</option>
        <option value="may_train">{t('May train on requests')}</option>
      </select>
      {terms !== 'unknown' && <input className="text-input" value={source} onChange={e => setSource(e.target.value)} disabled={busy}
        placeholder={t('Where the terms say so: a link, or the page and date')} aria-label={t('Source')}/>}
      {changed && <button type="button" className="button" onClick={save} disabled={busy}>{t('Save')}</button>}
    </span>
    {error && <ErrorNote>{error}</ErrorNote>}
  </div>;
}