import { useMemo, useState } from 'react';
import type { UsageBucket } from '../../lib/api';
import { locale, plural, t } from '../../lib/i18n';
import { microUsd } from '../../lib/money';

/** A round top for the axis whose half is round too: 1, 2, 4, 6 or 8 times a power of ten (in micro-USD, at least two
 *  cents, so the labels never need more than two decimals). */
function niceMax(value: number): number {
  const at = Math.max(value, 20_000);
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
    out.push(byStart.get(start) ?? {start, requests: 0, failed: 0, knownMicroUsd: 0, actualMicroUsd: 0, unknownRequests: 0});
  }
  return out;
}

/**
 * Known API cost per hour (24 hours) or per day, one series in the accent colour. Unknown costs are not in the bars;
 * a period with requests of unknown cost carries a warning marker, named in the legend and the tooltip.
 */
export function CostOverTime({days, buckets}: {days: number; buckets: UsageBucket[]}) {
  const rows = useMemo(() => periods(days, buckets), [days, buckets]);
  const [hover, setHover] = useState<number | null>(null);
  const hourly = days <= 1;
  const top = niceMax(Math.max(0, ...rows.map(r => r.knownMicroUsd)));
  const anyUnknown = rows.some(r => r.unknownRequests > 0);
  const when = (start: number) => new Date(start).toLocaleString(locale(), hourly
    ? {hour: 'numeric'} : {day: 'numeric', month: 'numeric'});
  const whenLong = (start: number) => new Date(start).toLocaleString(locale(), hourly
    ? {weekday: 'short', hour: '2-digit', minute: '2-digit'} : {weekday: 'short', day: 'numeric', month: 'long'});
  // About six labels under the axis, whatever the number of periods.
  const every = Math.max(1, Math.ceil(rows.length / 6));
  const shown = hover !== null ? rows[hover] : null;

  return <section className="group">
    <h2>{hourly ? t('Cost per Hour') : t('Cost per Day')}</h2>
    <div className="group-box usage-chart">
      <div className="chart-area">
        <div className="chart-y" aria-hidden>
          <span>{microUsd(top)}</span><span>{microUsd(top / 2)}</span><span>{microUsd(0)}</span>
        </div>
        <div className="chart-plot" onMouseLeave={() => setHover(null)}>
          <div className="chart-grid" aria-hidden><span/><span/><span/></div>
          <div className="chart-bars" role="list" aria-label={hourly ? t('Cost per Hour') : t('Cost per Day')}>
            {rows.map((r, i) => <div key={r.start} role="listitem" tabIndex={0} className={'chart-column' + (hover === i ? ' active' : '')}
              aria-label={`${whenLong(r.start)}: ${microUsd(r.knownMicroUsd)}, ${plural(r.requests, '{n} request', '{n} requests')}`
                + (r.unknownRequests ? `, ${plural(r.unknownRequests, '{n} with unknown cost', '{n} with unknown cost')}` : '')}
              onMouseEnter={() => setHover(i)} onFocus={() => setHover(i)} onBlur={() => setHover(null)}>
              {r.unknownRequests > 0 && <span className="chart-unknown" style={{bottom: `calc(${(r.knownMicroUsd / top) * 100}% + 4px)`}}/>}
              <span className="chart-bar" style={{height: r.knownMicroUsd > 0 ? `max(2px, ${(r.knownMicroUsd / top) * 100}%)` : 0}}/>
            </div>)}
          </div>
          {shown && hover !== null && <div className="chart-tooltip" role="status"
            style={{left: `${((hover + 0.5) / rows.length) * 100}%`}}
            data-side={hover > rows.length / 2 ? 'left' : 'right'}>
            <strong>{whenLong(shown.start)}</strong>
            <span>{t('{amount} known', {amount: microUsd(shown.knownMicroUsd)})}
              {shown.actualMicroUsd > 0 && <> · {t('{amount} actual', {amount: microUsd(shown.actualMicroUsd)})}</>}</span>
            <span className="secondary">{plural(shown.requests, '{n} request', '{n} requests')}
              {shown.failed > 0 && <> · {plural(shown.failed, '{n} failed', '{n} failed')}</>}</span>
            {shown.unknownRequests > 0 && <span className="warn-text">
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
