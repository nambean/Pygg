import { useState } from 'react';
import { ago } from '../lib/format';
import { plural, t } from '../lib/i18n';
import { call, errorText, gatewayChanged, type RetentionDays } from '../lib/api';
import { useGateway } from '../hooks/useGateway';
import { ConfirmSheet, ErrorNote, Spinner } from './ui';

const CHOICES: RetentionDays[] = [30, 90, 365, null];
const choiceLabel = (days: RetentionDays) => days === null ? t('Forever') : days === 365 ? t('1 year') : t('{n} days', {n: days});

/** Settings → Usage History (docs/archive/SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md §2.7): how long the ledger and route decisions are kept.
 * Forever by default. A shorter period asks first, with how many requests would go now (retention.rs). */
export function DataRetentionSettings() {
  const retention = useGateway(api => api.retention());
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<{days: number; count: number} | null>(null);
  const current = retention.data?.days ?? null;
  // Saving a shorter period deletes the older history at once, which takes a while on a large ledger: the choice is
  // held and the row says what is happening until it is done (Codex UI review, Later 6).
  const [busy, setBusy] = useState<'saving' | 'checking' | null>(null);
  const save = async (days: RetentionDays) => {
    setBusy('saving');
    try {
      await call(api => api.setRetention(days));
      retention.reload();
      gatewayChanged();
    } finally {
      setBusy(null);
    }
  };
  const choose = (days: RetentionDays) => {
    setError(null);
    const shorter = days !== null && (current === null || days < current);
    if (!shorter) { save(days).catch(e => setError(errorText(e))); return; }
    setBusy('checking');
    call(api => api.retentionPreview(days)).then(count => { setBusy(null); return count > 0 ? setConfirm({days, count}) : save(days); })
      .catch(e => { setBusy(null); setError(errorText(e)); });
  };
  const last = retention.data?.lastRun;
  return <section className="group" aria-labelledby="retention-heading">
    <h2 id="retention-heading">{t('Usage History')}</h2>
    <div className="group-box">
      <div className="group-row">
        <span className="group-label"><label htmlFor="retention-days">{t('Keep usage history for')}</label>
          <small>{t('The requests in Usage and why each route was chosen. Older ones are deleted when Pygg starts and once a day. Requests a task’s cost is counted from, and this month’s requests that client limits count, are always kept.')}</small></span>
        <span className="group-control">{retention.data && <select id="retention-days" className="popup-button"
          value={current === null ? 'forever' : String(current)} disabled={busy !== null} aria-busy={busy !== null}
          onChange={e => choose(e.target.value === 'forever' ? null : Number(e.target.value) as RetentionDays)}>
          {CHOICES.map(d => <option key={String(d)} value={d === null ? 'forever' : String(d)}>{choiceLabel(d)}</option>)}
        </select>}</span>
      </div>
    </div>
    {busy && <p className="group-footnote"><Spinner label={busy === 'checking'
      ? t('Counting the requests older than that…') : t('Saving; deleting the older history can take a moment…')}/></p>}
    {(error || retention.error) && <ErrorNote>{error ?? retention.error}</ErrorNote>}
    {last && <p className="group-footnote">{last.usageRows > 0
      ? plural(last.usageRows, 'Last clean-up {when}: {n} request deleted.', 'Last clean-up {when}: {n} requests deleted.').replace('{when}', ago(last.at))
      : t('Last clean-up {when}: nothing was old enough to delete.', {when: ago(last.at)})}</p>}
    {confirm && <ConfirmSheet title={t('Keep usage history for {period}?', {period: choiceLabel(confirm.days as RetentionDays)})}
      message={plural(confirm.count, '{n} request older than that is deleted now, and older ones every day after. This cannot be undone. Export CSV in Usage first to keep a copy.',
        '{n} requests older than that are deleted now, and older ones every day after. This cannot be undone. Export CSV in Usage first to keep a copy.')}
      confirmLabel={t('Delete Older History')} onConfirm={() => save(confirm.days as RetentionDays)} onClose={() => setConfirm(null)}/>}
  </section>;
}
