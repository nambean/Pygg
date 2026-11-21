import { useEffect, useMemo, useState } from 'react';
import { ArrowDown, ArrowUp, Play, Plus, X } from 'lucide-react';
import { call, errorText, gatewayChanged, type AgentInfo } from '../lib/api';
import { t } from '../lib/i18n';
import { MOD } from '../lib/platform';
import { MAX_STEPS, type Billing, type Hold, type LadderEstimate, type Sandbox, type Step, type Strategy, type Tier, type WorkspaceView } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { billingLabel } from './AttemptSequence';
import { ErrorNote, rovingIndex, Spinner } from './ui';
import { AddWorkspaceForm, WorkspaceChecks } from './WorkspacesView';

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

export type WorkMode = 'ask' | 'task' | 'plan';

/**
 * New work (SCREENS.md §5.2.1): one form for the three kinds of work in a project. Ask: an agent reads the repository
 * and answers. Task: one change by a ladder of agent steps, each in a fresh worktree, checked, kept as a branch.
 * Plan: Brain Storming turns a larger requirement into cards to review and approve. A paid step waits for approval.
 */
export function NewTaskView({onCreated, onPlanCreated, workspaceId: initial, mode, onMode}: {
  onCreated: (id: string) => void; onPlanCreated: (id: string) => void; workspaceId?: string | null; mode: WorkMode;
  onMode: (mode: WorkMode) => void;
}) {
  const workspaces = useGateway(api => api.workspaces());
  const agents = useGateway(api => api.agents());
  const [workspaceId, setWorkspaceId] = useState(initial ?? '');
  const [text, setText] = useState('');
  const [strategy, setStrategy] = useState<Strategy>('free_first');
  const [steps, setSteps] = useState<Step[] | null>(null);
  const [allowShell, setAllowShell] = useState(false);
  const [allowWeb, setAllowWeb] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [capText, setCapText] = useState('');
  const answer = mode === 'ask';
  const stepsKey = JSON.stringify(steps ?? []);
  const estimate = useGateway(api => steps && steps.length && mode !== 'plan' ? api.taskEstimate(steps) : Promise.resolve(null), [stepsKey, mode]);
  const agentIds = [...new Set((steps ?? []).map(s => s.agent))];
  const sandbox = useGateway(api => agentIds.length && mode !== 'plan' ? api.sandboxProfiles(agentIds, allowShell && mode === 'task', allowWeb) : Promise.resolve(null),
    [agentIds.join(','), allowShell, allowWeb, mode]);
  const cap = capText.trim() === '' ? null : Number(capText.replace(',', '.'));
  const capValid = cap === null || (Number.isFinite(cap) && cap > 0);

  const list = workspaces.data ?? [];
  const workspace = list.find(w => w.id === workspaceId);
  useEffect(() => { if (!workspaceId && list.length > 0) setWorkspaceId(list[0].id); }, [list, workspaceId]);
  const installed = useMemo(() => (agents.data ?? []).filter(a => a.installed), [agents.data]);
  useEffect(() => { if (agents.data) setSteps(suggestedSteps(strategy, agents.data)); }, [agents.data, strategy]);

  const canStart = !!workspace && text.trim() !== '' && (mode === 'plan' || (steps?.length ?? 0) > 0) && capValid && !busy;
  const start = () => {
    if (!canStart || !workspace) return;
    setBusy(true);
    setError(null);
    if (mode === 'plan') {
      call(api => api.brainstormStart(workspace.id, text)).then(b => { gatewayChanged(); onPlanCreated(b.id); },
        e => { setError(errorText(e)); setBusy(false); });
      return;
    }
    if (!steps) return;
    call(api => api.createTask({workspaceId: workspace.id, title: null, prompt: text.trim(), strategy, steps, allowShell: allowShell && mode === 'task', timeoutSecs: null, answer, capUsd: cap, allowWeb}))
      .then(id => { gatewayChanged(); onCreated(id); }, e => { setError(errorText(e)); setBusy(false); });
  };
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') { event.preventDefault(); start(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });
  const ladder = (steps ?? []).map((s, i) => `${i + 1}. ${installed.find(a => a.id === s.agent)?.name ?? s.agent}`).join(' → ');
  const kinds: [WorkMode, string, string][] = [
    ['ask', t('Ask'), t('A question about the code. An agent reads and answers; nothing changes.')],
    ['task', t('Task'), t('One change. An agent makes it on a copy, the checks test it, you keep it as a branch.')],
    ['plan', t('Plan'), t('A larger change. Brain Storming splits it into cards you review and approve, then runs them.')],
  ];

  return <div className="settings new-task">
    <section className="group">
      <h2 id="work-kind-label">{t('What do you want?')}</h2>
      <div className="work-kinds" role="radiogroup" aria-labelledby="work-kind-label" onKeyDown={e => {
        const next = rovingIndex(e.key, kinds.findIndex(k => k[0] === mode), kinds.length);
        if (next === null) return;
        e.preventDefault();
        onMode(kinds[next][0]);
        (e.currentTarget.children[next] as HTMLElement | undefined)?.focus();
      }}>
        {kinds.map(([value, label, hint]) => <button key={value} type="button" role="radio" aria-checked={mode === value} tabIndex={mode === value ? 0 : -1}
          className={`work-kind${mode === value ? ' selected' : ''}`} onClick={() => onMode(value)}>
          <strong>{label}</strong><small>{hint}</small></button>)}
      </div>
    </section>

    {workspaces.error && !workspaces.data
      ? <LoadFailed what={t('Projects could not be loaded: {error}', {error: workspaces.error})} retrying={workspaces.loading} onRetry={workspaces.reload}/>
      : <WorkspacePicker workspaces={list} loading={!workspaces.data} selected={workspaceId}
        onSelect={setWorkspaceId} onChanged={w => { workspaces.reload(); if (w) setWorkspaceId(w.id); }}/>}

    <section className="group">
      <h2><label htmlFor="task-text">{mode === 'ask' ? t('Question') : mode === 'task' ? t('Task') : t('What should be built?')}</label></h2>
      <textarea id="task-text" className="task-input" autoFocus value={text} onChange={e => setText(e.target.value)} rows={5}
        placeholder={mode === 'ask' ? t('Ask about the code, e.g. “How does the app start, and where is the database opened?”')
          : mode === 'task' ? t('Describe the change, e.g. “Fix timeout handling and add regression tests.”')
          : t('Describe the feature or change as you would to a colleague: what it does, for whom, what must not break.')}/>
      {mode === 'plan' && <p className="group-footnote">{t('Pygg’s planner reads only what you write here, not your files. It may ask questions, then proposes cards with a role and a model each; nothing runs until you approve. A paid planner waits for your pick.')}</p>}
    </section>

    {mode === 'task' && workspace && <WorkspaceChecks workspace={workspace} onChanged={workspaces.reload}/>}

    {mode !== 'plan' && <CostPanel estimate={estimate.data && estimate.data.steps.length === (steps ?? []).length ? estimate.data : null}
      steps={steps ?? []} names={Object.fromEntries(installed.map(a => [a.id, a.name]))}
      capText={capText} onCap={setCapText} capValid={capValid} updating={estimate.loading}/>}

    {mode !== 'plan' && sandbox.data && <SafetyPanel profiles={sandbox.data.filter(p => agentIds.includes(p.agent))} updating={sandbox.loading}
      names={Object.fromEntries(installed.map(a => [a.id, a.name]))}/>}

    {mode !== 'plan' && <details className="form-disclosure agent-settings">
      <summary>{t('Agent settings')}<span className="secondary"> · {ladder || t('no agent installed')}</span></summary>
      <section className="group">
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
          {!agents.data && agents.error ? <div className="group-row">
              <LoadFailed what={t('Agents could not be listed: {error}', {error: agents.error})} retrying={agents.loading} onRetry={agents.reload}/></div>
            : !agents.data ? <div className="group-row"><Spinner label={t('Looking for agents…')}/></div>
            : installed.length === 0 ? <div className="group-row"><span className="group-label">
              {t('No agent is installed. Install Claude Code, Codex CLI, Gemini CLI or OpenCode (see Agents) to run tasks.')}</span></div>
            : <StepEditor steps={steps ?? []} agents={installed} onChange={setSteps}/>}
          <div className="group-row">
            <span className="group-label"><label htmlFor="task-web">{t('Web access')}</label>
              <small>{allowWeb ? t('Agents may fetch pages and search the web while they work.') : t('Off: agents work from the repository alone.')}</small></span>
            <span className="group-control"><label className="check-row"><input id="task-web" type="checkbox" checked={allowWeb}
              onChange={e => setAllowWeb(e.target.checked)}/>{t('Allow')}</label></span>
          </div>
          {mode === 'task' && <div className="group-row">
            <span className="group-label"><label htmlFor="task-shell">{t('Shell commands')}</label>
              <small>{allowShell ? t('Agents may run commands in their copy, such as tests. Codex always runs in its own sandbox.')
                : t('Off: agents may read and edit files only. Codex always runs commands in its own sandbox.')}</small></span>
            <span className="group-control"><label className="check-row"><input id="task-shell" type="checkbox" checked={allowShell}
              onChange={e => setAllowShell(e.target.checked)}/>{t('Allow')}</label></span>
          </div>}
        </div>
        <p className="group-footnote">{t('Each step uses the agent’s own sign-in, in a fresh copy of the project’s current commit. If a step fails, the next one tries. Tier and billing are your classing: Pygg cannot see which plan an agent uses.')}</p>
      </section>
    </details>}

    {error && <ErrorNote>{error}</ErrorNote>}
    <div className="form-actions">
      <button type="button" className="button prominent" disabled={!canStart} onClick={start}>
        {busy ? <Spinner/> : <Play size={13} aria-hidden/>}{mode === 'ask' ? t('Ask') : mode === 'task' ? t('Start task') : t('Create plan')}</button>
      <span className="shortcut-hint">{`${MOD}Enter`}</span>
    </div>
  </div>;
}

/** A load that failed, with a way to try again, so it never looks like endless loading or missing setup. */
function LoadFailed({what, retrying, onRetry}: {what: string; retrying: boolean; onRetry: () => void}) {
  return <ErrorNote>{what}{' '}<button type="button" className="link-button" onClick={onRetry} disabled={retrying}>
    {retrying ? t('Trying again…') : t('Try Again')}</button></ErrorNote>;
}

function WorkspacePicker({workspaces, loading, selected, onSelect, onChanged}: {
  workspaces: WorkspaceView[]; loading: boolean; selected: string; onSelect: (id: string) => void;
  onChanged: (added: WorkspaceView | null) => void;
}) {
  const [adding, setAdding] = useState(false);
  const showForm = adding || (!loading && workspaces.length === 0);
  return <section className="group">
    <h2>{t('Project')}</h2>
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
      {showForm && <AddWorkspaceForm onAdded={w => { setAdding(false); onChanged(w); }}
        onCancel={workspaces.length > 0 ? () => setAdding(false) : null}/>}
    </div>
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

const dollars = (micro: number) => `$${(micro / 1e6).toFixed(micro > 0 && micro < 10_000 ? 3 : 2)}`;

/**
 * What the task may cost before it starts (the gap study's task budget): each step of the ladder with what its agent
 * and model cost on recent runs, the stop rules, and an optional cap for the whole task.
 */
function CostPanel({estimate, steps, names, capText, onCap, capValid, updating}: {estimate: LadderEstimate | null; steps: Step[];
  names: Record<string, string>; capText: string; onCap: (v: string) => void; capValid: boolean; updating: boolean}) {
  const priced = (e: LadderEstimate['steps'][number]) => e.kind === 'free' ? t('free') : e.kind === 'quota' ? t('your plan’s quota')
    : e.kind === 'unknown' ? t('unknown: never run') : e.lowMicroUsd === e.highMicroUsd ? t('≈{usd} ({n} runs)', {usd: dollars(e.highMicroUsd ?? 0), n: e.runs})
    : t('{low}–{high} ({n} runs)', {low: dollars(e.lowMicroUsd ?? 0), high: dollars(e.highMicroUsd ?? 0), n: e.runs});
  const first = !estimate ? null : estimate.firstHighMicroUsd === null ? t('unknown') : estimate.firstHighMicroUsd === 0 ? t('nothing')
    : estimate.firstLowMicroUsd === estimate.firstHighMicroUsd ? `≈${dollars(estimate.firstHighMicroUsd)}`
    : `${dollars(estimate.firstLowMicroUsd ?? 0)}–${dollars(estimate.firstHighMicroUsd)}`;
  return <section className="group cost-panel">
    <h2>{t('Cost')}{updating && estimate && <span className="brain-meta"> · {t('updating…')}</span>}</h2>
    <div className={updating && estimate ? 'group-box stale' : 'group-box'} aria-busy={updating}>
      {!estimate ? <div className="group-row"><Spinner label={t('Estimating…')}/></div> : <>
        <div className="group-row cost-lines">
          <span>{t('If the first step succeeds: {cost}', {cost: first ?? ''})}</span>
          <span>{estimate.unknownSteps > 0
            ? t('If every step runs: the steps with a history cost up to {cost} on past runs, plus {n} never run, unknown', {cost: dollars(estimate.allHighMicroUsd), n: estimate.unknownSteps})
            : t('If every step runs: up to {cost} on past runs', {cost: dollars(estimate.allHighMicroUsd)})}</span>
        </div>
        <ol className="ladder-list">
          {estimate.steps.map((e, i) => <li key={i}><span className="mono">{i + 1}. {names[steps[i]?.agent] ?? steps[i]?.agent}{steps[i]?.model ? ` · ${steps[i].model}` : ''}</span>
            <span className={e.kind === 'unknown' ? 'warn-text' : 'secondary'}>{priced(e)}</span></li>)}
        </ol>
        <p className="group-footnote">{t('The next step starts only when a step fails: the checks fail, it changes nothing, or the agent stops. A paid step asks you first. Estimates are what the same agent and model cost on earlier attempts, not a quote or a ceiling.')}</p>
      </>}
      <div className="group-row">
        <span className="group-label"><label htmlFor="task-cap">{t('Stop the task at')}</label>
          <small id="task-cap-hint">{capValid ? t('Optional. No further step starts once the task has cost this much, or once a paid step’s cost is unknown; a paid step’s cap is never above what is left. Only Claude Code stops itself mid-run at a cap: another agent’s step may go over, and the task stops after it.')
            : t('Enter an amount above 0, or leave it empty.')}</small></span>
        <span className="group-control"><span className={capValid ? 'money-input' : 'money-input invalid'}><span aria-hidden>$</span>
          <input id="task-cap" inputMode="decimal" placeholder={t('no cap')} value={capText} onChange={e => onCap(e.target.value)}
            aria-invalid={!capValid} aria-describedby="task-cap-hint"/></span></span>
      </div>
    </div>
  </section>;
}

const HOLD: Record<Hold, string> = {blocked: 'blocked', allowed: 'allowed by you', not_guaranteed: 'not guaranteed', partial: 'filtered, best effort', sandboxed: 'in its sandbox'};

/**
 * What the agents of this ladder are held to (the sandbox profile): files outside the copy, shell, web, and secrets
 * in the environment, per agent, with what Pygg cannot enforce said plainly.
 */
function SafetyPanel({profiles, names, updating}: {profiles: Sandbox[]; names: Record<string, string>; updating: boolean}) {
  const loose = profiles.some(p => [p.writesOutside, p.readsOutside, p.shell, p.network].includes('not_guaranteed'));
  return <section className="group safety-panel">
    <h2>{t('Safety')}{updating && <span className="brain-meta"> · {t('updating…')}</span>}</h2>
    <div className={updating ? 'group-box stale' : 'group-box'} aria-busy={updating}>
      <table className="safety-table">
        <thead><tr><th scope="col">{t('Agent')}</th><th scope="col">{t('Writes outside its copy')}</th><th scope="col">{t('Reads outside')}</th>
          <th scope="col">{t('Shell')}</th><th scope="col">{t('Network')}</th><th scope="col">{t('Secrets in the environment')}</th></tr></thead>
        <tbody>{profiles.map(p => <tr key={p.agent}><th scope="row">{names[p.agent] ?? p.agent}</th>
          {[p.writesOutside, p.readsOutside, p.shell, p.network, p.secrets].map((h, i) => <td key={i} className={`hold ${h}`}>{t(HOLD[h])}</td>)}</tr>)}</tbody>
      </table>
      {profiles.filter(p => p.note).map(p => <p key={p.agent} className="group-footnote safety-note">{names[p.agent] ?? p.agent}: {p.note}</p>)}
    </div>
    <p className="group-footnote">{t('Every attempt works on its own copy of the repository; your checkout is never changed. Variables in Pygg’s environment that look like secrets and that the agent’s own sign-in does not use (cloud, GitHub, database keys, URLs with a password) are not passed to it; secrets in files and credential stores are not covered.')}
      {loose && ` ${t('Not guaranteed means Pygg cannot hold that agent to it here: an allowed shell reaches past tool rules. Keep secrets out of the repository, or choose another agent.')}`}</p>
  </section>;
}
