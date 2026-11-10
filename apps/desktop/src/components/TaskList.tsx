import { Ban, CircleAlert, CircleCheck, CircleX, Hourglass, LoaderCircle, RefreshCw, Search, TriangleAlert,
  UserCheck } from 'lucide-react';
import type { RefObject } from 'react';
import { ago, elapsed, taskCost } from '../lib/format';
import { t } from '../lib/i18n';
import { matchesFilter, needsAttention, type TaskFilter, type TaskState, type TaskSummary, type Workspace } from '../lib/tasks';
import { AttemptSequence } from './AttemptSequence';

type Props = {
  tasks: TaskSummary[];
  filter: TaskFilter;
  workspaces: Workspace[];
  /** Set when one workspace is selected: rows then leave out the workspace name. */
  scoped: boolean;
  query: string;
  onQuery: (query: string) => void;
  searchRef: RefObject<HTMLInputElement | null>;
  onOpen: (task: TaskSummary) => void;
  onNewTask: () => void;
  onWorkspaceSettings: (() => void) | null;
};

/** Home screen (SCREENS.md §5.1): items that wait on the user pinned on top, then the filtered list. */
export function TaskList({tasks, filter, workspaces, scoped, query, onQuery, searchRef, onOpen, onNewTask,
  onWorkspaceSettings}: Props) {
  const needle = query.trim().toLowerCase();
  const visible = tasks.filter(task => matchesFilter(task, filter)
    && (!needle || task.title.toLowerCase().includes(needle)));
  // Only All Tasks pins attention items; the other filters are plain views of their own subset.
  const pinned = filter === 'all' ? visible.filter(needsAttention) : [];
  const rest = filter === 'all' ? visible.filter(task => !needsAttention(task)) : visible;
  const workspaceName = (id: string) => workspaces.find(w => w.id === id)?.name ?? '';

  return <div className="task-view">
    <div className="list-bar">
      <label className="search-field">
        <Search size={14} strokeWidth={2} aria-hidden/>
        <input ref={searchRef} type="search" value={query} onChange={event => onQuery(event.target.value)}
          placeholder={t('Search Tasks')} aria-label={t('Search Tasks')}/>
      </label>
      <span className="spacer"/>
      {onWorkspaceSettings && <button type="button" className="button" onClick={onWorkspaceSettings}>
        {t('Workspace Settings…')}</button>}
    </div>

    {visible.length === 0 ? <EmptyState filter={filter} query={query} hasTasks={tasks.length > 0}
      onClear={() => onQuery('')} onNewTask={onNewTask}/> : <>
      {pinned.length > 0 && <section className="task-section" aria-labelledby="attention-heading">
        <h2 id="attention-heading">{t('Needs Attention')}</h2>
        <ul className="task-list">
          {pinned.map(task => <TaskRow key={task.id} task={task} attention
            workspace={scoped ? null : workspaceName(task.workspaceId)} onOpen={onOpen}/>)}
        </ul>
      </section>}
      {rest.length > 0 && <section className="task-section" aria-labelledby="tasks-heading">
        {pinned.length > 0 && <h2 id="tasks-heading">{t('Tasks')}</h2>}
        <ul className="task-list" aria-labelledby={pinned.length > 0 ? 'tasks-heading' : undefined}>
          {rest.map(task => <TaskRow key={task.id} task={task} attention={false}
            workspace={scoped ? null : workspaceName(task.workspaceId)} onOpen={onOpen}/>)}
        </ul>
      </section>}
    </>}
  </div>;
}

function TaskRow({task, attention, workspace, onOpen}: {
  task: TaskSummary; attention: boolean; workspace: string | null; onOpen: (task: TaskSummary) => void;
}) {
  return <li className={`task-row${attention ? ' attention' : ''}`}>
    <StateIcon state={task.state}/>
    <div className="row-body">
      <div className="row-title">
        <span className="name">{task.title}</span>
        {workspace && <span className="host">{workspace}</span>}
      </div>
      <div className="row-status">{statusLine(task)}</div>
      <div className="row-meta">
        <AttemptSequence attempts={task.attempts}/>
        <span className="row-cost">{taskCost(task.cost)}</span>
      </div>
    </div>
    <div className="row-actions">
      <button type="button" className={attention ? 'button prominent' : 'button'} onClick={() => onOpen(task)}>
        {attention ? t('Review') : t('Open')}
      </button>
    </div>
  </li>;
}

export function statusLine(task: TaskSummary, now = Date.now()): string {
  const attempt = task.attempts.length;
  switch (task.state) {
    case 'running': {
      const phase = task.phase === 'preparing' ? t('Preparing') : task.phase === 'checking' ? t('Checking')
        : t('Generating');
      return t('{phase} · attempt {n} · {time}', {phase, n: attempt, time: elapsed(now - task.startedAt)});
    }
    case 'waiting_approval': return t('Waiting for approval · attempt {n}', {n: attempt});
    case 'needs_revalidation': return t('Plan needs revalidation');
    case 'checks_passed': return t('Checks passed · {time}', {time: ago(task.updatedAt, now)});
    case 'manually_accepted': return t('Manually accepted · {time}', {time: ago(task.updatedAt, now)});
    case 'check_failed': return attempt === 1 ? t('Check failed · {time}', {time: ago(task.updatedAt, now)})
      : t('Check failed after {n} attempts · {time}', {n: attempt, time: ago(task.updatedAt, now)});
    case 'blocked': return t('Blocked by policy · {time}', {time: ago(task.updatedAt, now)});
    case 'canceled': return t('Canceled · {time}', {time: ago(task.updatedAt, now)});
    case 'interrupted': return t('Interrupted · needs review');
  }
}

export function StateIcon({state}: {state: TaskState}) {
  const props = {size: 18, strokeWidth: 1.9, 'aria-hidden': true} as const;
  const icon = (() => {
    switch (state) {
      case 'running': return <LoaderCircle {...props} className="spin"/>;
      case 'waiting_approval': return <Hourglass {...props}/>;
      case 'needs_revalidation': return <RefreshCw {...props}/>;
      case 'checks_passed': return <CircleCheck {...props}/>;
      case 'manually_accepted': return <UserCheck {...props}/>;
      case 'check_failed': return <CircleAlert {...props}/>;
      case 'blocked': return <Ban {...props}/>;
      case 'canceled': return <CircleX {...props}/>;
      case 'interrupted': return <TriangleAlert {...props}/>;
    }
  })();
  return <span className={`state-icon ${state}`}>{icon}</span>;
}

function EmptyState({filter, query, hasTasks, onClear, onNewTask}: {
  filter: TaskFilter; query: string; hasTasks: boolean; onClear: () => void; onNewTask: () => void;
}) {
  if (query.trim()) {
    return <div className="placeholder">
      <h2>{t('No Tasks Match “{query}”', {query: query.trim()})}</h2>
      <button type="button" className="button" onClick={onClear}>{t('Clear Search')}</button>
    </div>;
  }
  if (!hasTasks) {
    return <div className="placeholder">
      <h2>{t('Ready for Your First Task')}</h2>
      <p>{t('Describe the work, review the route the app proposes, then run it.')}</p>
      <button type="button" className="button prominent" onClick={onNewTask}>{t('New Task')}</button>
    </div>;
  }
  const title: Record<TaskFilter, string> = {
    all: t('No Tasks'), running: t('Nothing Is Running'), approval: t('Nothing Is Waiting for Approval'),
    completed: t('No Completed Tasks'), failed: t('No Failed Tasks'), stopped: t('No Stopped Tasks'),
  };
  return <div className="placeholder"><h2>{title[filter]}</h2></div>;
}
