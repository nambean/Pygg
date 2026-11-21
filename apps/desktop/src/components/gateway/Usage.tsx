import { useEffect, useMemo, useState } from 'react';
import { CalendarClock, ChartColumn, ChevronRight, Cpu, FileDown } from 'lucide-react';
import { call, errorText, type CostStatus, type LocalUsage, type Outcome, type SubscriptionUsage, type TaskQuality, type UsageEvent,
  type RequestFilter, type UsageGroup } from '../../lib/api';
import { ago, windowText } from '../../lib/format';
import { locale, plural, t } from '../../lib/i18n';
import { costText, microUsd, sourceText, tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { Badge, Empty, ErrorNote, Segmented, Spinner } from '../ui';
import { UsageAlerts } from './UsageAlerts';
import { RepeatedOutput } from './RepeatedOutput';
import { ClearingShadow } from './ClearingShadow';
import { CacheLifetime } from './CacheLifetime';
import { CostOverTime } from './CostOverTime';
import { Savings, SavingsDetails, useSavings } from './Savings';
import { TokenMix, cacheMissLabel, cacheMissText } from './TokenMix';
import { Routing } from './Routing';

type Range = '1' | '7' | '30';
/** Requests shown at a time: the summary's first page, and each Show More. */
const PAGE = 200;
/** The money ledger, plans paid by the month, and models on this computer (SCREENS.md §5.6). */
type View = 'api' | 'subscriptions' | 'local';
/** API Costs as a summary or as its requests (Codex's third review A4 and fifth discussion): two views, not one long scroll. */
type Part = 'summary' | 'requests';

export function OutcomeBadge({outcome, status}: {outcome: Outcome; status: number | null}) {
  const code = status && status !== 200 ? ` · ${status}` : '';
  switch (outcome) {
    case 'ok': return <Badge tone="ok">{t('OK')}</Badge>;
    case 'fallback': return <Badge tone="warn" title={t('Failed; the next model of the combo was tried')}>{t('Fell back')}{code}</Badge>;
    case 'client_disconnected': return <Badge tone="off">{t('Client left')}</Badge>;
    case 'stream_error': return <Badge tone="error">{t('Cut off')}</Badge>;
    case 'interrupted': return <Badge tone="warn">{t('Interrupted')}</Badge>;
    default: return <Badge tone="error">{t('Failed')}{code}</Badge>;
  }
}

const statusLabel = (s: CostStatus) => ({actual: t('Actual'), estimated: t('Estimated'), unknown: t('Unknown')}[s]);

/** Usage (SCREENS.md §5.6): what went through the gateway, with actual, estimated and unknown costs kept apart. */
/** `requests:<client>` opens Requests filtered to that client; `failed:<client>` to its failures (Codex review 7). */
export function Usage({onOpenTools, intent, onIntentDone}: {onOpenTools?: () => void; intent?: string; onIntentDone?: () => void} = {}) {
  const [range, setRange] = useState<Range>('7');
  const [view, setView] = useState<View>('api');
  const [part, setPart] = useState<Part>('summary');
  const usage = useGateway(api => api.usage(Number(range)), [range], 15_000);
  const quality = useGateway(api => api.taskQuality(Number(range)), [range], 15_000);
  const overTime = useGateway(api => api.usageOverTime(Number(range)), [range], 15_000);
  const savings = useSavings(Number(range));
  const [show, setShow] = useState<'all' | 'problems' | 'unknown'>('all');
  // null is every client; '' the requests that came without one.
  const [client, setClient] = useState<string | null>(null);
  const [model, setModel] = useState('');
  const [openId, setOpenId] = useState<string | null>(null);
  useEffect(() => {
    if (!intent) return;
    const [kind, ...rest] = intent.split(':');
    // A drill-down starts clean: only the filter it names.
    setClient(rest.join(':'));
    setModel('');
    setShow(kind === 'failed' ? 'problems' : 'all');
    setOpenId(null);
    setPart('requests');
    onIntentDone?.();
  }, [intent, onIntentDone]);
  // The summary brings the first page of requests; Show More fetches the next pages, which the next refresh keeps.
  const [more, setMore] = useState<{range: Range; rows: UsageEvent[]}>({range, rows: []});
  const [loadingMore, setLoadingMore] = useState<string | null>(null);
  useEffect(() => setMore(m => m.range === range ? m : {range, rows: []}), [range]);
  const recent = useMemo(() => {
    const first = usage.data?.recent ?? [];
    if (more.range !== range || more.rows.length === 0) return first;
    const seen = new Set(first.map(e => e.id));
    return [...first, ...more.rows.filter(e => !seen.has(e.id))];
  }, [usage.data, more, range]);
  const total = usage.data?.total.requests ?? 0;
  const [paging, setPaging] = useState(false);
  const showMore = () => {
    if (paging) return;
    setLoadingMore(null);
    setPaging(true);
    const asked = range;
    call(api => api.usageRequests(Number(asked), recent.length, PAGE)).then(
      page => setMore(m => {
        // An answer for a period no longer shown is dropped; rows already listed are not listed twice.
        if (asked !== range) return m;
        const rows = m.range === asked ? m.rows : [];
        const seen = new Set(rows.map(e => e.id));
        return {range: asked, rows: [...rows, ...page.filter(e => !seen.has(e.id))]};
      }),
      e => setLoadingMore(errorText(e))).finally(() => setPaging(false));
  };
  // A filter searches the whole period in the store's query (Codex review 7), page by page; without one, the summary's
  // first page and Show More's.
  const filtering = show !== 'all' || client !== null || model !== '';
  const filterKey = JSON.stringify([range, client, model, show]);
  const [matched, setMatched] = useState<{key: string; rows: UsageEvent[]; total: number} | null>(null);
  const [matchError, setMatchError] = useState<string | null>(null);
  useEffect(() => {
    if (!filtering) return;
    let current = true;
    setMatchError(null);
    const filter: RequestFilter = {client, model: model || null, show: show === 'all' ? null : show};
    call(api => api.usageRequestsMatching(Number(range), filter, 0, PAGE)).then(
      page => { if (current) setMatched({key: filterKey, rows: page.rows, total: page.total}); },
      e => { if (current) setMatchError(errorText(e)); });
    return () => { current = false; };
  }, [filtering, filterKey, range, client, model, show, usage.data]);
  const rows = filtering ? (matched?.key === filterKey ? matched.rows : []) : recent;
  const rowsTotal = filtering ? (matched?.key === filterKey ? matched.total : 0) : total;
  const moreMatching = () => {
    if (paging || !matched) return;
    setPaging(true);
    const key = filterKey;
    const filter: RequestFilter = {client, model: model || null, show: show === 'all' ? null : show};
    call(api => api.usageRequestsMatching(Number(range), filter, matched.rows.length, PAGE)).then(
      page => setMatched(m => {
        if (!m || m.key !== key) return m;
        const seen = new Set(m.rows.map(e => e.id));
        return {...m, rows: [...m.rows, ...page.rows.filter(e => !seen.has(e.id))], total: page.total};
      }),
      e => setLoadingMore(errorText(e))).finally(() => setPaging(false));
  };
  // The choices come from the whole period's totals, not from the rows loaded.
  const clients = useMemo(() => (usage.data?.byClient ?? []).map(g => g.key).sort(), [usage.data]);
  const models = useMemo(() => (usage.data?.byModel ?? []).map(g => g.key).sort(), [usage.data]);
  const [exporting, setExporting] = useState(false);
  const [exported, setExported] = useState<{ok: boolean; text: string} | null>(null);
  const exportCsv = () => {
    setExporting(true);
    setExported(null);
    call(api => api.exportUsage(Number(range))).then(path => path && setExported({ok: true, text: t('Saved to {path}', {path})}),
      e => setExported({ok: false, text: errorText(e)})).finally(() => setExporting(false));
  };
  // A row of the summaries opens the requests behind it, as request logs in other gateways do: the Requests view, filtered.
  const pick = (set: (v: string) => void, value: string) => { set(value); setOpenId(null); setPart('requests'); };
  const EVERY = '\u0001every';

  const bar = <div className="page-bar">
    <Segmented label={t('Usage view')} value={view} onChange={setView} options={[
      {value: 'api', label: t('API Costs')}, {value: 'subscriptions', label: t('Subscriptions')}, {value: 'local', label: t('Local Runtime')}]}/>
    <Segmented label={t('Period')} value={range} onChange={setRange} options={[
      {value: '1', label: t('24 Hours')}, {value: '7', label: t('7 Days')}, {value: '30', label: t('30 Days')}]}/>
    <span className="spacer"/>
    {view === 'api' && (usage.data?.total.requests ?? 0) > 0 && <button type="button" className="button small" disabled={exporting}
      onClick={exportCsv}><FileDown size={13} aria-hidden/>{t('Export CSV…')}</button>}
  </div>;
  const parts = view === 'api' && total > 0 && <Segmented label={t('API costs view')} value={part} onChange={v => { setPart(v); setOpenId(null); }}
    options={[{value: 'summary', label: t('Summary')}, {value: 'requests', label: t('Requests ({n})', {n: total})}]}/>;
  if (view === 'subscriptions') return <div className="page data-page">{bar}<Subscriptions days={Number(range)}/></div>;
  if (view === 'local') return <div className="page data-page">{bar}<LocalRuntime days={Number(range)}/></div>;
  if (usage.loading && !usage.data) return <div className="page data-page">{bar}<Spinner label={t('Loading usage…')}/></div>;
  const data = usage.data;
  return <div className="page data-page">
    {bar}
    {usage.error && <ErrorNote>{usage.error}</ErrorNote>}
    <UsageAlerts/>
    {exported && (exported.ok ? <p className="group-footnote" role="status">{exported.text}</p> : <ErrorNote>{exported.text}</ErrorNote>)}
    {data && data.total.requests === 0 && <Empty icon={<ChartColumn size={40} strokeWidth={1.4}/>} title={t('No gateway requests in this period')}
      action={onOpenTools && <button type="button" className="button prominent" onClick={onOpenTools}>{t('Open CLI Tools')}</button>}>
      {t('Set up a coding tool, then send a request: it appears here with its model, its cost and where that figure comes from.')}
    </Empty>}
    {data && data.total.requests === 0 && quality.data && quality.data.tasks > 0 && <TaskResults q={quality.data}/>}
    {data && data.total.requests > 0 && <div className="page-bar">{parts}</div>}
    {data && data.total.requests > 0 && part === 'summary' && <>
      <Savings savings={savings} period={range === '1' ? t('last 24 hours') : t('last {n} days', {n: range})}/>
      <section className="group">
        <h2>{t('API Costs')}</h2>
        <div className="group-box totals">
          <div className="totals-main">
            <span className="totals-figure">{microUsd(data.total.knownMicroUsd)}</span>
            <span className="secondary">{t('known')}</span>
          </div>
          <div className="totals-parts">
            <span>{t('{amount} actual', {amount: microUsd(data.total.actualMicroUsd)})}</span>
            <span>{t('≈{amount} estimated', {amount: microUsd(data.total.knownMicroUsd - data.total.actualMicroUsd)})}</span>
            <span className={data.total.unknownRequests ? 'warn-text' : ''}>
              {plural(data.total.unknownRequests, '{n} request with unknown cost', '{n} requests with unknown cost')}</span>
          </div>
          <p className="group-footnote">{t('Unknown costs are not in the total. Give a model a price in Providers to estimate them.')}</p>
        </div>
      </section>

      <Coverage days={Number(range)} total={data.total} quality={quality.data ?? null} qualityError={quality.data ? null : quality.error ?? null}/>
      {overTime.data && <CostOverTime days={Number(range)} buckets={overTime.data}
        comparison={savings.data?.baseline && savings.data.baselinePriced ? {model: savings.data.baseline, periods: savings.data.periods} : null}/>}
      <TokenMix days={Number(range)}/>
      <Routing days={Number(range)} onReview={() => { setShow('problems'); setOpenId(null); setPart('requests'); }}/>
      <SavingsDetails savings={savings} onPickClient={key => pick(setClient, key)}/>
      <RepeatedOutput days={Number(range)}/>
      <ClearingShadow days={Number(range)}/>
      <CacheLifetime days={Number(range)}/>

      <div className="two-columns">
        <GroupTable title={t('By Model')} column={t('Model')} groups={data.byModel} mono onPick={key => pick(setModel, key)}/>
        <GroupTable title={t('By Client')} column={t('Client')} groups={data.byClient} onPick={key => pick(setClient, key)}/>
      </div>
      {quality.data && quality.data.tasks > 0 && <TaskResults q={quality.data}/>}
    </>}
    {data && data.total.requests > 0 && part === 'requests' && <>
      <section className="group">
        <div className="group-heading">
          <h2>{t('Requests')}</h2>
          <span className="heading-actions">
            <select className="popup-button" value={client ?? EVERY} onChange={e => setClient(e.target.value === EVERY ? null : e.target.value)}
              aria-label={t('Client')}>
              <option value={EVERY}>{t('Every client')}</option>
              {clients.map(c => <option key={c} value={c}>{c || t('No client')}</option>)}
            </select>
            <select className="popup-button" value={model} onChange={e => setModel(e.target.value)} aria-label={t('Model')}>
              <option value="">{t('Every model')}</option>
              {models.map(m => <option key={m} value={m}>{m}</option>)}
            </select>
            <select className="popup-button" value={show} onChange={e => setShow(e.target.value as typeof show)} aria-label={t('Show')}>
              <option value="all">{t('All requests')}</option>
              <option value="problems">{t('Fallbacks and failures')}</option>
              <option value="unknown">{t('Unknown cost')}</option>
            </select>
            {filtering && <button type="button" className="link-button"
              onClick={() => { setClient(null); setModel(''); setShow('all'); }}>{t('Clear Filters')}</button>}
          </span>
        </div>
        <div className="group-box table-box">
          <table className="data-table ledger" aria-label={t('Requests')}>
            <thead><tr>
              <th scope="col">{t('Time')}</th><th scope="col">{t('Client')}</th><th scope="col">{t('Model')}</th>
              <th scope="col" className="num">{t('Tokens in → out')}</th><th scope="col" className="num">{t('Cost')}</th>
              <th scope="col">{t('Source')}</th><th scope="col" className="num">{t('Time taken')}</th><th scope="col">{t('Result')}</th>
            </tr></thead>
            <tbody>
              {rows.map(e => <LedgerRow key={e.id} event={e} open={openId === e.id} onToggle={() => setOpenId(openId === e.id ? null : e.id)}
                chain={e.requestId ? recent.filter(o => o.requestId === e.requestId).sort((a, b) => (a.attempt ?? 1) - (b.attempt ?? 1)) : []}/>)}
              {rows.length === 0 && <tr><td colSpan={8} className="table-empty">{filtering && matched?.key !== filterKey && !matchError
                ? t('Searching the period…') : t('Nothing matches in this period.')}</td></tr>}
            </tbody>
          </table>
        </div>
        {matchError && <ErrorNote>{matchError}</ErrorNote>}
        {rows.length < rowsTotal && <button type="button" className="link-button table-more" onClick={filtering ? moreMatching : showMore}
          disabled={paging}>
          {paging ? t('Loading older attempts…') : t('Show {n} more', {n: Math.min(PAGE, rowsTotal - rows.length)})}</button>}
        {loadingMore && <ErrorNote>{loadingMore}</ErrorNote>}
        <p className="group-footnote">{filtering
          ? t('{n} of the period’s attempts match these filters; {shown} shown, newest first.', {n: rowsTotal, shown: rows.length})
          : rows.length < rowsTotal
          ? t('Showing the {shown} most recent of {total} requests in this period; Show More goes further back, and Export CSV has them all.', {shown: rows.length, total: rowsTotal})
          : t('Every request of this period is listed, newest first.')}{' '}
          {t('Failed attempts before a fallback are listed too. A request the provider refused before answering is usually not billed; one cut off after it started may have been, so its cost shows as unknown or estimated.')}</p>
      </section>
    </>}
  </div>;
}

const periodEmpty = () => t('Nothing in this period');

/** Usage → Subscriptions: plans paid by the month count requests against a quota, so no request has a price here. */
function Subscriptions({days}: {days: number}) {
  const subs = useGateway(api => api.usageSubscriptions(days), [days], 15_000);
  if (subs.loading && !subs.data) return <Spinner label={t('Loading usage…')}/>;
  if (subs.error && !subs.data) return <ErrorNote>{subs.error}</ErrorNote>;
  const list = subs.data ?? [];
  if (list.length === 0) return <Empty icon={<CalendarClock size={40} strokeWidth={1.4}/>} title={t('No Subscriptions')}>
    {t('Set a connection’s billing to Subscription in Providers, for a plan paid by the month. Its requests are counted here against the plan, not as money.')}
  </Empty>;
  return <section className="group">
    <h2>{t('Subscriptions')}</h2>
    <div className="group-box table-box">
      <table className="data-table" aria-label={t('Subscriptions')}>
        <thead><tr>
          <th scope="col">{t('Connection')}</th><th scope="col" className="num">{t('Requests')}</th>
          <th scope="col" className="num">{t('Tokens in → out')}</th><th scope="col">{t('Last used')}</th>
          <th scope="col">{t('Limits, as the provider last stated')}</th>
        </tr></thead>
        <tbody>{list.map(s => <SubscriptionRow key={s.providerId} s={s}/>)}</tbody>
      </table>
    </div>
    {subs.error && <ErrorNote>{subs.error}</ErrorNote>}
    <p className="group-footnote">{t('A subscription costs its monthly price, not a price per request, so these requests have no cost of their own; in API Costs they count as unknown. Limits are read from the headers of the provider’s last answer for each model. Pygg does not guess what is left: a provider that states nothing shows Unknown.')}</p>
  </section>;
}

function SubscriptionRow({s}: {s: SubscriptionUsage}) {
  const stated = s.limits.filter(l => (l.windows?.length ?? 0) > 0);
  const account = s.account?.freeRequests ? windowText(s.account.freeRequests) : null;
  return <tr>
    <td>{s.label}</td>
    <td className="num">{s.requests}{s.notOk > 0 && <small className="secondary"> · {t('{n} not OK', {n: s.notOk})}</small>}</td>
    <td className="num">{s.requests ? `${tokens(s.inputTokens)} → ${tokens(s.outputTokens)}` : '—'}</td>
    <td className="secondary nowrap">{s.lastUsedAt ? ago(s.lastUsedAt) : periodEmpty()}</td>
    <td>{stated.length || account ? <>
      {stated.map(l => <div key={l.modelId}>
        <span className="mono">{l.modelId}</span> <small className="secondary">· {ago(l.observedAt)}</small>
        <div>{l.windows!.map(windowText).join(' · ')}</div>
      </div>)}
      {account && <div>{t('Account')}: {account}</div>}
    </> : <span className="secondary" title={t('The provider has not stated a limit in its answers yet.')}>{t('Unknown')}</span>}</td>
  </tr>;
}

/** Usage → Local Runtime: models on this computer cost no API money, but use its time and power, which are not measured. */
function LocalRuntime({days}: {days: number}) {
  const local = useGateway(api => api.usageLocal(days), [days], 15_000);
  if (local.loading && !local.data) return <Spinner label={t('Loading usage…')}/>;
  if (local.error && !local.data) return <ErrorNote>{local.error}</ErrorNote>;
  const list = local.data ?? [];
  if (list.length === 0) return <Empty icon={<Cpu size={40} strokeWidth={1.4}/>} title={t('No Local Runtime')}>
    {t('Add Ollama, LM Studio or another server on this computer in Providers. The requests it answers are counted here.')}
  </Empty>;
  const models = list.flatMap(l => l.models);
  const requests = models.reduce((n, m) => n + m.requests, 0);
  const tokenSum = models.reduce((n, m) => n + m.inputTokens + m.outputTokens, 0);
  return <>
    <section className="group">
      <h2>{t('Local Runtime')}</h2>
      <div className="group-box totals">
        <div className="totals-main">
          <span className="totals-figure">{microUsd(0)}</span>
          <span className="secondary">{t('API cost')}</span>
        </div>
        <div className="totals-parts">
          <span>{plural(requests, '{n} request', '{n} requests')}</span>
          <span>{t('{n} tokens', {n: tokens(tokenSum)})}</span>
        </div>
        <p className="group-footnote">{t('No API money is spent on a local model. Electricity and the computer’s time are not measured, so they are not in any figure here.')}</p>
      </div>
    </section>
    <section className="group">
      <h2>{t('By Model')}</h2>
      <div className="group-box table-box">
        <table className="data-table" aria-label={t('Local models')}>
          <thead><tr>
            <th scope="col">{t('Connection')}</th><th scope="col">{t('Model')}</th><th scope="col" className="num">{t('Requests')}</th>
            <th scope="col" className="num">{t('Tokens in → out')}</th><th scope="col" className="num">{t('Median time')}</th>
            <th scope="col">{t('Last used')}</th>
          </tr></thead>
          <tbody>{list.map(l => <LocalRows key={l.providerId} l={l}/>)}</tbody>
        </table>
      </div>
      <p className="group-footnote">{t('Median time is of successful requests, from when the gateway sent the request until the answer ended. It includes loading the model when it was not in memory.')}</p>
    </section>
  </>;
}

function LocalRows({l}: {l: LocalUsage}) {
  if (l.models.length === 0) return <tr><td>{l.label}</td><td colSpan={5} className="secondary">{periodEmpty()}</td></tr>;
  return <>{l.models.map((m, i) => <tr key={m.model}>
    <td>{i === 0 ? l.label : ''}</td>
    <td className="mono" title={m.model}>{m.model}</td>
    <td className="num">{m.requests}{m.notOk > 0 && <small className="secondary"> · {t('{n} not OK', {n: m.notOk})}</small>}</td>
    <td className="num">{`${tokens(m.inputTokens)} → ${tokens(m.outputTokens)}`}</td>
    <td className="num">{m.medianLatencyMs != null ? `${(m.medianLatencyMs / 1000).toFixed(1)} s` : '—'}</td>
    <td className="secondary nowrap">{m.lastUsedAt ? ago(m.lastUsedAt) : '—'}</td>
  </tr>)}</>;
}

/** How tasks turned out (ROADMAP C4): what the user kept, and the known money behind each kept result. */
function TaskResults({q}: {q: TaskQuality}) {
  return <section className="group">
    <h2>{t('Task Results')}</h2>
    <div className="group-box totals">
      <div className="totals-main">
        <span className="totals-figure">{q.microUsdPerKept == null ? '—' : microUsd(q.microUsdPerKept)}</span>
        <span className="secondary">{t('known money per kept result')}</span>
      </div>
      <div className="totals-parts">
        <span>{t('{kept} of {tasks} kept', {kept: String(q.kept), tasks: String(q.tasks)})}</span>
        <span>{t('{passed} of {checked} attempts passed their checks', {passed: String(q.passedAttempts), checked: String(q.checkedAttempts)})}</span>
        {(q.discarded > 0 || q.retried > 0 || q.cancelled > 0) &&
          <span>{t('{d} discarded · {r} retried · {c} stopped', {d: String(q.discarded), r: String(q.retried), c: String(q.cancelled)})}</span>}
        {q.unknownEntries > 0 && <span className="warn-text">
          {plural(q.unknownEntries, '{n} agent run with unknown cost', '{n} agent runs with unknown cost')}</span>}
      </div>
      <p className="group-footnote">{t('Every paid step counts, kept or not, so a cheap step that failed is in the cost of the result that followed. Agents signed in with a subscription report no money, so their runs are not in the figure.')}</p>
    </div>
  </section>;
}

function GroupTable({title, column, groups, mono, onPick}: {title: string; column: string; groups: UsageGroup[]; mono?: boolean;
  onPick: (key: string) => void}) {
  const [all, setAll] = useState(false);
  const shown = all ? groups : groups.slice(0, 8);
  return <section className="group">
    <h2>{title}</h2>
    <div className="group-box table-box">
      <table className="data-table" aria-label={title}>
        <thead><tr>
          <th scope="col">{column}</th><th scope="col" className="num">{t('Requests')}</th>
          <th scope="col" className="num">{t('Tokens')}</th><th scope="col" className="num">{t('Known cost')}</th>
        </tr></thead>
        <tbody>
          {shown.map(g => <tr key={g.key}>
            <td className={mono ? 'mono' : ''} title={g.key}>
              <button type="button" className="row-link" onClick={() => onPick(g.key)}
                aria-label={t('Show the requests of {name}', {name: g.key || t('No client')})}>{g.key || t('No client')}</button></td>
            <td className="num">{g.requests}{g.fallbacks + g.failed > 0 && <small className="secondary"> · {t('{n} not OK', {n: g.fallbacks + g.failed})}</small>}</td>
            <td className="num">{tokens(g.inputTokens + g.outputTokens)}</td>
            <td className="num">{microUsd(g.knownMicroUsd)}{g.unknownRequests > 0 && <small className="warn-text"> + {t('{n} unknown', {n: g.unknownRequests})}</small>}</td>
          </tr>)}
        </tbody>
      </table>
    </div>
    {groups.length > 8 && <button type="button" className="link-button table-more" onClick={() => setAll(v => !v)}>
      {all ? t('Show the top 8') : t('Show all {n}', {n: groups.length})}</button>}
  </section>;
}

/** How a route was chosen (schema v30's `route_kind`), in words. */
function routeKindText(kind: string): string {
  const [base, session] = kind.split('+');
  const text = ({auto: t('auto (Pygg’s choice by tier)'), 'auto/cheap': t('auto/cheap (Pygg’s choice by cost)'),
    'auto/plan': t('auto/plan (Pygg’s choice by the facts for plan)'), 'auto/code': t('auto/code (Pygg’s choice by the facts for code)'),
    'auto/test': t('auto/test (Pygg’s choice by the facts for test)'), 'auto/review': t('auto/review (Pygg’s choice by the facts for review)'),
    combo: t('a combo, in its order'),
    'combo:cheapest': t('a combo, cheapest first'), pool: t('a pool of accounts'), named: t('a model by name')} as Record<string, string>)[base] ?? base;
  if (session === 'pinned') return t('{route}; the routing session’s model, kept', {route: text});
  if (session === 'home') return t('{route}; the session back on its first model after a fallback', {route: text});
  return text;
}

const cacheHintText = (hint: string) => ({added: t('Turned on by Pygg: Anthropic’s automatic caching, which this client cannot ask for'), preserved: t('Kept as the client sent them'), translated: t('Carried into the OpenAI format'),
  dropped: t('Dropped: this provider takes none in this format'), none: t('None sent')} as Record<string, string>)[hint] ?? hint;

function LedgerRow({event: e, open, onToggle, chain}: {event: UsageEvent; open: boolean; onToggle: () => void; chain: UsageEvent[]}) {
  const time = new Date(e.createdAt).toLocaleString(locale(), {month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'});
  return <>
    <tr className={open ? 'ledger-row open' : 'ledger-row'} onClick={onToggle}>
      <td className="secondary nowrap">
        <button type="button" className="row-toggle" aria-expanded={open} onClick={ev => { ev.stopPropagation(); onToggle(); }}>
          <ChevronRight size={12} aria-hidden className={open ? 'chevron open' : 'chevron'}/>{time}</button>
      </td>
      <td>{e.client ?? t('No client')}</td>
      <td className="mono" title={`${e.provider}/${e.model}`}>{e.provider}/{e.model}</td>
      <td className="num">{e.inputTokens != null ? `${tokens(e.inputTokens)} → ${tokens(e.outputTokens ?? 0)}` : '—'}</td>
      <td className={`num cost ${e.costStatus}`}>{costText(e.costMicroUsd, e.costStatus)}</td>
      <td className="secondary">{statusLabel(e.costStatus)}</td>
      <td className="num secondary">{e.latencyMs != null ? `${(e.latencyMs / 1000).toFixed(1)} s` : '—'}</td>
      <td><OutcomeBadge outcome={e.outcome} status={e.httpStatus}/>
        {e.outcome === 'ok' && (e.attempt ?? 1) > 1 && <small className="secondary"> {t('attempt {n}', {n: e.attempt ?? 1})}</small>}</td>
    </tr>
    {open && <tr className="ledger-detail"><td colSpan={8}>
      <dl>
        <dt>{t('Asked for')}</dt><dd className="mono">{e.requestedModel ?? '—'}</dd>
        <dt>{e.outcome === 'ok' ? t('Answered by') : t('Attempted model')}</dt><dd className="mono">{e.provider}/{e.model}</dd>
        <dt>{t('Cost source')}</dt><dd>{statusLabel(e.costStatus)} · {sourceText(e.costSource)}</dd>
        {e.atRiskMicroUsd ? <><dt>{t('Counted against the limit')}</dt>
          <dd>{t('{amount}, the amount held for it: the provider may have charged for it', {amount: microUsd(e.atRiskMicroUsd)})}</dd></> : null}
        {(e.cacheTokens || e.cacheWriteTokens) ? <><dt>{t('Prompt cache')}</dt>
          <dd>{t('{read} read · {write} written', {read: tokens(e.cacheTokens ?? 0), write: tokens(e.cacheWriteTokens ?? 0)})}</dd></> : null}
        {e.routeMode && <><dt>{t('Route')}</dt><dd>{e.routeMode === 'native' ? t('Passed through in the same format')
          : t('Translated ({mode})', {mode: e.routeMode.replace('translated:', '')})}</dd></>}
        {e.routeKind && <><dt>{t('How the route was chosen')}</dt><dd>{routeKindText(e.routeKind)}</dd></>}
        {e.keyLabel && <><dt>{t('Key')}</dt><dd>{t('{key}, of the connection’s keys', {key: e.keyLabel})}</dd></>}
        {e.routeReason && <><dt>{t('Why this model')}</dt><dd>{e.routeReason}</dd></>}
        {e.billing && <><dt>{t('Billing when sent')}</dt><dd>{e.billing}</dd></>}
        {e.cacheHint && <><dt>{t('Prompt-cache hints')}</dt><dd>{cacheHintText(e.cacheHint)}</dd></>}
        {e.cacheMiss && <><dt>{t('Cache missed')}</dt><dd>{cacheMissLabel(e.cacheMiss)}: {cacheMissText(e.cacheMiss)}</dd></>}
        {chain.length > 1 && <><dt>{t('Attempts of this request')}</dt><dd>
          <ol className="attempt-chain">{chain.map(a => <li key={a.id} className={a.id === e.id ? 'current' : ''}>
            <span className="mono">{a.provider}/{a.model}</span> <OutcomeBadge outcome={a.outcome} status={a.httpStatus}/></li>)}</ol></dd></>}
        {e.sessionKey && <><dt>{t('Routing session')}</dt><dd><span className="mono">{e.sessionKey}</span>{' · '}
          {e.sessionKey.startsWith('launch-') ? t('one launch of the tool through pygg run') : t('named by the tool or a task step')}</dd></>}
        <dt>{t('Time taken')}</dt><dd>{e.latencyMs != null ? `${(e.latencyMs / 1000).toFixed(1)} s` : '—'}</dd>
        {e.error && <><dt>{t('Error')}</dt><dd className="selectable">{e.error}</dd></>}
      </dl>
    </td></tr>}
  </>;
}

/**
 * What Pygg can see (the gap study's coverage-aware usage): requests through the gateway with a price, those without
 * one, agent runs that reported no cost, subscriptions known only as quota, and what never passes through Pygg.
 */
function Coverage({days, total, quality, qualityError}: {days: number; total: UsageGroup; quality: TaskQuality | null; qualityError: string | null}) {
  const subs = useGateway(api => api.usageSubscriptions(days), [days], 30_000);
  const priced = total.requests - total.unknownRequests;
  const subscriptions = subs.data ?? [];
  const stated = subscriptions.filter(x => x.limits.length > 0).length;
  const rows: {what: string; seen: string; tone: 'ok' | 'warn' | 'none'}[] = [
    {what: t('Requests through Pygg with a price'), seen: t('{n} requests · {amount}', {n: priced, amount: microUsd(total.knownMicroUsd)}), tone: 'ok'},
    {what: t('Requests through Pygg without a price'), seen: total.unknownRequests ? t('{n} requests: give their models a price in Providers', {n: total.unknownRequests}) : t('none'),
      tone: total.unknownRequests ? 'warn' : 'ok'},
    {what: t('Paid agent runs with no recorded cost'), seen: qualityError ? t('could not be read: {error}', {error: qualityError}) : !quality ? t('loading…')
      : (quality.unknownCostRuns ?? 0) > 0 ? t('{n} runs', {n: quality.unknownCostRuns ?? 0}) : t('none'),
      tone: qualityError || (quality?.unknownCostRuns ?? 0) > 0 ? 'warn' : 'ok'},
    {what: t('Subscriptions'), seen: subs.error && !subs.data ? t('could not be read: {error}', {error: subs.error}) : !subs.data ? t('loading…')
      : subscriptions.length === 0 ? t('none connected')
      : t('{n} connected · {m} state their limits; the rest are Unknown', {n: subscriptions.length, m: stated}),
      tone: subs.error && !subs.data ? 'warn' : subscriptions.length && stated < subscriptions.length ? 'warn' : 'ok'},
    {what: t('Tools not pointed at Pygg'), seen: t('not seen at all: their use and limits are in their own apps'), tone: 'none'},
  ];
  return <section className="group" aria-labelledby="coverage-heading">
    <h2 id="coverage-heading">{t('What Pygg sees')}</h2>
    <dl className="group-box coverage">
      {rows.map(r => <div key={r.what} className={`coverage-row ${r.tone}`}><dt>{r.what}</dt><dd>{r.seen}</dd></div>)}
    </dl>
    <p className="group-footnote">{t('Totals above count only the first row. Pygg never guesses a price or a quota a provider has not stated.')}</p>
  </section>;
}
