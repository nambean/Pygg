import { Ban, CircleAlert, CircleCheck, CircleX, Eye, Hourglass, LoaderCircle, MessageSquareText, RefreshCw, TriangleAlert,
  UserCheck } from 'lucide-react';
import { ago, elapsed } from '../lib/format';
import { t } from '../lib/i18n';
import { type TaskState, type TaskSummary, phaseLabel } from '../lib/tasks';

/** A task's state as a status line and an icon, for Work and the task page. */
export function statusLine(task: TaskSummary, now = Date.now()): string {
  const attempt = task.attempts.length;
  switch (task.state) {
    case 'running': {
      const phase = phaseLabel(task.phase);
      return t('{phase} · attempt {n} · {time}', {phase, n: attempt, time: elapsed(now - task.startedAt)});
    }
    case 'waiting_approval': return t('Waiting for approval · attempt {n}', {n: attempt});
    case 'needs_revalidation': return t('Plan needs revalidation');
    case 'checks_passed': return t('Checks passed · {time}', {time: ago(task.updatedAt, now)});
    case 'needs_review': return t('Finished, not ready: nothing checked it · {time}', {time: ago(task.updatedAt, now)});
    case 'manually_accepted': return t('Manually accepted · {time}', {time: ago(task.updatedAt, now)});
    case 'answered': return t('Answered · {time}', {time: ago(task.updatedAt, now)});
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
      case 'needs_review': return <Eye {...props}/>;
      case 'manually_accepted': return <UserCheck {...props}/>;
      case 'answered': return <MessageSquareText {...props}/>;
      case 'check_failed': return <CircleAlert {...props}/>;
      case 'blocked': return <Ban {...props}/>;
      case 'canceled': return <CircleX {...props}/>;
      case 'interrupted': return <TriangleAlert {...props}/>;
    }
  })();
  return <span className={`state-icon ${state}`}>{icon}</span>;
}
