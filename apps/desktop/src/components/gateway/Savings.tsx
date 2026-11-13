import { useState } from 'react';
import { PiggyBank } from 'lucide-react';
import { call, errorText, gatewayChanged, type Savings as SavingsData, type SavingsPart } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { ErrorNote } from '../ui';

/** The estimate for the last `days`, shared by the hero and the details on one screen (one fetch, not two). */
export function useSavings(days: number) {
  return useGateway(api => api.savings(days), [days], 15_000);
}
type Loaded = ReturnType<typeof useSavings>;

/** The billings an answer can have, in the order the bar shows them; the first three cost no money per request. */
const BILLINGS = ['free', 'local', 'subscription', 'paid', 'unknown'] as const;
const billingName = (b: string) => ({free: t('Free'), local: t('Local'), subscription: t('Subscription'), paid: t('Paid'),
  unknown: t('Billing unknown')} as Record<string, string>)[b] ?? b;
const routeName = (r: string) => ({auto: t('auto (Pygg chose)'), 'auto/cheap': t('auto/cheap (Pygg chose the cheapest)'),
  combo: t('A combo (your order, Pygg’s pick)'), named: t('A model by name (your choice)')} as Record<string, string>)[r] ?? r;
const answered = (s: SavingsData) => s.answered.free + s.answered.local + s.answered.subscription + s.answered.paid + s.answered.unknown;

/**
 * What routing saved, as an estimate (gateway/savings.rs): the answered requests priced again at the model the user
 * would otherwise use, with the same tokens. The figure leads Home and Usage with the three numbers behind it, and
 * where the answers went (free, local, a plan, paid), so the mechanism is visible, not only the result. What it left
 * out is always said, and an unknown cost is never taken as $0.
 */
export function Savings({savings, period}: {savings: Loaded; period: string}) {
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
  const total = answered(s);
  const diff = s.baselineMicroUsd - s.routedMicroUsd;
  const ready = !!s.baseline && s.baselinePriced && s.compared > 0;
  const percent = ready && s.baselineMicroUsd > 0 ? Math.round(diff / s.baselineMicroUsd * 100) : null;
  const exclusions = [
    s.leftOutUnknownCost && plural(s.leftOutUnknownCost, '{n} answer with unknown cost excluded', '{n} answers with unknown cost excluded'),
    s.leftOutNoTokens && plural(s.leftOutNoTokens, '{n} answer without token counts excluded', '{n} answers without token counts excluded'),
    s.answered.subscription && plural(s.answered.subscription, '{n} subscription answer not compared (a plan, not a price)', '{n} subscription answers not compared (a plan, not a price)'),
  ].filter(Boolean) as string[];

  return <section className="savings-hero" aria-label={t('Estimated savings')}>
    <span className="savings-icon" aria-hidden><PiggyBank size={22} strokeWidth={1.8}/></span>
    <div className="savings-hero-body">
      {ready ? <>
        <div className="savings-hero-line">
          <span className={diff >= 0 ? 'savings-amount' : 'savings-amount more'}>≈{microUsd(Math.abs(diff))}</span>
          <span className="savings-caption">{diff >= 0 ? t('estimated savings') : t('estimated extra cost')} · {period}
            {percent !== null && diff >= 0 && <span className="savings-percent"> · {t('{n}% less', {n: String(percent)})}</span>}</span>
        </div>
        <p className="savings-line">{t('{n} of {total} answered requests compared with {model}: the same tokens priced at it, against what they cost through Pygg.',
          {n: String(s.compared), total: String(total), model: s.baseline ?? ''})}</p>
        <dl className="savings-figures">
          <div><dt>{t('Priced at {model}', {model: s.baseline ?? ''})}</dt><dd>≈{microUsd(s.baselineMicroUsd)}</dd></div>
          <div><dt>{t('Cost through Pygg')}</dt><dd>{microUsd(s.routedMicroUsd)}</dd></div>
          <div><dt>{diff >= 0 ? t('Estimated difference') : t('Estimated extra cost')}</dt>
            <dd className={diff >= 0 ? 'savings-good' : 'savings-bad'}>≈{microUsd(Math.abs(diff))}</dd></div>
        </dl>
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
      {total > 0 && <BillingMix s={s}/>}
      {exclusions.length > 0 && (ready || total > 0) && <p className="savings-line warn-text">{exclusions.join(' · ')}</p>}
      {s.failedUnknownCost > 0 && <p className="savings-line warn-text">
        {plural(s.failedUnknownCost, '{n} failed attempt may have been billed at an unknown cost, which no figure here includes.',
          '{n} failed attempts may have been billed at an unknown cost, which no figure here includes.')}</p>}
      <details className="savings-method">
        <summary>{t('How this estimate works')}</summary>
        <ul>
          <li>{t('The same token counts and cache split, at list prices. Another model would have written a different amount.')}</li>
          <li>{t('Cost through Pygg is what the compared answers cost, plus failed attempts with a known cost in the period.')}
            {ready && s.failedAttemptsMicroUsd > 0 && <> {t('Here: {answers} for the answers and {failed} for failed attempts.',
              {answers: microUsd(s.routedMicroUsd - s.failedAttemptsMicroUsd), failed: microUsd(s.failedAttemptsMicroUsd)})}</>}</li>
          <li>{t('A subscription is a plan, not a price per request: its answers are counted, never priced. Local runs exclude electricity and hardware.')}</li>
          <li>{t('Billing and prices are read from the current settings of each connection, so a changed setting changes past figures too.')}</li>
          <li>{t('A request that went to a free, local or plan model used what you already have; that is not proof you would have paid for it otherwise.')}</li>
        </ul>
      </details>
      {error && <ErrorNote>{error}</ErrorNote>}
    </div>
    <div className="savings-hero-actions">
      {(changing || !s.baseline) ? picker
        : <button type="button" className="button small" onClick={() => setChanging(true)}>{t('Change comparison')}</button>}
    </div>
  </section>;
}

/** Where the answered requests went: one bar, every billing with its count and share. Shown with or without a baseline. */
function BillingMix({s}: {s: SavingsData}) {
  const total = answered(s);
  const parts = BILLINGS.map(b => ({key: b, n: s.answered[b]})).filter(p => p.n > 0);
  const noMoney = s.answered.free + s.answered.local + s.answered.subscription;
  return <div className="billing-mix">
    <p className="savings-line"><b>{t('Where the answers came from')}</b> · {t('{n} of {total} from free, local or plan models', {n: String(noMoney), total: String(total)})}</p>
    <div className="mix-bar" role="img" aria-label={parts.map(p => `${billingName(p.key)} ${p.n}`).join(', ')}>
      {parts.map(p => <span key={p.key} className={`mix-${p.key}`} style={{flexGrow: p.n}} title={`${billingName(p.key)}: ${p.n}`}/>)}
    </div>
    <ul className="mix-legend">
      {parts.map(p => <li key={p.key}><span className={`mix-dot mix-${p.key}`} aria-hidden/>{billingName(p.key)} <b>{p.n}</b>
        <span className="secondary"> · {Math.round(p.n / total * 100)}%</span></li>)}
    </ul>
  </div>;
}

/**
 * Usage: the estimate taken apart, so the mechanism shows. By client (what each tool's requests would have cost), by
 * how the model was asked for (Pygg's choice against the user's), and the fallbacks that were tried. Nothing here is
 * claimed that the ledger does not hold: a model asked for by name is the user's choice, and a fallback is not a saving.
 */
export function SavingsDetails({savings, onPickClient}: {savings: Loaded; onPickClient: (client: string) => void}) {
  const s = savings.data;
  if (!s || !s.baseline || !s.baselinePriced || s.compared === 0) return null;
  const model = s.baseline;
  const diff = (p: SavingsPart) => p.baselineMicroUsd - p.routedMicroUsd;
  const Diff = ({p}: {p: SavingsPart}) => p.compared === 0 ? <span className="secondary">—</span>
    : <span className={diff(p) >= 0 ? 'savings-good' : 'savings-bad'}>{diff(p) >= 0 ? '≈' : '−≈'}{microUsd(Math.abs(diff(p)))}</span>;
  const head = <thead><tr>
    <th scope="col"/><th scope="col" className="num">{t('Compared / answered')}</th><th scope="col" className="num">{t('Through Pygg')}</th>
    <th scope="col" className="num">{t('At {model}', {model})}</th><th scope="col" className="num">{t('Difference')}</th>
  </tr></thead>;
  const row = (p: SavingsPart, name: React.ReactNode) => <tr key={p.key}>
    <td>{name}</td>
    <td className="num">{p.compared} / {p.answered}</td>
    <td className="num">{p.compared ? microUsd(p.routedMicroUsd) : '—'}</td>
    <td className="num">{p.compared ? `≈${microUsd(p.baselineMicroUsd)}` : '—'}</td>
    <td className="num"><Diff p={p}/></td>
  </tr>;
  return <section className="group">
    <h2>{t('How Pygg Saved It')}</h2>
    <div className="two-columns">
      <div className="group-box table-box">
        <table className="data-table" aria-label={t('Comparison by client')}>
          <caption>{t('By client')}</caption>
          {head}
          <tbody>{s.byClient.map(p => row(p, <button type="button" className="row-link" onClick={() => onPickClient(p.key)}
            aria-label={t('Show the requests of {name}', {name: p.key || t('No client')})}>{p.key || t('No client')}</button>))}</tbody>
        </table>
      </div>
      <div className="group-box table-box">
        <table className="data-table" aria-label={t('Comparison by how the model was asked for')}>
          <caption>{t('By how the model was asked for')}</caption>
          {head}
          <tbody>{s.byRoute.map(p => row(p, routeName(p.key)))}</tbody>
        </table>
      </div>
    </div>
    <p className="group-footnote">
      {t('Answers through auto and combos are Pygg’s choice. A model asked for by name is yours: its difference is listed, not claimed.')}{' '}
      {s.fellBack > 0 && t('{n} fallback attempts in the period: a model failed and the next one was tried. A fallback keeps a request alive; it is not in itself a saving.', {n: String(s.fellBack)})}{' '}
      {t('Failed attempts with a known cost are in the total above, not in these rows.')}
    </p>
  </section>;
}
