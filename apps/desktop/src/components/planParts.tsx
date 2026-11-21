import { useState } from 'react';
import { Check, Copy, GitBranch } from 'lucide-react';
import { call, errorText } from '../lib/api';
import { t } from '../lib/i18n';
import type { BoardCard, Brainstorm, TaskSummary } from '../lib/tasks';
import { DiffView } from './DiffView';
import { ErrorNote, Spinner } from './ui';
import './projects.css';

/** Parts of a plan's page (BrainstormDetail) and of the New work form: progress, output, the start form. */

/** A plan's progress from its cards on the board (dropped ones left out). */
export function planProgress(cards: BoardCard[]) {
  const live = cards.filter(c => c.lane !== 'discarded');
  const count = (lane: string) => live.filter(c => c.lane === lane).length;
  return {total: live.length, done: count('done'), running: count('running') + count('checking'), review: count('review'), blocked: count('blocked')};
}

export function PlanOutput({plan, tasks, progress}: {plan: Brainstorm; tasks: TaskSummary[]; progress: {total: number; done: number}}) {
  const apart = tasks.filter(task => task.result === 'kept' && task.resultBranch && task.resultBranch !== plan.branch);
  const finished = progress.total > 0 && progress.done === progress.total;
  return <div className="plan-output">
    <p className="output-line"><GitBranch size={15} aria-hidden/>
      <span>{finished
        ? apart.length ? t('Every card is done. The plan’s branch has all but {n} of them:', {n: apart.length}) : t('Every card is done. The work is on one branch:')
        : t('{done} of {total} cards done so far; the plan’s branch:', {done: progress.done, total: progress.total})}</span>
      <CopyText text={plan.branch!}/></p>
    <p className="group-footnote">{t('Review it, then merge it into your branch when you are happy:')}</p>
    <pre className="selectable">{`git log --oneline ${plan.branch}\ngit diff HEAD...${plan.branch}\ngit merge ${plan.branch}`}</pre>
    <PlanChanges id={plan.id}/>
    {apart.length > 0 && <div className="output-apart">
      <p><strong>{t('{n} results on branches of their own:', {n: apart.length})}</strong> {t('their work could not be added to the plan’s branch (it conflicted with a card that ran beside it, or the branch was checked out). Review each and merge it by hand.')}</p>
      <ul className="output-list">{apart.map(task => <li key={task.id}><GitBranch size={13} aria-hidden/>{task.title}<CopyText text={task.resultBranch!}/></li>)}</ul>
    </div>}
  </div>;
}

export function CopyText({text}: {text: string}) {
  const [copied, setCopied] = useState(false);
  return <span className="copy-text"><code className="selectable">{text}</code>
    <button type="button" className="button small" aria-label={t('Copy {text}', {text})} title={t('Copy')}
      onClick={() => { void navigator.clipboard?.writeText(text); setCopied(true); setTimeout(() => setCopied(false), 1500); }}>
      {copied ? <Check size={12} aria-hidden/> : <Copy size={12} aria-hidden/>}</button></span>;
}

/** The plan's combined changes, loaded when asked (it can be large). */
function PlanChanges({id}: {id: string}) {
  const [diff, setDiff] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const show = () => {
    const opening = !open;
    setOpen(opening);
    if (opening) { setDiff(null); setError(null); call(api => api.brainstormDiff(id)).then(setDiff, e => setError(errorText(e))); }
  };
  return <div className="plan-changes">
    <button type="button" className="button" onClick={show}>{open ? t('Hide the changes') : t('Review the changes')}</button>
    {open && (error ? <ErrorNote>{error}</ErrorNote> : diff === null ? <Spinner label={t('Loading changes…')}/>
      : diff.trim() === '' ? <p className="group-footnote">{t('No changes against your branch: it may be merged already.')}</p> : <DiffView diff={diff}/>)}
  </div>;
}
