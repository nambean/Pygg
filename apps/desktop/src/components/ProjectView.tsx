import { useState } from 'react';
import { Check, Copy, GitBranch, MessageSquareText, Plus, Settings2 } from 'lucide-react';
import { t } from '../lib/i18n';
import { type BoardCard, type BoardSummary, type Brainstorm, type TaskSummary, type WorkspaceView } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { call, errorText } from '../lib/api';
import { Board } from './Board';
import { DiffView } from './DiffView';
import { ErrorNote, Spinner } from './ui';
import { BrainstormRow, TaskRow } from './TaskList';
import './projects.css';

type Props = {
  workspace: WorkspaceView;
  tasks: TaskSummary[];
  brainstorms: Brainstorm[];
  board: BoardSummary | null;
  onOpenTask: (task: TaskSummary) => void;
  onOpenTaskId: (id: string) => void;
  onOpenBrainstorm: (brainstorm: Brainstorm) => void;
  onOpenBrainstormId: (id: string) => void;
  onNewTask: () => void;
  onAsk: () => void;
  onSettings: () => void;
};

type Step = { name: string; state: 'todo' | 'now' | 'done'; detail: string };

/** One project (a workspace): plan → tasks → output for its brainstorms, and its solo tasks. */
export function ProjectView({workspace: w, tasks, brainstorms, board, onOpenTask, onOpenTaskId, onOpenBrainstorm, onOpenBrainstormId, onNewTask, onAsk, onSettings}: Props) {
  const [newPlan, setNewPlan] = useState(false);
  const plans = brainstorms.filter(b => b.state !== 'declined').sort((a, b) => b.createdAt - a.createdAt);
  // A plan that waits on you comes first, then the newest approved one.
  const active = plans.find(b => b.state === 'proposed' || b.state === 'waiting_pick' || b.state === 'failed')
    ?? plans.find(b => b.state === 'approved') ?? plans[0] ?? null;
  const [planId, setPlanId] = useState<string | null>(null);
  const plan = plans.find(b => b.id === planId) ?? active;
  const planTasks = plan ? tasks.filter(task => task.brainstormId === plan.id) : [];
  const solo = tasks.filter(task => !task.brainstormId);
  // Progress is the board's cards, not kept tasks: an IDE-run card has no task, a dropped one never runs.
  const boardData = useGateway(api => board && board.cards > 0 ? api.board(w.id) : Promise.resolve(null), [w.id, board?.cards ?? 0], 5_000);
  const cards: BoardCard[] = boardData.data?.cards ?? [];
  const progress = plan ? planProgress(cards.filter(c => c.brainstormId === plan.id)) : null;
  const finished = !!progress && plan?.state === 'approved' && progress.total > 0 && progress.done === progress.total;
  const boardDone = cards.length > 0 && cards.every(c => c.lane === 'done' || c.lane === 'discarded');
  const [boardOpen, setBoardOpen] = useState<boolean | null>(null);
  const showBoard = boardOpen ?? !boardDone;

  const steps: Step[] = [
    {name: t('Plan'), state: !plan ? 'now' : plan.state === 'approved' ? 'done' : 'now',
      detail: !plan ? t('No plan yet') : plan.state === 'approved' ? t('Approved · {n} cards', {n: progress?.total ?? 0})
        : plan.state === 'proposed' ? t('Proposed · review it') : plan.state === 'waiting_pick' ? t('Waits for your planner pick')
        : plan.state === 'thinking' ? t('The planner is working') : t('The planner failed')},
    {name: t('Tasks'), state: !plan || plan.state !== 'approved' ? 'todo' : finished ? 'done' : 'now',
      detail: !progress || plan?.state !== 'approved' ? t('After you approve the plan')
        : progress.total === 0 ? t('No cards on the board')
        : [t('{done} of {total} done', {done: progress.done, total: progress.total}),
           progress.running ? t('{n} running', {n: progress.running}) : null,
           progress.review ? t('{n} to review', {n: progress.review}) : null,
           progress.blocked ? t('{n} blocked', {n: progress.blocked}) : null].filter(Boolean).join(' · ')},
    {name: t('Output'), state: finished ? 'done' : 'todo',
      detail: finished ? t('A branch to review and merge') : t('When every card is done')},
  ];

  // What waits on the user, on this project's board: a paid step to approve, a result to review, a blocked card.
  type Need = {card: BoardCard; what: string; action: string | null};
  const needs = cards.flatMap((c): Need[] => {
    if (!c.taskId) return c.lane === 'blocked' && c.blockedReason ? [{card: c, what: c.blockedReason, action: null}] : [];
    if (c.task?.state === 'waiting_approval') return [{card: c, what: t('A paid step waits for your approval'), action: t('Approve…')}];
    if (c.lane === 'review') return [{card: c, what: t('Finished: review the result, then keep or discard it'), action: t('Review')}];
    if (c.lane === 'blocked') return [{card: c, what: c.blockedReason ?? t('Blocked'), action: t('Open')}];
    return [];
  });

  return <div className="projects project">
    <div className="project-head">
      <div>
        <p className="project-path mono selectable">{w.path}</p>
      </div>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onSettings}><Settings2 size={13} aria-hidden/>{t('Project settings')}</button>
      <button type="button" className="button" onClick={onAsk}><MessageSquareText size={13} aria-hidden/>{t('Ask about this project')}</button>
      <button type="button" className="button" onClick={onNewTask}><Plus size={13} aria-hidden/>{t('New solo task')}</button>
    </div>

    <ol className="project-steps" aria-label={t('Where this project is')}>
      {steps.map((s, i) => <li key={s.name} className={`step ${s.state}`} aria-current={s.state === 'now' ? 'step' : undefined}>
        <span className="step-number">{s.state === 'done' ? <Check size={13} strokeWidth={2.6} aria-hidden/> : i + 1}</span>
        <span className="step-text"><span className="step-name">{s.name}</span><span className="step-detail">{s.detail}</span></span>
      </li>)}
    </ol>

    {needs.length > 0 && <section className="needs-you" aria-labelledby="needs-you">
      <h2 id="needs-you">{t('Waiting for you · {n}', {n: needs.length})}</h2>
      <ul>{needs.map(({card, what, action}) => <li key={card.id}>
        <span className="needs-title">{card.title}</span><span className="needs-what">{what}</span>
        {action && card.taskId && <button type="button" className="button prominent" onClick={() => onOpenTaskId(card.taskId!)}>{action}</button>}
      </li>)}</ul>
    </section>}

    <section className="task-section" aria-labelledby="project-plan">
      <div className="section-heading">
        <h2 id="project-plan">{t('1 · Plan')}</h2>
        {plans.length > 1 && <label className="plan-picker">{t('Plan')}
          <select value={plan?.id ?? ''} onChange={e => setPlanId(e.target.value)}>
            {plans.map(b => <option key={b.id} value={b.id}>{b.requirement.split('\n')[0].slice(0, 70)} · {new Date(b.createdAt).toLocaleDateString()}</option>)}
          </select></label>}
        {plans.length > 0 && !newPlan && <button type="button" className="button" onClick={() => setNewPlan(true)}><Plus size={13} aria-hidden/>{t('New plan')}</button>}
      </div>
      {(plans.length === 0 || newPlan) && <StartPlan workspaceId={w.id} onStarted={id => { setNewPlan(false); onOpenBrainstormId(id); }}
        onCancel={plans.length > 0 ? () => setNewPlan(false) : undefined}/>}
      {plans.length === 0 ? <p className="group-footnote">{t('Or skip planning and run one solo task.')}</p> : <ul className="task-list">
        {plans.map(b => <BrainstormRow key={b.id} brainstorm={b} attention={b.state === 'proposed' || b.state === 'waiting_pick' || b.state === 'failed'}
          onOpen={onOpenBrainstorm} project={b.id === plan?.id && plans.length > 1 ? t('shown below') : null}/>)}
      </ul>}
    </section>

    <section className="task-section" aria-labelledby="project-tasks">
      <h2 id="project-tasks">{t('2 · Tasks')}</h2>
      {board && board.cards > 0 ? <>
        {boardDone && <p className="group-footnote">{t('Every card on this project’s board is done.')}{' '}
          <button type="button" className="link-button" onClick={() => setBoardOpen(!showBoard)}>{showBoard ? t('Hide the board') : t('Show the board')}</button></p>}
        {showBoard && <Board workspaceId={w.id} workspaces={[{workspaceId: w.id, workspaceName: w.name}]} onWorkspace={() => {}} onOpenTask={onOpenTaskId}/>}
      </> : <p className="group-footnote">{t('Approved plans put their cards on this project’s board: Pygg runs them one at a time, each checked before the next starts.')}</p>}
      {solo.length > 0 && <>
        <h3 className="subheading">{t('Solo tasks in this project')}</h3>
        <ul className="task-list">
          {solo.map(task => <TaskRow key={task.id} task={task} attention={false} workspace={null} onOpen={onOpenTask}/>)}
        </ul>
      </>}
    </section>

    <section className="task-section" aria-labelledby="project-output">
      <h2 id="project-output">{t('3 · Output')}</h2>
      {plan?.state === 'approved' && plan.branch && progress ? <PlanOutput plan={plan} tasks={planTasks} progress={progress}/>
        : <p className="group-footnote">{t('The output of an approved plan is one branch with every card’s work on it, ready for you to review and merge. Nothing is merged for you.')}</p>}
      {solo.some(task => task.result === 'kept') && <>
        <h3 className="subheading">{t('Kept solo results')}</h3>
        <ul className="output-list">
          {solo.filter(task => task.result === 'kept' && task.resultBranch).slice(0, 8).map(task =>
            <li key={task.id}><GitBranch size={13} aria-hidden/><button type="button" className="link-button" onClick={() => onOpenTask(task)}>{task.title}</button>
              <CopyText text={task.resultBranch!}/></li>)}
        </ul>
      </>}
    </section>
  </div>;
}

/** A plan's progress from its cards on the board (dropped ones left out). */
function planProgress(cards: BoardCard[]) {
  const live = cards.filter(c => c.lane !== 'discarded');
  const count = (lane: string) => live.filter(c => c.lane === lane).length;
  return {total: live.length, done: count('done'), running: count('running') + count('checking'), review: count('review'), blocked: count('blocked')};
}

function PlanOutput({plan, tasks, progress}: {plan: Brainstorm; tasks: TaskSummary[]; progress: {total: number; done: number}}) {
  const apart = tasks.filter(task => task.result === 'kept' && task.resultBranch && task.resultBranch !== plan.branch);
  const finished = progress.total > 0 && progress.done === progress.total;
  return <div className="plan-output">
    <p className="output-line"><GitBranch size={15} aria-hidden/>
      <span>{finished
        ? apart.length ? t('Every card is done. The plan’s branch has all but {n} of them:', {n: apart.length}) : t('Every card is done. The work is on one branch:')
        : t('{done} of {total} cards done so far; the plan’s branch:', {done: progress.done, total: progress.total})}</span>
      <CopyText text={plan.branch!}/></p>
    <p className="group-footnote">{t('Review it, then merge it into your branch when you are happy:')}</p>
    <pre className="selectable">{`git log --oneline ${plan.branch}\ngit diff HEAD...${plan.branch}\ngit merge ${plan.branch}`}</pre>
    <PlanChanges id={plan.id}/>
    {apart.length > 0 && <div className="output-apart">
      <p><strong>{t('{n} results on branches of their own:', {n: apart.length})}</strong> {t('their work could not be added to the plan’s branch (it conflicted with a card that ran beside it, or the branch was checked out). Review each and merge it by hand.')}</p>
      <ul className="output-list">{apart.map(task => <li key={task.id}><GitBranch size={13} aria-hidden/>{task.title}<CopyText text={task.resultBranch!}/></li>)}</ul>
    </div>}
  </div>;
}

function CopyText({text}: {text: string}) {
  const [copied, setCopied] = useState(false);
  return <span className="copy-text"><code className="selectable">{text}</code>
    <button type="button" className="button small" aria-label={t('Copy {text}', {text})} title={t('Copy')}
      onClick={() => { void navigator.clipboard?.writeText(text); setCopied(true); setTimeout(() => setCopied(false), 1500); }}>
      {copied ? <Check size={12} aria-hidden/> : <Copy size={12} aria-hidden/>}</button></span>;
}

/** The plan's combined changes, loaded when asked (it can be large). */
function PlanChanges({id}: {id: string}) {
  const [diff, setDiff] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const show = () => {
    const opening = !open;
    setOpen(opening);
    if (opening) { setDiff(null); setError(null); call(api => api.brainstormDiff(id)).then(setDiff, e => setError(errorText(e))); }
  };
  return <div className="plan-changes">
    <button type="button" className="button" onClick={show}>{open ? t('Hide the changes') : t('Review the changes')}</button>
    {open && (error ? <ErrorNote>{error}</ErrorNote> : diff === null ? <Spinner label={t('Loading changes…')}/>
      : diff.trim() === '' ? <p className="group-footnote">{t('No changes against your branch: it may be merged already.')}</p> : <DiffView diff={diff}/>)}
  </div>;
}

/** A new plan for this project: the requirement goes to Pygg's planner (Brain Storming), which splits it into cards
 *  for review here. The same plan can start in a coding agent with the model pygg/brain. */
function StartPlan({workspaceId, onStarted, onCancel}: {workspaceId: string; onStarted: (id: string) => void; onCancel?: () => void}) {
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const start = () => {
    setBusy(true); setError(null);
    call(api => api.brainstormStart(workspaceId, text)).then(b => { setBusy(false); setText(''); onStarted(b.id); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  return <form className="start-plan" onSubmit={e => { e.preventDefault(); if (text.trim() && !busy) start(); }}>
    <label htmlFor="plan-requirement"><strong>{t('What should be built?')}</strong></label>
    <textarea id="plan-requirement" className="plan-requirement" rows={4} value={text} onChange={e => setText(e.target.value)}
      placeholder={t('Describe the feature or change as you would to a colleague: what it does, for whom, what must not break.')}/>
    <div className="form-actions">
      <button type="submit" className="button prominent" disabled={busy || !text.trim()}>{busy ? t('Starting…') : t('Start planning')}</button>
      {onCancel && <button type="button" className="button" onClick={onCancel} disabled={busy}>{t('Cancel')}</button>}
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
    <p className="group-footnote">{t('Pygg’s planner reads only what you write here, not your files. It splits the work into cards with a role each; you review, answer its questions and approve before anything runs. A paid planner waits for your pick.')}</p>
    <details className="form-disclosure"><summary>{t('Or start it from your coding agent')}</summary>
      <p className="group-footnote">{t('Ask your agent, in this repository, to use the model pygg/brain with the requirement; the proposal appears here the same way.')}</p>
      <pre className="selectable">{t('Use the model pygg/brain: <describe what to build>')}</pre></details>
  </form>;
}
