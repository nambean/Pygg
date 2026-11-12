import { useState } from 'react';
import { call, errorText, gatewayChanged } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { ErrorNote } from '../ui';

/**
 * What routing saved, as an estimate (gateway/savings.rs): the answered requests priced again at a model the user
 * chose, with the same tokens. Says what it left out; an unknown cost is never taken as $0.
 */
export function Savings({days, compact}: {days: number; compact?: boolean}) {
  const savings = useGateway(api => api.savings(days), [days], 15_000);
  const models = useGateway(api => api.models());
  const [error, setError] = useState<string | null>(null);
  const priced = (models.data ?? []).filter(m => (m.price?.inputMicroPerMtok ?? 0) > 0 || (m.price?.outputMicroPerMtok ?? 0) > 0);
  const choose = (value: string) => {
    setError(null);
    call(api => api.setSavingsBaseline(value || null)).then(() => { savings.reload(); gatewayChanged(); }, e => setError(errorText(e)));
  };
  const s = savings.data;
  if (savings.error) return <ErrorNote>{savings.error}</ErrorNote>;
  if (!s) return null;
  const picker = <select className="popup-button" value={s.baseline ?? ''} onChange={e => choose(e.target.value)}
    aria-label={t('Model to compare with')}>
    <option value="">{t('Choose a model…')}</option>
    {priced.map(m => <option key={`${m.providerId}/${m.modelId}`} value={`${m.providerId}/${m.modelId}`}>{m.providerId}/{m.modelId}</option>)}
  </select>;
  const answered = s.answered.free + s.answered.local + s.answered.subscription + s.answered.paid + s.answered.unknown;
  const breakdown = [
    s.answered.free && t('{n} free', {n: String(s.answered.free)}), s.answered.local && t('{n} local', {n: String(s.answered.local)}),
    s.answered.subscription && t('{n} subscription', {n: String(s.answered.subscription)}),
    s.answered.paid && t('{n} paid', {n: String(s.answered.paid)}), s.answered.unknown && t('{n} unknown billing', {n: String(s.answered.unknown)}),
  ].filter(Boolean).join(' · ');
  const diff = s.baselineMicroUsd - s.routedMicroUsd;
  const leftOut = s.leftOutUnknownCost + s.leftOutNoTokens;
  const figure = !s.baseline ? null : !s.baselinePriced ? null : s.compared === 0 ? null
    : diff >= 0 ? t('≈{amount} less than {model}', {amount: microUsd(diff), model: s.baseline})
    : t('≈{amount} more than {model}', {amount: microUsd(-diff), model: s.baseline});

  const body = <>
    <div className="savings-main">
      {figure ? <span className="savings-figure">{figure}</span>
        : <span className="secondary">{!s.baseline ? t('Choose the model you would otherwise use, to see what routing saved.')
          : !s.baselinePriced ? t('{model} has no price yet: set one in Providers to compare.', {model: s.baseline})
          : t('No answered request with a known cost and tokens to compare yet.')}</span>}
      <span className="spacer"/>
      <label className="inline-field"><span>{t('Compared with')}</span>{picker}</label>
    </div>
    {figure && <p className="savings-detail">
      {t('Routed: {routed} · the same tokens at {model}: {baseline}.', {routed: microUsd(s.routedMicroUsd), model: s.baseline ?? '', baseline: microUsd(s.baselineMicroUsd)})}{' '}
      {t('Based on {n} of {total} answered requests.', {n: String(s.compared), total: String(answered)})}
      {s.failedAttemptsMicroUsd > 0 && <> {t('Routed includes {amount} for attempts that failed.', {amount: microUsd(s.failedAttemptsMicroUsd)})}</>}
    </p>}
    {(leftOut > 0 || s.failedUnknownCost > 0) && figure && <p className="savings-detail warn-text">
      {[s.leftOutUnknownCost && plural(s.leftOutUnknownCost, '{n} request with unknown cost left out', '{n} requests with unknown cost left out'),
        s.leftOutNoTokens && plural(s.leftOutNoTokens, '{n} without token counts left out', '{n} without token counts left out'),
        s.failedUnknownCost && plural(s.failedUnknownCost, '{n} failed attempt may have been billed, at an unknown cost', '{n} failed attempts may have been billed, at an unknown cost'),
      ].filter(Boolean).join(' · ')}</p>}
    {!compact && breakdown && <p className="savings-detail secondary">{t('Answered: {breakdown}. Subscription requests count against a plan and are not compared.', {breakdown})}</p>}
    {!compact && <p className="savings-detail secondary">{t('An estimate with the same tokens and cache split: another model would have written a different amount. Prices are list prices.')}</p>}
    {error && <ErrorNote>{error}</ErrorNote>}
  </>;
  return compact ? <div className="group-box savings compact">{body}</div> : <section className="group">
    <h2>{t('Estimated Savings')}</h2>
    <div className="group-box savings">{body}</div>
  </section>;
}
