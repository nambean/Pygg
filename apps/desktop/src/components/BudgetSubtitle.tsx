import { useEffect, useRef, useState } from 'react';
import { usd } from '../lib/format';
import { t } from '../lib/i18n';
import type { DayBudget } from '../lib/tasks';

/**
 * Toolbar subtitle (SCREENS.md §2.3): running and waiting counts plus a qualified budget summary. Clicking
 * it opens the budget popover. Reservations are shown apart from spend; unknown costs make the summary
 * "needs review" instead of a confident remaining amount.
 */
export function BudgetSubtitle({running, waiting, budget}: {running: number; waiting: number; budget?: DayBudget}) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const outside = (event: MouseEvent) => { if (!box.current?.contains(event.target as Node)) setOpen(false); };
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') setOpen(false); };
    document.addEventListener('mousedown', outside);
    document.addEventListener('keydown', escape);
    return () => { document.removeEventListener('mousedown', outside); document.removeEventListener('keydown', escape); };
  }, [open]);

  if (!budget) return <span className="subtitle-anchor">{[t('{n} running', {n: running}), t('{n} waiting', {n: waiting})].join(' · ')}</span>;
  // Paid runs that reported no cost are counted at their cap, so the total is an upper bound, not a figure.
  const money = budget.limitUsd === null
    ? t('{recorded} on paid steps today', {recorded: usd(budget.recordedUsd + budget.reservedUsd)})
    : t('{recorded} recorded + {reserved} reserved / {limit} today',
      {recorded: usd(budget.recordedUsd), reserved: usd(budget.reservedUsd), limit: usd(budget.limitUsd)});
  const line = [t('{n} running', {n: running}), t('{n} waiting', {n: waiting}), money].join(' · ');

  return <span className="subtitle-anchor" ref={box}>
    <button type="button" className="subtitle-button" aria-expanded={open} onClick={() => setOpen(o => !o)}>{line}</button>
    {open && <div className="popover" role="dialog" aria-label={t('Today’s budget for paid steps')}>
      <h3>{t('Today’s budget for paid steps')}</h3>
      <dl>
        <dt>{t('Recorded')}</dt><dd>{usd(budget.recordedUsd)}</dd>
        <dt>{t('Reserved for running work')}</dt><dd>{usd(budget.reservedUsd)}</dd>
        <dt>{t('Daily limit')}</dt><dd>{budget.limitUsd === null ? t('None') : usd(budget.limitUsd)}</dd>
        <dt>{t('Runs with unknown cost')}</dt><dd>{budget.unknownCalls}</dd>
      </dl>
      <p>{t('Recorded is the agents’ own estimates; a run that reported none counts at its cap. Reserved is held for running steps. Free, local and subscription steps are not included. Resets at 00:00 UTC; set the limit in Settings.')}</p>
    </div>}
  </span>;
}
