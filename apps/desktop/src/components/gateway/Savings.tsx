import { useState } from 'react';
import { PiggyBank } from 'lucide-react';
import { call, errorText, gatewayChanged } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { ErrorNote } from '../ui';

/**
 * What routing saved, as an estimate (gateway/savings.rs): the answered requests priced again at the model the user
 * would otherwise use, with the same tokens. The figure leads Home and Usage; what it left out is always said, and
 * an unknown cost is never taken as $0.
 */
export function Savings({days, period}: {days: number; period: string}) {
  const savings = useGateway(api => api.savings(days), [days], 15_000);
  const models = useGateway(api => api.models());
  const [error, setError] = useState<string | null>(null);
  const [changing, setChanging] = useState(false);
  const priced = (models.data ?? []).filter(m => (m.price?.inputMicroPerMtok ?? 0) > 0 || (m.price?.outputMicroPerMtok ?? 0) > 0);
  const choose = (value: string | null) => {
    setError(null);
    call(api => api.setSavingsBaseline(value)).then(() => { setChanging(false); savings.reload(); gatewayChanged(); },
      e => setError(errorText(e)));
  };
  const s = savings.data;
  if (savings.error) return <ErrorNote>{savings.error}</ErrorNote>;
  if (!s) return null;
  const picker = <select className="popup-button" value={s.baseline ?? ''} onChange={e => choose(e.target.value || null)}
    aria-label={t('Model to compare with')}>
    <option value="">{t('Choose a model…')}</option>
    {priced.map(m => <option key={`${m.providerId}/${m.modelId}`} value={`${m.providerId}/${m.modelId}`}>{m.providerId}/{m.modelId}</option>)}
  </select>;
  const answered = s.answered.free + s.answered.local + s.answered.subscription + s.answered.paid + s.answered.unknown;
  const diff = s.baselineMicroUsd - s.routedMicroUsd;
  const ready = !!s.baseline && s.baselinePriced && s.compared > 0;
  const percent = ready && s.baselineMicroUsd > 0 ? Math.round(diff / s.baselineMicroUsd * 100) : null;
  const exclusions = [
    s.leftOutUnknownCost && plural(s.leftOutUnknownCost, '{n} request with unknown cost left out', '{n} requests with unknown cost left out'),
    s.leftOutNoTokens && plural(s.leftOutNoTokens, '{n} request without token counts left out', '{n} requests without token counts left out'),
    s.answered.subscription && plural(s.answered.subscription, '{n} subscription request not compared (a plan, not a price)', '{n} subscription requests not compared (a plan, not a price)'),
    s.failedUnknownCost && plural(s.failedUnknownCost, '{n} failed attempt may have been billed at an unknown cost', '{n} failed attempts may have been billed at an unknown cost'),
  ].filter(Boolean) as string[];

  return <section className="savings-hero" aria-label={t('Estimated savings')}>
    <span className="savings-icon" aria-hidden><PiggyBank size={22} strokeWidth={1.8}/></span>
    <div className="savings-hero-body">
      {ready ? <>
        <div className="savings-hero-line">
          <span className={diff >= 0 ? 'savings-amount' : 'savings-amount more'}>≈{microUsd(Math.abs(diff))}</span>
          <span className="savings-caption">{diff >= 0 ? t('estimated savings') : t('more than the comparison')} · {period}
            {percent !== null && diff >= 0 && <span className="savings-percent"> · {t('{n}% less', {n: String(percent)})}</span>}</span>
        </div>
        <p className="savings-line">{t('Compared with {model}: routed {routed}, the same tokens at {model} {baseline}.',
          {model: s.baseline ?? '', routed: microUsd(s.routedMicroUsd), baseline: microUsd(s.baselineMicroUsd)})}{' '}
          <b>{t('{n} of {total} answered requests compared.', {n: String(s.compared), total: String(answered)})}</b>
          {s.failedAttemptsMicroUsd > 0 && <> {t('Routed includes {amount} for failed attempts in the period.', {amount: microUsd(s.failedAttemptsMicroUsd)})}</>}</p>
      </> : <>
        <div className="savings-hero-line"><span className="savings-title">{t('See what Pygg saves you')}</span></div>
        <p className="savings-line">{!s.baseline ? t('Choose the model you would otherwise use; Pygg prices the same requests at it and shows the difference.')
          : !s.baselinePriced ? t('{model} has no price, so nothing can be compared. Add a price in Providers or choose another model.', {model: s.baseline})
          : t('No answered request in this period has a known cost and token counts yet.')}</p>
        {!s.baseline && s.suggested && <p className="savings-line">
          <button type="button" className="button prominent small" onClick={() => choose(s.suggested)}>
            {t('Compare with {model}', {model: s.suggested})}</button>{' '}
          <span className="secondary">{t('your most-used paid model, asked for by name in this period')}</span></p>}
      </>}
      {exclusions.length > 0 && (ready || answered > 0) && <p className="savings-line warn-text">{exclusions.join(' · ')}</p>}
      <p className="savings-line secondary">{t('An estimate with the same tokens and cache split at list prices: another model would have written a different amount. Local runs exclude electricity and hardware.')}</p>
      {error && <ErrorNote>{error}</ErrorNote>}
    </div>
    <div className="savings-hero-actions">
      {(changing || !s.baseline) ? picker
        : <button type="button" className="button small" onClick={() => setChanging(true)}>{t('Change comparison')}</button>}
    </div>
  </section>;
}
