import { useState } from 'react';
import { FolderGit2, FolderOpen, Plus, RefreshCw, ShieldAlert, Trash2, X } from 'lucide-react';
import { call, errorText, gatewayChanged } from '../lib/api';
import { t } from '../lib/i18n';
import { engineText } from '../lib/engineText';
import { runningInDesktop } from '../lib/native';
import type { WorkspaceView } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { ErrorNote, FormRow, Spinner } from './ui';

/**
 * Workspaces (SCREENS.md §5.3): the repositories tasks run in. A workspace an IDE agent created arrives with its
 * checks as suggestions; approving them here is what lets a task's result be confirmed rather than reviewed.
 */
export function WorkspacesView() {
  const workspaces = useGateway(api => api.workspaces());
  const [adding, setAdding] = useState(false);
  const list = workspaces.data ?? [];
  return <div className="settings workspaces">
    <div className="page-bar">
      <p className="group-intro">{t('Tasks from your IDE add their repository here by themselves. Approve its checks so results can be confirmed.')}</p>
      <span className="spacer"/>
      {!adding && <button type="button" className="button" onClick={() => setAdding(true)}><Plus size={13} aria-hidden/>{t('Add Workspace…')}</button>}
    </div>
    {adding && <section className="group"><div className="group-box">
      <AddWorkspaceForm onAdded={() => { setAdding(false); workspaces.reload(); gatewayChanged(); }} onCancel={() => setAdding(false)}/>
    </div></section>}
    {workspaces.error && <ErrorNote>{workspaces.error}</ErrorNote>}
    {!workspaces.data && workspaces.loading && <Spinner/>}
    {workspaces.data && list.length === 0 && !adding && <div className="placeholder">
      <h2>{t('No Workspaces Yet')}</h2>
      <p>{t('Add a Git repository, or start a task from your IDE: its repository is added for you.')}</p>
    </div>}
    {list.map(w => <WorkspaceCard key={w.id} workspace={w} onChanged={() => { workspaces.reload(); gatewayChanged(); }}/>)}
  </div>;
}

/** Workspaces whose checks wait for the user: suggestions and none approved. */
export function needsChecks(w: WorkspaceView): boolean {
  return w.checks.length > 0 && !w.checks.some(c => c.approved);
}

function WorkspaceCard({workspace: w, onChanged}: {workspace: WorkspaceView; onChanged: () => void}) {
  const [error, setError] = useState<string | null>(null);
  const [budget, setBudget] = useState(w.dailyBudgetUsd === null ? '' : String(w.dailyBudgetUsd));
  const act = (f: () => Promise<unknown>) => { setError(null); f().then(onChanged, e => setError(errorText(e))); };
  const amount = budget.trim() === '' ? null : Number(budget.replace(',', '.'));
  const validBudget = amount === null || (Number.isFinite(amount) && amount > 0);
  const saveBudget = () => { if (validBudget && amount !== w.dailyBudgetUsd) act(() => call(api => api.setWorkspaceBudget(w.id, amount))); };
  return <section className="group workspace-card">
    <div className="group-heading">
      <h2>{w.name}{needsChecks(w) && <span className="badge warn">{t('Checks to review')}</span>}</h2>
      <span className="heading-actions">
        {w.tasks === 0 && <button type="button" className="button small" onClick={() => act(() => call(api => api.removeWorkspace(w.id)))}>
          <Trash2 size={12} aria-hidden/>{t('Remove')}</button>}
      </span>
    </div>
    <div className="group-box">
      <div className="group-row"><span className="group-label">{t('Folder')}<small className="mono selectable">{w.path}</small></span>
        <span className="group-control value">{w.tasks === 1 ? t('1 task') : t('{n} tasks', {n: w.tasks})}</span></div>
      <div className="group-row">
        <span className="group-label"><label htmlFor={`privacy-${w.id}`}>{t('Privacy')}</label>
          <small>{w.privacyClass === 'sensitive'
            ? t('Every step asks before an agent sees this code, free ones too: Pygg cannot yet check where an agent sends it.')
            : t('Free and subscription steps run without asking; paid steps ask.')}</small></span>
        <span className="group-control">
          {w.privacyClass === 'sensitive' && <ShieldAlert size={14} className="warn-text" aria-hidden/>}
          <select id={`privacy-${w.id}`} className="popup-button" value={w.privacyClass}
            onChange={e => act(() => call(api => api.setWorkspacePrivacy(w.id, e.target.value as 'standard' | 'sensitive')))}>
            <option value="standard">{t('Standard')}</option>
            <option value="sensitive">{t('Sensitive')}</option>
          </select>
        </span>
      </div>
      <div className="group-row">
        <span className="group-label"><label htmlFor={`budget-${w.id}`}>{t('Paid steps per day here')}</label>
          <small>{t('On top of the overall limit in Settings. Empty: no limit of its own.')}</small></span>
        <span className="group-control"><span className={validBudget ? 'money-input' : 'money-input invalid'}><span aria-hidden>$</span>
          <input id={`budget-${w.id}`} inputMode="decimal" value={budget} placeholder={t('No limit')} onChange={e => setBudget(e.target.value)}
            onBlur={saveBudget} onKeyDown={e => { if (e.key === 'Enter') saveBudget(); }}/></span></span>
      </div>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
    <WorkspaceChecks workspace={w} onChanged={onChanged}/>
  </section>;
}

/** Adding a repository by path, or with the system's folder picker in the desktop app. */
export function AddWorkspaceForm({onAdded, onCancel}: {onAdded: (w: WorkspaceView) => void; onCancel: (() => void) | null}) {
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const add = (folder: string) => {
    setBusy(true);
    setError(null);
    call(api => api.addWorkspace(folder.trim(), null)).then(
      w => { setPath(''); setBusy(false); onAdded(w); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  const browse = () => call(api => api.pickFolder()).then(folder => { if (folder) { setPath(folder); add(folder); } },
    e => setError(errorText(e)));
  return <div className="form">
    <FormRow label={t('Folder')} htmlFor="workspace-path"
      hint={t('A Git repository. Every attempt works on its own worktree of it, so your files and branch are never touched.')}>
      <span className="input-with-icon"><FolderGit2 size={13} aria-hidden/>
        <input id="workspace-path" className="text-input mono" value={path} onChange={e => setPath(e.target.value)}
          placeholder="C:\\path\\to\\repo" spellCheck={false} disabled={busy}/>
        {runningInDesktop && <button type="button" className="button" onClick={browse} disabled={busy}>
          <FolderOpen size={13} aria-hidden/>{t('Browse…')}</button>}</span>
    </FormRow>
    <div className="form-actions">
      {onCancel && <button type="button" className="button" onClick={onCancel} disabled={busy}>{t('Cancel')}</button>}
      <button type="button" className="button prominent" onClick={() => add(path)} disabled={busy || !path.trim()}>{t('Add Workspace')}</button>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </div>;
}
/** The workspace's checks. Approving one lets it run on every attempt's changes, in that attempt's copy. */
export function WorkspaceChecks({workspace, onChanged}: {workspace: WorkspaceView; onChanged: () => void}) {
  const [command, setCommand] = useState('');
  const [error, setError] = useState<string | null>(null);
  const act = (cmd: string, action: 'approve' | 'unapprove' | 'add' | 'remove') => {
    setError(null);
    call(api => api.workspaceCheck(workspace.id, cmd, action)).then(() => { if (action === 'add') setCommand(''); onChanged(); },
      e => setError(errorText(e)));
  };
  const approved = workspace.checks.filter(c => c.approved).length;
  return <section className="group">
    <h2>{t('Checks')}</h2>
    <div className="group-box">
      {workspace.checks.map(c => <div key={c.command} className="group-row">
        <label className="check-line group-label">
          <input type="checkbox" checked={c.approved} onChange={e => act(c.command, e.target.checked ? 'approve' : 'unapprove')}/>
          <span className="mono">{c.command}</span>
          <small className="secondary">{c.source === 'user' ? t('added by you') : t('found in {file}', {file: c.source})}</small>
          {c.covers && <small className="check-covers secondary">{engineText(c.covers)}</small>}
        </label>
        {c.source === 'user' && <button type="button" className="button small icon-only" aria-label={t('Remove')} title={t('Remove')}
          onClick={() => act(c.command, 'remove')}><X size={13}/></button>}
      </div>)}
      <div className="group-row">
        <input className="text-input mono" value={command} onChange={e => setCommand(e.target.value)} aria-label={t('Add a check')}
          placeholder={t('Add a check, e.g. pnpm test')} onKeyDown={e => { if (e.key === 'Enter' && command.trim()) act(command, 'add'); }}/>
        <button type="button" className="button" disabled={!command.trim()} onClick={() => act(command, 'add')}>{t('Add')}</button>
        <button type="button" className="button" title={t('Look for checks again')}
          onClick={() => call(api => api.detectChecks(workspace.id)).then(onChanged, e => setError(errorText(e)))}>
          <RefreshCw size={13} aria-hidden/>{t('Detect')}</button>
      </div>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
    <p className="group-footnote">{approved === 0
      ? t('No check is approved: a finished attempt waits for your review instead of being confirmed.')
      : t('Approved checks run in order on each attempt’s copy, and stop at the first failure. They run the code the agent wrote, on this computer.')}</p>
  </section>;
}

