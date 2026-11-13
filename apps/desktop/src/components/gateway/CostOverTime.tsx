import { useMemo, useState } from 'react';
import type { UsageBucket } from '../../lib/api';
import { locale, plural, t } from '../../lib/i18n';
import { microUsd, tokens } from '../../lib/money';
import { Segmented } from '../ui';

/** What the bars measure: known cost, requests, or tokens in and out (a switch 9router's usage chart has too). */
export type ChartMetric = 'cost' | 'requests' | 'tokens';

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
export function CostOverTime({days, buckets}: {days: number; buckets: UsageBucket[]}) {
  const rows = useMemo(() => periods(days, buckets), [days, buckets]);
  const [hover, setHover] = useState<number | null>(null);
  const [metric, setMetric] = useState<ChartMetric>('cost');
  const hourly = days <= 1;
  const value = (r: UsageBucket) => metric === 'cost' ? r.knownMicroUsd : metric === 'requests' ? r.requests : r.inputTokens + r.outputTokens;
  const label = (n: number) => metric === 'cost' ? microUsd(n) : metric === 'requests' ? plural(n, '{n} request', '{n} requests')
    : t('{n} tokens', {n: tokens(n)});
  const axis = (n: number) => metric === 'cost' ? microUsd(n) : tokens(n);
  const top = niceMax(Math.max(0, ...rows.map(value)), metric === 'cost' ? 20_000 : 4);
  const anyUnknown = metric === 'cost' && rows.some(r => r.unknownRequests > 0);
  const title = {
    cost: hourly ? t('Cost per Hour') : t('Cost per Day'),
    requests: hourly ? t('Requests per Hour') : t('Requests per Day'),
    tokens: hourly ? t('Tokens per Hour') : t('Tokens per Day'),
  }[metric];
  const when = (start: number) => new Date(start).toLocaleString(locale(), hourly
    ? {hour: 'numeric'} : {day: 'numeric', month: 'numeric'});
  const whenLong = (start: number) => new Date(start).toLocaleString(locale(), hourly
    ? {weekday: 'short', hour: '2-digit', minute: '2-digit'} : {weekday: 'short', day: 'numeric', month: 'long'});
  // About six labels under the axis, whatever the number of periods.
  const every = Math.max(1, Math.ceil(rows.length / 6));
  const shown = hover !== null ? rows[hover] : null;

  return <section className="group">
    <div className="group-heading">
      <h2>{title}</h2>
      <span className="heading-actions">
        <Segmented label={t('Chart metric')} value={metric} onChange={m => { setMetric(m); setHover(null); }} options={[
          {value: 'cost', label: t('Cost')}, {value: 'requests', label: t('Requests')}, {value: 'tokens', label: t('Tokens')}]}/>
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
            {rows.map((r, i) => <div key={r.start} role="listitem" tabIndex={0} className={'chart-column' + (hover === i ? ' active' : '')}
              aria-label={`${whenLong(r.start)}: ${label(value(r))}`
                + (metric !== 'requests' ? `, ${plural(r.requests, '{n} request', '{n} requests')}` : '')
                + (anyUnknown && r.unknownRequests ? `, ${plural(r.unknownRequests, '{n} with unknown cost', '{n} with unknown cost')}` : '')}
              onMouseEnter={() => setHover(i)} onFocus={() => setHover(i)} onBlur={() => setHover(null)}>
              {anyUnknown && r.unknownRequests > 0 && <span className="chart-unknown" style={{bottom: `calc(${(value(r) / top) * 100}% + 4px)`}}/>}
              <span className="chart-bar" style={{height: value(r) > 0 ? `max(2px, ${(value(r) / top) * 100}%)` : 0}}/>
            </div>)}
          </div>
          {shown && hover !== null && <div className="chart-tooltip" role="status"
            style={{left: `${((hover + 0.5) / rows.length) * 100}%`}}
            data-side={hover > rows.length / 2 ? 'left' : 'right'}>
            <strong>{whenLong(shown.start)}</strong>
            {metric === 'cost' && <span>{t('{amount} known', {amount: microUsd(shown.knownMicroUsd)})}
              {shown.actualMicroUsd > 0 && <> · {t('{amount} actual', {amount: microUsd(shown.actualMicroUsd)})}</>}</span>}
            {metric === 'tokens' && <span>{t('{in} in → {out} out', {in: tokens(shown.inputTokens), out: tokens(shown.outputTokens)})}</span>}
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
    </div>
  </section>;
}
