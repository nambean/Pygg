import { ChevronLeft } from 'lucide-react';
import { taskCost } from '../lib/format';
import { t } from '../lib/i18n';
import type { TaskSummary, Workspace } from '../lib/tasks';
import { billingLabel, outcomeLabel } from './AttemptSequence';
import { StateIcon, statusLine } from './TaskList';

/**
 * Task detail, first cut: the attempts of a task. The plan, live output and result phases
 * (SCREENS.md §5.2) need the task engine and come with it.
 */
export function TaskDetail({task, workspace, onBack}: {task: TaskSummary; workspace: Workspace | undefined;
  onBack: () => void}) {
  return <div className="detail">
    <button type="button" className="back-button" onClick={onBack}>
      <ChevronLeft size={15} strokeWidth={2} aria-hidden/>{t('Tasks')}
    </button>
    <header className="detail-header">
      <StateIcon state={task.state}/>
      <div>
        <h2 className="selectable">{task.title}</h2>
        <p>{statusLine(task)}{workspace ? ` · ${workspace.name}` : ''}</p>
      </div>
    </header>

    <section className="group">
      <h2>{t('Attempts')}</h2>
      <ol className="group-box attempt-rows">
        {task.attempts.map((attempt, index) => <li key={index} className="group-row">
          <span className={`attempt-number ${attempt.outcome}`}>{index + 1}</span>
          <span className="group-label">
            <span className="mono selectable">{attempt.model}</span>
            <small>{`T${attempt.tier} · ${billingLabel(attempt.billing)}`}</small>
          </span>
          <span className="group-control value">{outcomeLabel(attempt.outcome)}</span>
        </li>)}
      </ol>
      <p className="group-footnote">{t('Cost so far: {cost}', {cost: taskCost(task.cost)})}</p>
    </section>

    <p className="detail-note">{t('The plan, live output and result review come with the task engine.')}</p>
  </div>;
}
