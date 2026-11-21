import { useState } from 'react';
import { Lightbulb, MessageSquareText, Plus, Search } from 'lucide-react';
import { ago } from '../lib/format';
import { t } from '../lib/i18n';
import { brainstormNeedsAttention, needsAttention, type Brainstorm, type PlanProgress, type TaskSummary, type WorkspaceView } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { StateIcon, statusLine } from './TaskList';
import { AttemptSequence } from './AttemptSequence';
import { taskCost } from '../lib/format';
import './projects.css';

type Props = {
  tasks: TaskSummary[];
  brainstorms: Brainstorm[];
  workspaces: WorkspaceView[];
  /** Set on a project's page: only that project's work, without the project name on each row. */
  project?: string | null;
  onOpenTask: (task: TaskSummary) => void;
  onOpenPlan: (plan: Brainstorm) => void;
  onNewWork: () => void;
};

type Item =
  | {kind: 'plan'; plan: Brainstorm; progress: PlanProgress | null; at: number; needsYou: boolean; running: boolean}
  | {kind: 'task'; task: TaskSummary; at: number; needsYou: boolean; running: boolean};

const RECENT = 25;

/**
 * Work (SCREENS.md §5.0): every plan, task and question across projects, in three sections: what needs you, what is
 * running, and the rest by time. A plan's cards live under their plan; a task started from an IDE shows its source.
 */
export function WorkList({tasks, brainstorms, workspaces, project, onOpenTask, onOpenPlan, onNewWork}: Props) {
  const [query, setQuery] = useState('');
  const [all, setAll] = useState(false);
  const progress = useGateway(api => api.plansProgress(), [], 5_000);
  const progressOf = (id: string) => progress.data?.find(p => p.brainstormId === id) ?? null;
  const needle = query.trim().toLowerCase();
  const name = (id: string | null) => workspaces.find(w => w.id === id)?.name ?? '';
  const scoped = (workspaceId: string | null) => !project || workspaceId === project;

  const items: Item[] = [
    ...brainstorms.filter(b => b.state !== 'declined' && scoped(b.workspaceId)).map((b): Item => {
      const cards = tasks.filter(task => task.brainstormId === b.id);
      const p = progressOf(b.id);
      return {kind: 'plan', plan: b, progress: p, at: Math.max(b.updatedAt, ...cards.map(c => c.updatedAt)),
        needsYou: brainstormNeedsAttention(b) || cards.some(needsAttention) || (p?.waiting ?? 0) > 0,
        running: b.state === 'thinking' || cards.some(c => c.state === 'running') || (p?.running ?? 0) > 0};
    }),
    ...tasks.filter(task => !task.brainstormId && scoped(task.workspaceId)).map((task): Item =>
      ({kind: 'task', task, at: task.updatedAt, needsYou: needsAttention(task), running: task.state === 'running'})),
  ].filter(item => !needle || (item.kind === 'plan' ? item.plan.requirement : item.task.title).toLowerCase().includes(needle))
    .sort((a, b) => b.at - a.at);

  const needsYou = items.filter(i => i.needsYou);
  const running = items.filter(i => !i.needsYou && i.running);
  const rest = items.filter(i => !i.needsYou && !i.running);
  const shown = all ? rest : rest.slice(0, RECENT);

  const row = (item: Item) => item.kind === 'plan'
    ? <PlanRow key={item.plan.id} item={item} project={project ? null : name(item.plan.workspaceId)} onOpen={() => onOpenPlan(item.plan)}/>
    : <WorkTaskRow key={item.task.id} task={item.task} attention={item.needsYou} project={project ? null : name(item.task.workspaceId)} onOpen={() => onOpenTask(item.task)}/>;

  return <div className="work">
    <div className="list-bar">
      <label className="search-field">
        <Search size={14} strokeWidth={2} aria-hidden/>
        <input type="search" value={query} onChange={e => setQuery(e.target.value)} placeholder={t('Search work')} aria-label={t('Search work')}/>
      </label>
    </div>

    {items.length === 0 ? <div className="placeholder">
      <h2>{needle ? t('Nothing matches “{query}”', {query}) : t('No work yet')}</h2>
      {!needle && <p>{t('Ask about a project, give it one task, or plan a larger change with Brain Storming.')}</p>}
      {!needle && <button type="button" className="button prominent" onClick={onNewWork}><Plus size={13} aria-hidden/>{t('New work')}</button>}
    </div> : <>
      {needsYou.length > 0 && <section className="task-section" aria-labelledby="work-needs-you">
        <h2 id="work-needs-you">{t('Needs you · {n}', {n: needsYou.length})}</h2>
        <ul className="task-list">{needsYou.map(row)}</ul>
      </section>}
      {running.length > 0 && <section className="task-section" aria-labelledby="work-running">
        <h2 id="work-running">{t('Running · {n}', {n: running.length})}</h2>
        <ul className="task-list">{running.map(row)}</ul>
      </section>}
      {rest.length > 0 && <section className="task-section" aria-labelledby="work-recent">
        <h2 id="work-recent">{t('Recent')}</h2>
        <ul className="task-list">{shown.map(row)}</ul>
        {rest.length > shown.length && <button type="button" className="link-button" onClick={() => setAll(true)}>
          {t('Show {n} more', {n: rest.length - shown.length})}</button>}
      </section>}
    </>}
  </div>;
}

/** A plan: its requirement, where it stands, and its cards' progress once approved. */
function PlanRow({item, project, onOpen}: {item: Extract<Item, {kind: 'plan'}>; project: string | null; onOpen: () => void}) {
  const b = item.plan;
  const p = item.progress;
  const cards = p?.total ?? b.proposal.cards.filter(c => !c.dropped).length;
  const done = p?.done ?? 0;
  const waiting = p?.waiting ?? 0;
  const status = b.state === 'waiting_pick' ? t('Choose a planner to start')
    : b.state === 'thinking' ? t('The planner is working')
    : b.state === 'failed' ? t('The planner failed')
    : b.state === 'proposed' ? t('Proposed · {n} cards to review', {n: cards})
    : waiting > 0 ? t('{done} of {total} cards done · {n} waiting for you', {done, total: cards, n: waiting})
    : done >= cards && cards > 0 ? t('Every card done · output ready')
    : t('{done} of {total} cards done', {done, total: cards}) + (p?.running ? ` · ${t('{n} running', {n: p.running})}` : '');
  return <li className={`task-row${item.needsYou ? ' attention' : ''}`}>
    <span className={`state-icon ${b.state === 'failed' ? 'check_failed' : b.state === 'approved' ? 'checks_passed' : 'waiting_approval'}`}>
      <Lightbulb size={18} strokeWidth={1.9} aria-hidden/></span>
    <div className="row-body">
      <div className="row-title">
        <span className="kind-chip plan">{t('Plan')}</span>
        <span className="name">{b.requirement.split('\n')[0]}</span>
        {project && <span className="host">{project}</span>}
        {!b.session.startsWith('app-') && b.clientId && <span className="host">{t('from your coding agent')}</span>}
      </div>
      <div className="row-status">{status} · {ago(item.at)}</div>
    </div>
    <div className="row-actions">
      <button type="button" className={item.needsYou ? 'button prominent' : 'button'} onClick={onOpen}
        aria-label={`${item.needsYou ? t('Review') : t('Open')}: ${b.requirement.split('\n')[0]}`}>
        {item.needsYou ? t('Review') : t('Open')}</button>
    </div>
  </li>;
}

/** A solo task or a question (Ask). */
function WorkTaskRow({task, attention, project, onOpen}: {task: TaskSummary; attention: boolean; project: string | null; onOpen: () => void}) {
  return <li className={`task-row${attention ? ' attention' : ''}`}>
    {task.answer ? <span className={`state-icon ${task.state}`}><MessageSquareText size={18} strokeWidth={1.9} aria-hidden/></span>
      : <StateIcon state={task.state}/>}
    <div className="row-body">
      <div className="row-title">
        <span className={`kind-chip ${task.answer ? 'ask' : 'task'}`}>{task.answer ? t('Ask') : t('Task')}</span>
        <span className="name">{task.title}</span>
        {project && <span className="host">{project}</span>}
        {task.source && <span className="host">{t('from {client}', {client: task.source})}</span>}
      </div>
      <div className="row-status">{statusLine(task)}</div>
      {!task.answer && <div className="row-meta">
        <AttemptSequence attempts={task.attempts}/>
        <span className="row-cost">{taskCost(task.cost)}</span>
      </div>}
    </div>
    <div className="row-actions">
      <button type="button" className={attention ? 'button prominent' : 'button'} onClick={onOpen}
        aria-label={`${attention ? t('Review') : t('Open')}: ${task.title}`}>
        {attention ? t('Review') : t('Open')}</button>
    </div>
  </li>;
}
