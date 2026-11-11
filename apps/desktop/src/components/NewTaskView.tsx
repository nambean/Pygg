import { useEffect, useMemo, useState } from 'react';
import { ArrowDown, ArrowUp, FolderGit2, Play, Plus, RefreshCw, X } from 'lucide-react';
import { call, errorText, gatewayChanged, type AgentInfo } from '../lib/api';
import { t } from '../lib/i18n';
import { MOD } from '../lib/platform';
import { MAX_STEPS, type Billing, type Step, type Strategy, type Tier, type WorkspaceView } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { billingLabel } from './AttemptSequence';
import { ErrorNote, FormRow, Spinner } from './ui';

/** The order each strategy tries the installed agents in; the user can change every step. */
const ORDER: Record<Strategy, string[]> = {
  free_first: ['opencode', 'gemini-cli', 'codex', 'claude-code'],
  balanced: ['codex', 'opencode', 'claude-code', 'gemini-cli'],
  quality: ['claude-code', 'codex', 'gemini-cli', 'opencode'],
};
/** A starting guess for how capable each agent's default model is; editable per step. */
const TIER_GUESS: Record<string, Tier> = {opencode: 1, 'gemini-cli': 2, codex: 3, 'claude-code': 3};
const BILLINGS: Billing[] = ['free', 'local', 'subscription', 'paid', 'unknown'];

function suggestedSteps(strategy: Strategy, agents: AgentInfo[]): Step[] {
  return ORDER[strategy].map(id => agents.find(a => a.id === id && a.installed)).filter((a): a is AgentInfo => !!a).slice(0, 3)
    .map(a => ({agent: a.id, model: null, tier: TIER_GUESS[a.id] ?? 2,
      // Only Codex says how it is signed in; anything else stays unknown until the user says.
      billing: a.id === 'codex' && /chatgpt/i.test(a.login ?? '') ? 'subscription' : 'unknown'}));
}

/**
 * New Task (SCREENS.md §5.2.1): the workspace, its checks, and the ladder of agent steps the task climbs. Each
 * step runs with the agent's own sign-in in a fresh worktree; a step that fails the checks hands what failed to
 * the next one. A step marked paid waits for approval.
 */
export function NewTaskView({onCreated}: {onCreated: (id: string) => void}) {
  const workspaces = useGateway(api => api.workspaces());
  const agents = useGateway(api => api.agents());
  const [workspaceId, setWorkspaceId] = useState('');
  const [text, setText] = useState('');
  const [strategy, setStrategy] = useState<Strategy>('free_first');
  const [steps, setSteps] = useState<Step[] | null>(null);
  const [allowShell, setAllowShell] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const list = workspaces.data ?? [];
  const workspace = list.find(w => w.id === workspaceId);
  useEffect(() => { if (!workspaceId && list.length > 0) setWorkspaceId(list[0].id); }, [list, workspaceId]);
  const installed = useMemo(() => (agents.data ?? []).filter(a => a.installed), [agents.data]);
  useEffect(() => { if (agents.data) setSteps(suggestedSteps(strategy, agents.data)); }, [agents.data, strategy]);

  const canStart = !!workspace && text.trim() !== '' && (steps?.length ?? 0) > 0 && !busy;
  const start = () => {
    if (!canStart || !workspace || !steps) return;
    setBusy(true);
    setError(null);
    call(api => api.createTask({workspaceId: workspace.id, title: null, prompt: text.trim(), strategy, steps, allowShell, timeoutSecs: null}))
      .then(id => { gatewayChanged(); onCreated(id); }, e => { setError(errorText(e)); setBusy(false); });
  };
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') { event.preventDefault(); start(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  return <div className="settings new-task">
    <section className="group">
      <h2><label htmlFor="task-text">{t('Task')}</label></h2>
      <textarea id="task-text" className="task-input" autoFocus value={text} onChange={e => setText(e.target.value)}
        placeholder={t('Describe the change, e.g. “Fix timeout handling and add regression tests.”')} rows={5}/>
    </section>

    <WorkspacePicker workspaces={list} loading={workspaces.loading && !workspaces.data} selected={workspaceId}
      onSelect={setWorkspaceId} onChanged={w => { workspaces.reload(); if (w) setWorkspaceId(w.id); }}/>
    {workspace && <Checks workspace={workspace} onChanged={workspaces.reload}/>}

    <section className="group">
      <h2>{t('Steps')}</h2>
      <div className="group-box">
        <div className="group-row">
          <span className="group-label"><label htmlFor="strategy">{t('Strategy')}</label>
            <small>{strategy === 'free_first' ? t('Starts with the agents most likely to be free; a paid step asks first.')
              : strategy === 'balanced' ? t('Starts with a capable agent and keeps cheaper ones as fallbacks.')
              : t('Starts with the strongest agent for hard or risky work.')}</small></span>
          <span className="group-control">
            <select id="strategy" className="popup-button" value={strategy} onChange={e => setStrategy(e.target.value as Strategy)}>
              <option value="free_first">{t('Free First')}</option>
              <option value="balanced">{t('Balanced')}</option>
              <option value="quality">{t('Quality')}</option>
            </select>
          </span>
        </div>
        {!agents.data ? <div className="group-row"><Spinner label={t('Looking for agents…')}/></div>
          : installed.length === 0 ? <div className="group-row"><span className="group-label">
            {t('No agent is installed. Install Claude Code, Codex CLI, Gemini CLI or OpenCode (see Agents) to run tasks.')}</span></div>
          : <StepEditor steps={steps ?? []} agents={installed} onChange={setSteps}/>}
        <div className="group-row">
          <span className="group-label"><label htmlFor="task-shell">{t('Shell commands')}</label>
            <small>{allowShell ? t('Agents may run commands in their copy, such as tests. Codex always runs in its own sandbox.')
              : t('Off: agents may read and edit files only. Codex always runs commands in its own sandbox.')}</small></span>
          <span className="group-control"><label className="check-row"><input id="task-shell" type="checkbox" checked={allowShell}
            onChange={e => setAllowShell(e.target.checked)}/>{t('Allow')}</label></span>
        </div>
      </div>
      <p className="group-footnote">{t('Each step uses the agent’s own sign-in, in a fresh copy of the workspace’s current commit. Tier and billing are your classing: Pygg cannot see which plan an agent uses, so its cost is recorded as unknown.')}</p>
    </section>

    {error && <ErrorNote>{error}</ErrorNote>}
    <div className="form-actions">
      <button type="button" className="button prominent" disabled={!canStart} onClick={start}>
        {busy ? <Spinner/> : <Play size={13} aria-hidden/>}{t('Start Task')}</button>
      <span className="shortcut-hint">{`${MOD}Enter`}</span>
    </div>
  </div>;
}

function WorkspacePicker({workspaces, loading, selected, onSelect, onChanged}: {
  workspaces: WorkspaceView[]; loading: boolean; selected: string; onSelect: (id: string) => void;
  onChanged: (added: WorkspaceView | null) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const showForm = adding || (!loading && workspaces.length === 0);
  const add = () => {
    setBusy(true);
    setError(null);
    call(api => api.addWorkspace(path.trim(), null)).then(
      w => { setPath(''); setAdding(false); setBusy(false); onChanged(w); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  return <section className="group">
    <h2>{t('Workspace')}</h2>
    <div className="group-box">
      {loading && <div className="group-row"><Spinner/></div>}
      {workspaces.length > 0 && <div className="group-row">
        <span className="group-label"><label htmlFor="workspace">{t('Repository')}</label>
          <small className="mono selectable">{workspaces.find(w => w.id === selected)?.path}</small></span>
        <span className="group-control">
          <select id="workspace" className="popup-button" value={selected} onChange={e => onSelect(e.target.value)}>
            {workspaces.map(w => <option key={w.id} value={w.id}>{w.name}</option>)}
          </select>
          {!adding && <button type="button" className="button" onClick={() => setAdding(true)}><Plus size={13} aria-hidden/>{t('Add…')}</button>}
        </span>
      </div>}
      {showForm && <div className="form">
        <FormRow label={t('Folder')} htmlFor="workspace-path"
          hint={t('A Git repository. Every attempt works on its own worktree of it, so your files and branch are never touched.')}>
          <span className="input-with-icon"><FolderGit2 size={13} aria-hidden/>
            <input id="workspace-path" className="text-input mono" value={path} onChange={e => setPath(e.target.value)}
              placeholder="C:\\path\\to\\repo" spellCheck={false} disabled={busy}/></span>
        </FormRow>
        <div className="form-actions">
          {workspaces.length > 0 && <button type="button" className="button" onClick={() => setAdding(false)} disabled={busy}>{t('Cancel')}</button>}
          <button type="button" className="button prominent" onClick={add} disabled={busy || !path.trim()}>{t('Add Workspace')}</button>
        </div>
        {error && <ErrorNote>{error}</ErrorNote>}
      </div>}
    </div>
  </section>;
}

/** The workspace's checks. Approving one lets it run on every attempt's changes, in that attempt's copy. */
function Checks({workspace, onChanged}: {workspace: WorkspaceView; onChanged: () => void}) {
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

function StepEditor({steps, agents, onChange}: {steps: Step[]; agents: AgentInfo[]; onChange: (steps: Step[]) => void}) {
  const update = (i: number, change: Partial<Step>) => onChange(steps.map((s, j) => j === i ? {...s, ...change} : s));
  const move = (i: number, by: number) => {
    const next = [...steps];
    [next[i], next[i + by]] = [next[i + by], next[i]];
    onChange(next);
  };
  return <>
    <ol className="step-list">
      {steps.map((s, i) => <li key={i} className="group-row step-row">
        <span className="attempt-number">{i + 1}</span>
        <select className="popup-button" aria-label={t('Agent for step {n}', {n: i + 1})} value={s.agent}
          onChange={e => update(i, {agent: e.target.value, tier: TIER_GUESS[e.target.value] ?? s.tier})}>
          {agents.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
        </select>
        <input className="text-input mono step-model" aria-label={t('Model for step {n}', {n: i + 1})} value={s.model ?? ''}
          placeholder={t('default model')} onChange={e => update(i, {model: e.target.value.trim() ? e.target.value : null})}/>
        <select className="popup-button" aria-label={t('Tier for step {n}', {n: i + 1})} value={s.tier}
          onChange={e => update(i, {tier: Number(e.target.value) as Tier})}>
          {[0, 1, 2, 3, 4].map(n => <option key={n} value={n}>T{n}</option>)}
        </select>
        <select className="popup-button" aria-label={t('Billing for step {n}', {n: i + 1})} value={s.billing}
          onChange={e => update(i, {billing: e.target.value as Billing})}>
          {BILLINGS.map(b => <option key={b} value={b}>{billingLabel(b)}</option>)}
        </select>
        <span className="step-actions">
          <button type="button" className="button small icon-only" aria-label={t('Move up')} disabled={i === 0} onClick={() => move(i, -1)}><ArrowUp size={13}/></button>
          <button type="button" className="button small icon-only" aria-label={t('Move down')} disabled={i === steps.length - 1} onClick={() => move(i, 1)}><ArrowDown size={13}/></button>
          <button type="button" className="button small icon-only" aria-label={t('Remove step {n}', {n: i + 1})} onClick={() => onChange(steps.filter((_, j) => j !== i))}><X size={13}/></button>
        </span>
      </li>)}
    </ol>
    {steps.length < MAX_STEPS && <div className="group-row">
      <button type="button" className="button" onClick={() => onChange([...steps, {agent: agents[0].id, model: null,
        tier: TIER_GUESS[agents[0].id] ?? 2, billing: 'unknown'}])}><Plus size={13} aria-hidden/>{t('Add Step')}</button>
      {steps.some(s => s.billing === 'paid') && <small className="secondary">{t('Steps marked paid wait for your approval before they run.')}</small>}
    </div>}
  </>;
}
