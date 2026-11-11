import { Check, Circle, Eye, Hourglass, LoaderCircle, X } from 'lucide-react';
import { t } from '../lib/i18n';
import type { Attempt, AttemptOutcome, Billing } from '../lib/tasks';

/**
 * The app's signature element (SCREENS.md, intro): one chip per attempt, tier and outcome, joined by
 * arrows, so a failed attempt visibly leads to the next decision.
 */
export function AttemptSequence({attempts}: {attempts: Attempt[]}) {
  return <ol className="attempts" aria-label={t('Attempts')}>
    {attempts.map((attempt, index) => <li key={index} className={`attempt ${attempt.outcome}`}
      title={`${attempt.model} · ${billingLabel(attempt.billing)} · ${outcomeLabel(attempt.outcome)}`}>
      {index > 0 && <span className="attempt-arrow" aria-hidden>→</span>}
      <span className="attempt-chip">
        <OutcomeIcon outcome={attempt.outcome}/>
        <span>T{attempt.tier}</span>
        <span className="sr-only">{`${billingLabel(attempt.billing)}, ${outcomeLabel(attempt.outcome)}`}</span>
      </span>
    </li>)}
  </ol>;
}

function OutcomeIcon({outcome}: {outcome: AttemptOutcome}) {
  const props = {size: 11, strokeWidth: 2.6, 'aria-hidden': true} as const;
  switch (outcome) {
    case 'passed': return <Check {...props}/>;
    case 'done': return <Eye {...props}/>;
    case 'failed': return <X {...props}/>;
    case 'running': return <LoaderCircle {...props} className="spin"/>;
    case 'waiting': return <Hourglass {...props}/>;
    case 'canceled': return <Circle {...props}/>;
  }
}

export function billingLabel(billing: Billing): string {
  switch (billing) {
    case 'free': return t('Free');
    case 'local': return t('Local');
    case 'subscription': return t('Subscription');
    case 'paid': return t('Paid');
    case 'unknown': return t('Unknown billing');
  }
}

export function outcomeLabel(outcome: AttemptOutcome): string {
  switch (outcome) {
    case 'passed': return t('Passed');
    case 'done': return t('Finished, not checked');
    case 'failed': return t('Check failed');
    case 'running': return t('Running');
    case 'waiting': return t('Waiting for approval');
    case 'canceled': return t('Stopped');
  }
}
