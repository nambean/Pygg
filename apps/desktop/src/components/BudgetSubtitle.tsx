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

  // Tasks have no daily API budget yet; the counts stand alone until they do.
  if (!budget) return <span className="subtitle-anchor">{[t('{n} running', {n: running}), t('{n} waiting', {n: waiting})].join(' · ')}</span>;
  const money = budget.unknownCalls > 0 ? t('Budget needs review')
    : t('{recorded} recorded + {reserved} reserved / {limit} today',
      {recorded: usd(budget.recordedUsd), reserved: usd(budget.reservedUsd), limit: usd(budget.limitUsd)});
  const line = [t('{n} running', {n: running}), t('{n} waiting', {n: waiting}), money].join(' · ');

  return <span className="subtitle-anchor" ref={box}>
    <button type="button" className="subtitle-button" aria-expanded={open} onClick={() => setOpen(o => !o)}>{line}</button>
    {open && <div className="popover" role="dialog" aria-label={t('Today’s API budget')}>
      <h3>{t('Today’s API budget')}</h3>
      <dl>
        <dt>{t('Recorded')}</dt><dd>{usd(budget.recordedUsd)}</dd>
        <dt>{t('Reserved for running work')}</dt><dd>{usd(budget.reservedUsd)}</dd>
        <dt>{t('Daily limit')}</dt><dd>{usd(budget.limitUsd)}</dd>
        <dt>{t('Calls with unknown cost')}</dt><dd>{budget.unknownCalls}</dd>
      </dl>
      <p>{t('Reserved money is held for running attempts; it is not spent. Subscription and local use are not included.')}</p>
    </div>}
  </span>;
}
