import { useState } from 'react';
import { FileDown } from 'lucide-react';
import { t } from '../lib/i18n';
import { call, errorText } from '../lib/api';
import { ErrorNote, Spinner } from './ui';

/** Settings → Diagnostics (SCREENS.md §5.8): one redacted zip to send when something goes wrong. The same bundle as
 * `pygg-cli diagnostics --out PATH`; what it holds is listed here before anything is saved (docs/SECURITY.md). */
export function DiagnosticsSettings() {
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<{ok: boolean; text: string} | null>(null);
  const save = () => {
    setSaving(true);
    setResult(null);
    call(api => api.exportDiagnostics())
      .then(path => path && setResult({ok: true, text: t('Saved to {path}', {path})}), e => setResult({ok: false, text: errorText(e)}))
      .finally(() => setSaving(false));
  };
  return <section className="group" aria-labelledby="diagnostics-heading">
    <h2 id="diagnostics-heading">{t('Diagnostics')}</h2>
    <div className="group-box">
      <div className="group-row">
        <span className="group-label">{t('Diagnostics bundle')}
          <small>{t('A zip to send when something goes wrong. It never holds keys, tokens, prompts, agent output, diffs or file contents.')}</small></span>
        <span className="group-control"><button type="button" className="button" disabled={saving} onClick={save} aria-busy={saving}>
          {saving ? <Spinner label={t('Saving…')}/> : <><FileDown size={13} aria-hidden/>{t('Save Diagnostics…')}</>}</button></span>
      </div>
    </div>
    {result && (result.ok ? <p className="group-footnote" role="status">{result.text}</p> : <ErrorNote>{result.text}</ErrorNote>)}
    <details className="group-footnote">
      <summary>{t('What the bundle holds')}</summary>
      <ul>
        <li>{t('App, schema and Windows versions; the gateway’s state and port.')}</li>
        <li>{t('Each connection’s kind, endpoint without its query, enabled state, billing, terms, health, and whether a key is set. Not its label.')}</li>
        <li>{t('Each client’s name and limits. Not its token.')}</li>
        <li>{t('How many rows each table holds, and the latest 200 requests: models, outcome, tokens and cost. Not the provider’s error text.')}</li>
        <li>{t('Each task’s state and its steps’ states. Not its title, prompt or reasons.')}</li>
        <li>{t('The last 200 lines of Pygg’s log (logs\\pygg.log), already redacted when written.')}</li>
      </ul>
      {t('Every text is cut to 200 characters and passed through the same redaction as task logs. The terminal can save the same zip with pygg-cli diagnostics --out PATH.')}
    </details>
  </section>;
}
