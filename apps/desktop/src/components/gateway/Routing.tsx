import type { RoutingSummary } from '../../lib/api';
import { locale, t } from '../../lib/i18n';
import { microUsd, tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';

const pct = (part: number, whole: number) => whole > 0 ? Math.round((part / whole) * 100) : 0;

/**
 * Usage → Routing (Codex's fifth discussion, T2): whether requests stayed on one model, from attempts recorded with their
 * request (schema v30). A change of model inside a routing session is shown with the cache writes that followed; what the
 * change cost extra is not known (it would need what staying would have cost), so it is never called a cost.
 */
export function Routing({days, onReview}: {days: number; onReview: () => void}) {
  const routing = useGateway(api => api.usageRouting(days), [days], 15_000);
  const r: RoutingSummary | null = routing.data ?? null;
  if (!r) return null;
  const when = (at: number) => new Date(at).toLocaleString(locale(), {month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'});
  return <section className="group">
    <div className="group-heading">
      <h2>{t('Routing')}</h2>
      {r.answeredAfterFallback > 0 && <span className="heading-actions">
        <button type="button" className="link-button" onClick={onReview}>{t('Review Fallbacks')}</button></span>}
    </div>
    <div className="group-box token-mix">
      <p className="group-intro">{t('Whether requests stayed on one model, and what a fallback did.')}</p>
      {r.requests === 0
        ? <p className="secondary">{t('Recorded for requests sent from this version of Pygg on: none in this period yet.')}</p>
        : <dl className="savings-figures">
          <div><dt>{t('Requests')}</dt><dd>{r.requests}</dd></div>
          <div><dt>{t('Answered after a fallback')}</dt><dd>{r.answeredAfterFallback} <small className="secondary">{t('of {n} answered', {n: r.answered})}</small></dd></div>
          <div><dt>{t('Attempts with a routing session')}</dt><dd>{pct(r.withSession, r.attempts)}% <small className="secondary">{t('{n} sessions', {n: r.sessions})}</small></dd></div>
          <div><dt>{t('Changes of model in a session')}</dt><dd>{r.changes.length}</dd></div>
        </dl>}
      <p className="group-footnote">{t('A routing session keeps a launch on the model that answered it, and so on its prompt cache; pygg run gives each launch one. A fallback can happen before any answer, so it is not always a change.')}</p>
      {r.changes.length > 0 && <details className="token-mix-details">
        <summary>{t('Changes of model ({n})', {n: r.changes.length})}</summary>
        <div className="table-scroll"><table className="data-table" aria-label={t('Changes of model')}>
          <thead><tr>
            <th scope="col">{t('Time')}</th><th scope="col">{t('Client')}</th><th scope="col">{t('From')}</th><th scope="col">{t('To')}</th>
            <th scope="col">{t('Why')}</th><th scope="col" className="num">{t('Cache writes after')}</th><th scope="col" className="num">{t('Known cost')}</th>
          </tr></thead>
          <tbody>
            {r.changes.slice(0, 50).map((c, i) => <tr key={`${c.at}-${i}`}>
              <td className="secondary nowrap">{when(c.at)}</td><td>{c.client ?? t('No client')}</td>
              <td className="mono">{c.from}</td><td className="mono">{c.to}</td>
              <td className="secondary">{c.reason || t('Sent there first')}</td>
              <td className="num">{tokens(c.cacheWritesAfter)}</td>
              <td className="num">{c.knownMicroUsd === null ? t('Unknown') : microUsd(c.knownMicroUsd)}</td>
            </tr>)}
          </tbody>
        </table></div>
        <p className="group-footnote">{r.changes.length > 50 ? t('Showing the 50 latest of {n} changes. ', {n: r.changes.length}) : ''}{t('Usage after a change is observed; the extra cost caused by the change is not known.')}</p>
      </details>}
    </div>
  </section>;
}
