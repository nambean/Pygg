import { useState } from 'react';
import { FolderGit2, Plus } from 'lucide-react';
import { gatewayChanged } from '../lib/api';
import { ago } from '../lib/format';
import { t } from '../lib/i18n';
import { brainstormNeedsAttention, needsAttention, type BoardSummary, type Brainstorm, type TaskSummary, type WorkspaceView } from '../lib/tasks';
import { BrainstormRow, TaskRow } from './TaskList';
import { AddWorkspaceForm } from './WorkspacesView';
import './projects.css';

type Props = {
  tasks: TaskSummary[];
  brainstorms: Brainstorm[];
  workspaces: WorkspaceView[];
  boards: BoardSummary[];
  onOpenProject: (workspaceId: string) => void;
  onOpenTask: (task: TaskSummary) => void;
  onOpenBrainstorm: (brainstorm: Brainstorm) => void;
  onNewTask: () => void;
  onAllTasks: () => void;
  onChanged: () => void;
};

/** Smart Tasks' entry point: what waits on you, your projects, and recent solo tasks. A project is a Git repository
 *  (a workspace); in it you plan with Brain Storming, the board runs the tasks, and the output is a branch to merge. */
export function ProjectsView({tasks, brainstorms, workspaces, boards, onOpenProject, onOpenTask, onOpenBrainstorm, onNewTask, onAllTasks,
  onChanged}: Props) {
  const [adding, setAdding] = useState(false);
  const projectName = (id: string | null) => workspaces.find(w => w.id === id)?.name ?? '';
  const waitingBrains = brainstorms.filter(brainstormNeedsAttention);
  const waitingTasks = tasks.filter(needsAttention);
  const solo = tasks.filter(task => !task.brainstormId).slice(0, 6);

  return <div className="projects">
    <p className="projects-intro">{t('A project is one of your Git repositories. Plan it with Brain Storming, let the board run the tasks, and take the result as a branch. Or run one solo task.')}</p>

    {(waitingBrains.length > 0 || waitingTasks.length > 0) && <section className="task-section" aria-labelledby="projects-attention">
      <h2 id="projects-attention">{t('Needs your attention')}</h2>
      <ul className="task-list">
        {waitingBrains.map(b => <BrainstormRow key={b.id} brainstorm={b} attention onOpen={onOpenBrainstorm}
          project={b.workspaceId ? projectName(b.workspaceId) : t('no project yet')}/>)}
        {waitingTasks.map(task => <TaskRow key={task.id} task={task} attention workspace={projectName(task.workspaceId)} onOpen={onOpenTask}/>)}
      </ul>
    </section>}

    <section className="task-section" aria-labelledby="projects-list">
      <div className="section-heading">
        <h2 id="projects-list">{t('Projects')}</h2>
        {!adding && <button type="button" className="button" onClick={() => setAdding(true)}><Plus size={13} aria-hidden/>{t('Add project…')}</button>}
      </div>
      {adding && <div className="group-box project-add">
        <AddWorkspaceForm onAdded={w => { setAdding(false); gatewayChanged(); onChanged(); onOpenProject(w.id); }} onCancel={() => setAdding(false)}/>
      </div>}
      {workspaces.length === 0 && !adding ? <div className="placeholder">
        <h2>{t('No projects yet')}</h2>
        <p>{t('Add a Git repository to plan a project or run one task. A task your coding agent starts adds its repository by itself.')}</p>
        <button type="button" className="button prominent" onClick={() => setAdding(true)}><Plus size={13} aria-hidden/>{t('Add project…')}</button>
      </div> : <ul className="project-grid">
        {workspaces.map(w => <ProjectCard key={w.id} workspace={w} tasks={tasks.filter(task => task.workspaceId === w.id)}
          brainstorms={brainstorms.filter(b => b.workspaceId === w.id)} board={boards.find(b => b.workspaceId === w.id) ?? null}
          onOpen={() => onOpenProject(w.id)}/>)}
      </ul>}
    </section>

    <section className="task-section" aria-labelledby="projects-solo">
      <div className="section-heading">
        <h2 id="projects-solo">{t('Solo tasks')}</h2>
        <button type="button" className="button" onClick={onNewTask}><Plus size={13} aria-hidden/>{t('New solo task')}</button>
        {tasks.length > 0 && <button type="button" className="link-button" onClick={onAllTasks}>{t('All tasks')}</button>}
      </div>
      {solo.length === 0 ? <p className="group-footnote">{t('One change in one project, without a plan: describe it, pick the agents, and Pygg runs it on a copy of the repository and checks it.')}</p>
        : <ul className="task-list">
          {solo.map(task => <TaskRow key={task.id} task={task} attention={false} workspace={projectName(task.workspaceId)} onOpen={onOpenTask}/>)}
        </ul>}
    </section>
  </div>;
}

function ProjectCard({workspace: w, tasks, brainstorms, board, onOpen}: {
  workspace: WorkspaceView; tasks: TaskSummary[]; brainstorms: Brainstorm[]; board: BoardSummary | null; onOpen: () => void;
}) {
  const plans = brainstorms.filter(b => b.state !== 'declined');
  const running = tasks.filter(task => task.state === 'running').length;
  const waiting = tasks.filter(needsAttention).length + brainstorms.filter(brainstormNeedsAttention).length;
  const last = Math.max(0, ...tasks.map(task => task.updatedAt), ...brainstorms.map(b => b.updatedAt));
  const status = [
    plans.length === 0 ? t('no plan yet') : plans.length === 1 ? t('1 plan') : t('{n} plans', {n: plans.length}),
    board && board.cards > 0 ? t('{n} cards on the board', {n: board.cards}) : null,
    running ? t('{n} running', {n: running}) : null,
    waiting ? t('{n} waiting for you', {n: waiting}) : null,
  ].filter(Boolean).join(' · ');
  return <li className="project-card">
    <button type="button" className="project-open" onClick={onOpen} aria-label={t('Open project {name}', {name: w.name})}>
      <FolderGit2 size={18} strokeWidth={1.8} aria-hidden/>
      <span className="project-text">
        <span className="project-name">{w.name}</span>
        <span className="project-path mono">{w.path}</span>
        <span className="project-status">{status}{last ? ` · ${ago(last)}` : ''}</span>
      </span>
    </button>
  </li>;
}
