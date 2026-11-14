import { useMemo, useState } from 'react';
import type { SavingsPeriod, UsageBucket } from '../../lib/api';
import { locale, plural, t } from '../../lib/i18n';
import { microUsd, tokens } from '../../lib/money';
import { Segmented, rovingIndex } from '../ui';

/** What the bars measure: known cost, requests, tokens in and out (a switch 9router's usage chart has too), or the
 *  comparison: what the compared answers cost through Pygg against the same tokens at the comparison model. */
export type ChartMetric = 'cost' | 'requests' | 'tokens' | 'comparison';

/** A round top for the axis whose half is round too: 1, 2, 4, 6 or 8 times a power of ten, at least `floor` (two cents
 *  for money, so the labels never need more than two decimals; a handful for counts). */
function niceMax(value: number, floor: number): number {
  const at = Math.max(value, floor);
  const power = 10 ** Math.floor(Math.log10(at));
  return ([1, 2, 4, 6, 8, 10].find(m => m * power >= at) ?? 10) * power;
}

/** Every period of the range, oldest first, the empty ones included, on this computer's clock (as the backend). */
function periods(days: number, buckets: UsageBucket[]): UsageBucket[] {
  const size = days <= 1 ? 3_600_000 : 86_400_000;
  const offset = -new Date().getTimezoneOffset() * 60_000;
  const first = Math.floor((Date.now() - days * 86_400_000 + offset) / size) * size - offset;
  const byStart = new Map(buckets.map(b => [b.start, b]));
  const out: UsageBucket[] = [];
  for (let start = first; start <= Date.now(); start += size) {
    out.push(byStart.get(start) ?? {start, requests: 0, failed: 0, knownMicroUsd: 0, actualMicroUsd: 0, unknownRequests: 0,
      inputTokens: 0, outputTokens: 0});
  }
  return out;
}

/**
 * Usage per hour (24 hours) or per day, one series in the accent colour: known API cost, or, with the switch, requests
 * or tokens, from the same periods. Unknown costs are not in the cost bars; a period with requests of unknown cost
 * carries a warning marker, named in the legend and the tooltip.
 */
export function CostOverTime({days, buckets, comparison}: {days: number; buckets: UsageBucket[];
  /** The savings estimate's periods and its model, when one is chosen: the fourth metric. */
  comparison?: {model: string; periods: SavingsPeriod[]} | null}) {
  const rows = useMemo(() => periods(days, buckets), [days, buckets]);
  const compared = useMemo(() => new Map((comparison?.periods ?? []).map(p => [p.start, p])), [comparison]);
  const [hover, setHover] = useState<number | null>(null);
  const [metric, setMetric] = useState<ChartMetric>('cost');
  const hourly = days <= 1;
  const money = metric === 'cost' || metric === 'comparison';
  const at = (r: UsageBucket) => compared.get(r.start);
  const value = (r: UsageBucket) => metric === 'cost' ? r.knownMicroUsd : metric === 'requests' ? r.requests
    : metric === 'tokens' ? r.inputTokens + r.outputTokens : at(r)?.baselineMicroUsd ?? 0;
  const label = (n: number) => money ? microUsd(n) : metric === 'requests' ? plural(n, '{n} request', '{n} requests')
    : t('{n} tokens', {n: tokens(n)});
  const axis = (n: number) => money ? microUsd(n) : tokens(n);
  const top = niceMax(Math.max(0, ...rows.map(value)), money ? 20_000 : 4);
  const anyUnknown = metric === 'cost' && rows.some(r => r.unknownRequests > 0);
  const title = {
    cost: hourly ? t('Cost per Hour') : t('Cost per Day'),
    requests: hourly ? t('Requests per Hour') : t('Requests per Day'),
    tokens: hourly ? t('Tokens per Hour') : t('Tokens per Day'),
    comparison: hourly ? t('Comparison per Hour') : t('Comparison per Day'),
  }[metric];
  const when = (start: number) => new Date(start).toLocaleString(locale(), hourly
    ? {hour: 'numeric'} : {day: 'numeric', month: 'numeric'});
  const whenLong = (start: number) => new Date(start).toLocaleString(locale(), hourly
    ? {weekday: 'short', hour: '2-digit', minute: '2-digit'} : {weekday: 'short', day: 'numeric', month: 'long'});
  // About six labels under the axis, whatever the number of periods.
  const every = Math.max(1, Math.ceil(rows.length / 6));
  const shown = hover !== null ? rows[hover] : null;
  // One Tab stop for the chart (the latest period, or the one last looked at); the arrow keys, Home and End move along it.
  const [stop, setStop] = useState<number | null>(null);
  const tabStop = stop !== null && stop < rows.length ? stop : rows.length - 1;
  const keyDown = (e: React.KeyboardEvent<HTMLDivElement>, i: number) => {
    const next = rovingIndex(e.key, i, rows.length);
    if (next === null) return;
    e.preventDefault();
    setStop(next);
    setHover(next);
    (e.currentTarget.parentElement?.children[next] as HTMLElement | undefined)?.focus();
  };

  return <section className="group">
    <div className="group-heading">
      <h2>{title}</h2>
      <span className="heading-actions">
        <Segmented label={t('Chart metric')} value={metric} onChange={m => { setMetric(m); setHover(null); }} options={[
          {value: 'cost', label: t('Cost')}, {value: 'requests', label: t('Requests')}, {value: 'tokens', label: t('Tokens')},
          ...(comparison ? [{value: 'comparison' as const, label: t('Comparison')}] : [])]}/>
      </span>
    </div>
    <div className="group-box usage-chart">
      <div className="chart-area">
        <div className="chart-y" aria-hidden>
          <span>{axis(top)}</span><span>{axis(top / 2)}</span><span>{axis(0)}</span>
        </div>
        <div className="chart-plot" onMouseLeave={() => setHover(null)}>
          <div className="chart-grid" aria-hidden><span/><span/><span/></div>
          <div className="chart-bars" role="list" aria-label={title}>
            {rows.map((r, i) => <div key={r.start} role="listitem" tabIndex={i === tabStop ? 0 : -1} onKeyDown={e => keyDown(e, i)}
              className={'chart-column' + (hover === i ? ' active' : '')}
              aria-label={`${whenLong(r.start)}: ${label(value(r))}`
                + (metric === 'comparison' ? `, ${t('{amount} through Pygg', {amount: microUsd(at(r)?.routedMicroUsd ?? 0)})}` : '')
                + (metric !== 'requests' ? `, ${plural(r.requests, '{n} request', '{n} requests')}` : '')
                + (anyUnknown && r.unknownRequests ? `, ${plural(r.unknownRequests, '{n} with unknown cost', '{n} with unknown cost')}` : '')}
              onMouseEnter={() => setHover(i)} onFocus={() => { setHover(i); setStop(i); }} onBlur={() => setHover(null)}>
              {anyUnknown && r.unknownRequests > 0 && <span className="chart-unknown" style={{bottom: `calc(${(value(r) / top) * 100}% + 4px)`}}/>}
              <span className={metric === 'comparison' ? 'chart-bar baseline' : 'chart-bar'}
                style={{height: value(r) > 0 ? `max(2px, ${(value(r) / top) * 100}%)` : 0}}/>
              {metric === 'comparison' && (at(r)?.compared ?? 0) > 0 && <span className="chart-bar routed"
                style={{height: `max(2px, ${((at(r)?.routedMicroUsd ?? 0) / top) * 100}%)`}}/>}
            </div>)}
          </div>
          {shown && hover !== null && <div className="chart-tooltip" role="status"
            style={{left: `${((hover + 0.5) / rows.length) * 100}%`}}
            data-side={hover > rows.length / 2 ? 'left' : 'right'}>
            <strong>{whenLong(shown.start)}</strong>
            {metric === 'cost' && <span>{t('{amount} known', {amount: microUsd(shown.knownMicroUsd)})}
              {shown.actualMicroUsd > 0 && <> · {t('{amount} actual', {amount: microUsd(shown.actualMicroUsd)})}</>}</span>}
            {metric === 'tokens' && <span>{t('{in} in → {out} out', {in: tokens(shown.inputTokens), out: tokens(shown.outputTokens)})}</span>}
            {metric === 'comparison' && <Comparison period={at(shown)} model={comparison?.model ?? ''}/>}
            <span className={metric === 'requests' ? '' : 'secondary'}>{plural(shown.requests, '{n} request', '{n} requests')}
              {shown.failed > 0 && <> · {plural(shown.failed, '{n} failed', '{n} failed')}</>}</span>
            {metric === 'cost' && shown.unknownRequests > 0 && <span className="warn-text">
              {plural(shown.unknownRequests, '{n} request with unknown cost', '{n} requests with unknown cost')}</span>}
          </div>}
        </div>
      </div>
      <div className="chart-x" aria-hidden>
        {rows.map((r, i) => <span key={r.start}>{i % every === 0 ? when(r.start) : ''}</span>)}
      </div>
      {anyUnknown && <p className="chart-legend"><span className="chart-unknown legend"/>
        {t('Has requests with unknown cost, which are not in the bar')}</p>}
      {metric === 'comparison' && <p className="chart-legend">
        <span className="chart-key baseline" aria-hidden/>{t('The same tokens at {model}', {model: comparison?.model ?? ''})}
        <span className="chart-key routed" aria-hidden/>{t('What they cost through Pygg')}
        <span className="secondary">· {t('compared answers only; failed attempts and unknown costs are not in the bars')}</span></p>}
    </div>
  </section>;
}

/** The tooltip's comparison lines: both prices, the difference, and how many answers it rests on. */
function Comparison({period: c, model}: {period: SavingsPeriod | undefined; model: string}) {
  if (!c || c.compared === 0) return <span className="secondary">{t('No answer compared')}</span>;
  const d = c.baselineMicroUsd - c.routedMicroUsd;
  return <>
    <span>{t('≈{amount} at {model}', {amount: microUsd(c.baselineMicroUsd), model})} · {t('{amount} through Pygg', {amount: microUsd(c.routedMicroUsd)})}</span>
    <span className={d >= 0 ? 'savings-good' : 'savings-bad'}>{d >= 0 ? t('≈{amount} less', {amount: microUsd(d)}) : t('≈{amount} more', {amount: microUsd(-d)})}
      <span className="secondary"> · {plural(c.compared, '{n} answer compared', '{n} answers compared')}</span></span>
  </>;
}
