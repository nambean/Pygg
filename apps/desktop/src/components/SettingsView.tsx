import { LANGUAGES, t, type LanguageChoice } from '../lib/i18n';

/** Settings, first cut (SCREENS.md §5.8): language and the appearance note. */
export function SettingsView({language, onLanguage}: {language: LanguageChoice; onLanguage: (choice: LanguageChoice) => void}) {
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
