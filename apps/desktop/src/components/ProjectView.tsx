import { ChevronLeft, Plus, Settings2 } from 'lucide-react';
import { t } from '../lib/i18n';
import { type BoardSummary, type Brainstorm, type TaskSummary, type WorkspaceView } from '../lib/tasks';
import { Board } from './Board';
import { Segmented } from './ui';
import { WorkList } from './WorkList';
import './projects.css';

export type ProjectTab = 'work' | 'board';

type Props = {
  workspace: WorkspaceView;
  tasks: TaskSummary[];
  brainstorms: Brainstorm[];
  board: BoardSummary | null;
  tab: ProjectTab;
  onTab: (tab: ProjectTab) => void;
  onOpenTask: (task: TaskSummary) => void;
  onOpenTaskId: (id: string) => void;
  onOpenPlan: (plan: Brainstorm) => void;
  onNewWork: () => void;
  onSettings: () => void;
  backTo: string;
  onBack: () => void;
  /** The plan the board shows alone, when opened from its page. */
  plan?: {id: string; title: string} | null;
  onAllPlans?: () => void;
};

/**
 * One project (a Git repository, SCREENS.md §5.0): its work (plans, tasks, questions) as one list, or its board, the
 * Kanban view of every approved plan's cards. A plan's own page carries its progress and output.
 */
export function ProjectView({workspace: w, tasks, brainstorms, board, tab, onTab, onOpenTask, onOpenTaskId, onOpenPlan, onNewWork,
  onSettings, backTo, onBack, plan, onAllPlans}: Props) {
  return <div className="projects project">
    <button type="button" className="back-button project-back" onClick={onBack}><ChevronLeft size={15} strokeWidth={2} aria-hidden/>{backTo}</button>
    <div className="project-head">
      <p className="project-path mono selectable">{w.path}</p>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onSettings}><Settings2 size={13} aria-hidden/>{t('Project settings')}</button>
      <button type="button" className="button prominent" onClick={onNewWork}><Plus size={13} aria-hidden/>{t('New work')}</button>
    </div>
    <div className="page-bar">
      <Segmented<ProjectTab> label={t('Show')} value={tab} onChange={onTab}
        options={[{value: 'work', label: t('Work')},
          {value: 'board', label: `${t('Board')}${board && board.attention ? ` ${board.attention}` : ''}`}]}/>
    </div>
    {tab === 'work'
      ? <WorkList tasks={tasks} brainstorms={brainstorms} workspaces={[w]} project={w.id} onOpenTask={onOpenTask} onOpenPlan={onOpenPlan} onNewWork={onNewWork}/>
      : board && board.cards > 0
        ? <Board workspaceId={w.id} workspaces={[{workspaceId: w.id, workspaceName: w.name}]} onWorkspace={() => {}} onOpenTask={onOpenTaskId} plan={plan} onAllPlans={onAllPlans}/>
        : <div className="placeholder">
          <h2>{t('The board is empty')}</h2>
          <p>{t('An approved plan puts its cards here. Pygg runs them one at a time, each checked before the next starts.')}</p>
          <button type="button" className="button prominent" onClick={onNewWork}><Plus size={13} aria-hidden/>{t('New work')}</button>
        </div>}
  </div>;
}
