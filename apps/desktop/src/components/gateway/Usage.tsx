import { useMemo, useRef, useState } from 'react';
import { ArrowDown, ChartColumn, ChevronRight, FileDown } from 'lucide-react';
import { call, errorText, type CostStatus, type Outcome, type TaskQuality, type UsageEvent, type UsageGroup } from '../../lib/api';
import { locale, plural, t } from '../../lib/i18n';
import { costText, microUsd, sourceText, tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { Badge, Empty, ErrorNote, Segmented, Spinner } from '../ui';
import { CostOverTime } from './CostOverTime';

type Range = '1' | '7' | '30';

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
export function Usage() {
  const [range, setRange] = useState<Range>('7');
  const usage = useGateway(api => api.usage(Number(range)), [range], 15_000);
  const quality = useGateway(api => api.taskQuality(Number(range)), [range], 15_000);
  const overTime = useGateway(api => api.usageOverTime(Number(range)), [range], 15_000);
  const [show, setShow] = useState<'all' | 'problems' | 'unknown'>('all');
  const [client, setClient] = useState('');
  const [model, setModel] = useState('');
  const [openId, setOpenId] = useState<string | null>(null);
  const recent = usage.data?.recent ?? [];
  const rows = useMemo(() => recent.filter(e =>
    (show === 'all' || (show === 'unknown' ? e.costStatus === 'unknown' : e.outcome !== 'ok'))
    && (!client || (e.client ?? '') === client) && (!model || `${e.provider}/${e.model}` === model)), [recent, show, client, model]);
  const clients = useMemo(() => [...new Set(recent.map(e => e.client ?? ''))].sort(), [recent]);
  const models = useMemo(() => [...new Set(recent.map(e => `${e.provider}/${e.model}`))].sort(), [recent]);
  const requestsRef = useRef<HTMLElement>(null);
  const [exporting, setExporting] = useState(false);
  const [exported, setExported] = useState<{ok: boolean; text: string} | null>(null);
  const exportCsv = () => {
    setExporting(true);
    setExported(null);
    call(api => api.exportUsage(Number(range))).then(path => path && setExported({ok: true, text: t('Saved to {path}', {path})}),
      e => setExported({ok: false, text: errorText(e)})).finally(() => setExporting(false));
  };
  const toRequests = () => requestsRef.current?.scrollIntoView({behavior: 'smooth', block: 'start'});
  // A row of the summaries opens the requests behind it, as request logs in other gateways do.
  const pick = (set: (v: string) => void, value: string) => { set(value); setOpenId(null); setTimeout(toRequests, 0); };

  const bar = <div className="page-bar">
    <Segmented label={t('Period')} value={range} onChange={setRange} options={[
      {value: '1', label: t('24 Hours')}, {value: '7', label: t('7 Days')}, {value: '30', label: t('30 Days')}]}/>
    <span className="spacer"/>
    {(usage.data?.total.requests ?? 0) > 0 && <button type="button" className="button small" disabled={exporting}
      onClick={exportCsv}><FileDown size={13} aria-hidden/>{t('Export CSV…')}</button>}
    {recent.length > 0 && <button type="button" className="link-button" onClick={toRequests}>
      {t('Requests ({n})', {n: recent.length})}<ArrowDown size={12} aria-hidden/></button>}
  </div>;
  if (usage.loading && !usage.data) return <div className="page">{bar}<Spinner label={t('Loading usage…')}/></div>;
  const data = usage.data;
  return <div className="page">
    {bar}
    {usage.error && <ErrorNote>{usage.error}</ErrorNote>}
    {exported && (exported.ok ? <p className="group-footnote" role="status">{exported.text}</p> : <ErrorNote>{exported.text}</ErrorNote>)}
    {quality.data && quality.data.tasks > 0 && <TaskResults q={quality.data}/>}
    {data && data.total.requests === 0 && <Empty icon={<ChartColumn size={40} strokeWidth={1.4}/>} title={t('No Usage Recorded Yet')}>
      {t('Point a client at the gateway and its requests appear here, each with its cost and where that figure comes from.')}
    </Empty>}
    {data && data.total.requests > 0 && <>
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
          <p className="group-footnote">{t('Unknown costs are not in the total. Give a model a price in Connections to estimate them.')}</p>
        </div>
      </section>

      {overTime.data && <CostOverTime days={Number(range)} buckets={overTime.data}/>}

      <div className="two-columns">
        <GroupTable title={t('By Model')} column={t('Model')} groups={data.byModel} mono onPick={key => pick(setModel, key)}/>
        <GroupTable title={t('By Client')} column={t('Client')} groups={data.byClient} onPick={key => pick(setClient, key)}/>
      </div>

      <section className="group" ref={requestsRef}>
        <div className="group-heading">
          <h2>{t('Requests')}</h2>
          <span className="heading-actions">
            <select className="popup-button" value={client} onChange={e => setClient(e.target.value)} aria-label={t('Client')}>
              <option value="">{t('Every client')}</option>
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
            {(client || model || show !== 'all') && <button type="button" className="link-button"
              onClick={() => { setClient(''); setModel(''); setShow('all'); }}>{t('Clear Filters')}</button>}
          </span>
        </div>
        <div className="group-box table-box">
          <table className="data-table ledger">
            <thead><tr>
              <th scope="col">{t('Time')}</th><th scope="col">{t('Client')}</th><th scope="col">{t('Model')}</th>
              <th scope="col" className="num">{t('Tokens in → out')}</th><th scope="col" className="num">{t('Cost')}</th>
              <th scope="col">{t('Source')}</th><th scope="col" className="num">{t('Time taken')}</th><th scope="col">{t('Result')}</th>
            </tr></thead>
            <tbody>
              {rows.map(e => <LedgerRow key={e.id} event={e} open={openId === e.id} onToggle={() => setOpenId(openId === e.id ? null : e.id)}/>)}
              {rows.length === 0 && <tr><td colSpan={8} className="table-empty">{t('Nothing to show for this filter.')}</td></tr>}
            </tbody>
          </table>
        </div>
        <p className="group-footnote">{t('Shows up to 200 of the most recent requests in this period. Failed attempts before a fallback are listed too. A request the provider refused before answering is usually not billed; one cut off after it started may have been, so its cost shows as unknown or estimated.')}</p>
      </section>
    </>}
  </div>;
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
      <table className="data-table">
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

function LedgerRow({event: e, open, onToggle}: {event: UsageEvent; open: boolean; onToggle: () => void}) {
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
      <td><OutcomeBadge outcome={e.outcome} status={e.httpStatus}/></td>
    </tr>
    {open && <tr className="ledger-detail"><td colSpan={8}>
      <dl>
        <dt>{t('Asked for')}</dt><dd className="mono">{e.requestedModel ?? '—'}</dd>
        <dt>{t('Answered by')}</dt><dd className="mono">{e.provider}/{e.model}</dd>
        <dt>{t('Cost source')}</dt><dd>{statusLabel(e.costStatus)} · {sourceText(e.costSource)}</dd>
        {e.atRiskMicroUsd ? <><dt>{t('Counted against the limit')}</dt>
          <dd>{t('{amount}, the amount held for it: the provider may have charged for it', {amount: microUsd(e.atRiskMicroUsd)})}</dd></> : null}
        {(e.cacheTokens || e.cacheWriteTokens) ? <><dt>{t('Prompt cache')}</dt>
          <dd>{t('{read} read · {write} written', {read: tokens(e.cacheTokens ?? 0), write: tokens(e.cacheWriteTokens ?? 0)})}</dd></> : null}
        {e.routeMode && <><dt>{t('Route')}</dt><dd>{e.routeMode === 'native' ? t('Passed through in the same format')
          : t('Translated ({mode})', {mode: e.routeMode.replace('translated:', '')})}</dd></>}
        {e.routeReason && <><dt>{t('Why this model')}</dt><dd>{e.routeReason}</dd></>}
        {e.sessionKey && <><dt>{t('Session')}</dt><dd className="mono">{e.sessionKey}</dd></>}
        <dt>{t('Time taken')}</dt><dd>{e.latencyMs != null ? `${(e.latencyMs / 1000).toFixed(1)} s` : '—'}</dd>
        {e.error && <><dt>{t('Error')}</dt><dd className="selectable">{e.error}</dd></>}
      </dl>
    </td></tr>}
  </>;
}
