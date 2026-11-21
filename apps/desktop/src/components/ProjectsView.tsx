import { useState } from 'react';
import { FolderGit2, Plus } from 'lucide-react';
import { gatewayChanged } from '../lib/api';
import { ago } from '../lib/format';
import { t } from '../lib/i18n';
import { brainstormNeedsAttention, needsAttention, type Brainstorm, type TaskSummary, type WorkspaceView } from '../lib/tasks';
import { AddWorkspaceForm } from './WorkspacesView';
import './projects.css';

type Props = {
  tasks: TaskSummary[];
  brainstorms: Brainstorm[];
  workspaces: WorkspaceView[];
  onOpenProject: (workspaceId: string) => void;
  onChanged: () => void;
};

/** Projects (SCREENS.md §5.0): the Git repositories Pygg works in, with what waits and what runs in each. */
export function ProjectsView({tasks, brainstorms, workspaces, onOpenProject, onChanged}: Props) {
  const [adding, setAdding] = useState(false);
  return <div className="projects">
    <div className="list-bar">
      <p className="projects-intro">{t('A project is one of your Git repositories. Pygg works on copies of it and never changes your checkout.')}</p>
      <span className="spacer"/>
      {!adding && <button type="button" className="button prominent" onClick={() => setAdding(true)}><Plus size={13} aria-hidden/>{t('Add project…')}</button>}
    </div>
    {adding && <div className="group-box project-add">
      <AddWorkspaceForm onAdded={w => { setAdding(false); gatewayChanged(); onChanged(); onOpenProject(w.id); }} onCancel={() => setAdding(false)}/>
    </div>}
    {workspaces.length === 0 && !adding ? <div className="placeholder">
      <h2>{t('No projects yet')}</h2>
      <p>{t('Add a Git repository to ask about it, give it a task, or plan a change. A task your coding agent starts adds its repository by itself.')}</p>
      <button type="button" className="button prominent" onClick={() => setAdding(true)}><Plus size={13} aria-hidden/>{t('Add project…')}</button>
    </div> : <ul className="project-grid">
      {workspaces.map(w => <ProjectCard key={w.id} workspace={w} tasks={tasks.filter(task => task.workspaceId === w.id)}
        brainstorms={brainstorms.filter(b => b.workspaceId === w.id)} onOpen={() => onOpenProject(w.id)}/>)}
    </ul>}
  </div>;
}

function ProjectCard({workspace: w, tasks, brainstorms, onOpen}: {
  workspace: WorkspaceView; tasks: TaskSummary[]; brainstorms: Brainstorm[]; onOpen: () => void;
}) {
  const running = tasks.filter(task => task.state === 'running').length;
  const waiting = tasks.filter(needsAttention).length + brainstorms.filter(brainstormNeedsAttention).length;
  const last = Math.max(0, ...tasks.map(task => task.updatedAt), ...brainstorms.map(b => b.updatedAt));
  const status = [
    waiting ? t('{n} waiting for you', {n: waiting}) : null,
    running ? t('{n} running', {n: running}) : null,
    !waiting && !running ? (tasks.length + brainstorms.length === 0 ? t('no work yet') : t('nothing running')) : null,
  ].filter(Boolean).join(' · ');
  return <li className="project-card">
    <button type="button" className="project-open" onClick={onOpen} aria-label={t('Open project {name}', {name: w.name})}>
      <FolderGit2 size={18} strokeWidth={1.8} aria-hidden/>
      <span className="project-text">
        <span className="project-name">{w.name}</span>
        <span className="project-path mono">{w.path}</span>
        <span className={waiting ? 'project-status waiting' : 'project-status'}>{status}{last ? ` · ${ago(last)}` : ''}</span>
      </span>
    </button>
  </li>;
}
