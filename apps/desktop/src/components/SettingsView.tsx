import { useEffect, useState } from 'react';
import { LANGUAGES, t, type LanguageChoice } from '../lib/i18n';
import { runningInDesktop } from '../lib/native';
import { preferences, setPreference, type Preferences } from '../lib/shell';
import { errorText } from '../lib/api';
import { ErrorNote, Switch } from './ui';

/** Settings (SCREENS.md §5.8): language, the tray and notifications. Gateway settings live in Overview. */
export function SettingsView({language, onLanguage, onGateway}: {language: LanguageChoice; onLanguage: (choice: LanguageChoice) => void;
  onGateway: () => void}) {
  return <div className="settings">
    <section className="group">
      <h2>{t('General')}</h2>
      <div className="group-box">
        <div className="group-row">
          <span className="group-label"><label htmlFor="language">{t('Language')}</label></span>
          <span className="group-control">
            <select id="language" className="popup-button" value={language}
              onChange={e => onLanguage(e.target.value as LanguageChoice)}>
              <option value="system">{t('System')}</option>
              {LANGUAGES.map(l => <option key={l.code} value={l.code}>{l.name}</option>)}
            </select>
          </span>
        </div>
      </div>
    </section>
    {runningInDesktop && <Background/>}
    <section className="group">
      <h2>{t('Gateway')}</h2>
      <div className="group-box">
        <div className="group-row">
          <span className="group-label">{t('Port and start-up')}<small>{t('Set in Overview, next to the gateway’s address.')}</small></span>
          <span className="group-control"><button type="button" className="button" onClick={onGateway}>{t('Open Overview')}</button></span>
        </div>
      </div>
    </section>
    <section className="group">
      <h2>{t('Appearance')}</h2>
      <div className="group-box">
        <div className="group-row">
          <span className="group-label">{t('Follows the system’s light, dark and contrast settings.')}</span>
        </div>
      </div>
    </section>
  </div>;
}

/** The tray and notifications: tasks started from an IDE run while the window is closed. */
function Background() {
  const [prefs, setPrefs] = useState<Preferences | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { preferences().then(setPrefs, e => setError(errorText(e))); }, []);
  const change = (key: keyof Preferences, value: boolean) =>
    setPreference(key, value).then(setPrefs, e => setError(errorText(e)));
  return <section className="group">
    <h2>{t('Background')}</h2>
    <div className="group-box">
      <div className="group-row">
        <span className="group-label">{t('Keep running in the tray')}
          <small>{t('Closing the window keeps the gateway and your tasks running. Quit from the tray icon.')}</small></span>
        <span className="group-control">{prefs && <Switch label={t('Keep running in the tray')} checked={prefs.keepInTray}
          onChange={v => change('keepInTray', v)}/>}</span>
      </div>
      <div className="group-row">
        <span className="group-label">{t('Notify me about tasks')}
          <small>{t('When a paid step waits for approval, or a task is ready, needs review or did not finish. Notifications never show a task’s name or files.')}</small></span>
        <span className="group-control">{prefs && <Switch label={t('Notify me about tasks')} checked={prefs.notifyTasks}
          onChange={v => change('notifyTasks', v)}/>}</span>
      </div>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </section>;
}
