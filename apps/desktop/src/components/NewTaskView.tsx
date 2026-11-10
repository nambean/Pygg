import { useState } from 'react';
import { t } from '../lib/i18n';
import { MOD } from '../lib/platform';
import type { Workspace } from '../lib/tasks';

type Strategy = 'free_first' | 'balanced' | 'quality';

/**
 * New Task (SCREENS.md §5.2.1). Workspace properties are shown read-only: they belong to the workspace.
 * Analysis needs the task engine, so Analyze Task stays off and says why.
 */
export function NewTaskView({workspace, workspaces, onWorkspace}: {
  workspace: Workspace | undefined; workspaces: Workspace[]; onWorkspace: (id: string) => void;
}) {
  const [text, setText] = useState('');
  const [strategy, setStrategy] = useState<Strategy>('free_first');

  return <div className="settings new-task">
    <section className="group">
      <h2><label htmlFor="task-text">{t('Task')}</label></h2>
      <textarea id="task-text" className="task-input" autoFocus value={text} onChange={e => setText(e.target.value)}
        placeholder={t('Describe the change, e.g. “Fix timeout handling and add regression tests.”')} rows={5}/>
    </section>

    <section className="group">
      <div className="group-box">
        <div className="group-row">
          <span className="group-label"><label htmlFor="strategy">{t('Strategy')}</label>
            <small>{strategy === 'free_first' ? t('Starts with the lowest eligible tier; paid steps ask first.')
              : strategy === 'balanced' ? t('Balances cost, speed and quality.')
              : t('Starts with a strong model for hard or risky work.')}</small></span>
          <span className="group-control">
            <select id="strategy" className="popup-button" value={strategy}
              onChange={e => setStrategy(e.target.value as Strategy)}>
              <option value="free_first">{t('Free First')}</option>
              <option value="balanced">{t('Balanced')}</option>
              <option value="quality">{t('Quality')}</option>
            </select>
          </span>
        </div>
        {!workspace && <div className="group-row">
          <span className="group-label"><label htmlFor="workspace">{t('Workspace')}</label></span>
          <span className="group-control">
            <select id="workspace" className="popup-button" value="" onChange={e => onWorkspace(e.target.value)}>
              <option value="" disabled>{t('Choose…')}</option>
              {workspaces.map(w => <option key={w.id} value={w.id}>{w.name}</option>)}
            </select>
          </span>
        </div>}
      </div>
    </section>

    {workspace && <section className="group">
      <h2>{t('From Workspace')}</h2>
      <div className="group-box">
        <div className="group-row"><span className="group-label">{t('Path')}</span>
          <span className="group-control"><span className="path mono selectable">{workspace.path}</span></span></div>
        <div className="group-row"><span className="group-label">{t('Privacy')}</span>
          <span className="group-control value">{t('Not set yet')}</span></div>
        <div className="group-row"><span className="group-label">{t('Checks')}</span>
          <span className="group-control value">{t('Not detected yet')}</span></div>
      </div>
      <p className="group-footnote">{t('Privacy, checks and permissions come from Workspace Settings. A task can only tighten them.')}</p>
    </section>}

    <div className="form-actions">
      <button type="button" className="button prominent" disabled title={t('Needs the task engine')}>
        {t('Analyze Task')}</button>
      <span className="shortcut-hint">{`${MOD}Enter`}</span>
    </div>
    <p className="group-footnote">{t('Analysis needs the task engine, which isn’t built yet. Nothing is sent anywhere.')}</p>
  </div>;
}
