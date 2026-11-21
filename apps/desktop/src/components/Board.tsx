import { useEffect, useState } from 'react';
import { ArrowUp, Play } from 'lucide-react';
import { call, errorText, gatewayChanged } from '../lib/api';
import { elapsed, span } from '../lib/format';
import { t } from '../lib/i18n';
import { BOARD_LANES, phaseLabel, type BoardAction, type BoardCard, type BoardLane } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { AttemptSequence } from './AttemptSequence';
import { ConfirmSheet, ErrorNote, Spinner } from './ui';
import './brain.css';
import './board.css';

const LANE_NAMES: Record<BoardLane, string> = {
  discovery: 'Discovery', ready: 'Ready', running: 'Running', checking: 'Checking', blocked: 'Blocked', review: 'Review', done: 'Done', discarded: 'Discarded',
};
/** The order of the single-list layout: what waits on the user first (BRAIN.md 4.6). */
const LIST_ORDER: BoardLane[] = ['blocked', 'review', 'running', 'checking', 'ready', 'discovery', 'done'];

/**
 * The board (SCREENS.md §5.1, BRAIN.md §4.6): one per workspace, the seven columns of ADR-042 with their counts
 * and limits, Blocked pinned with its reasons and time, moves by button, and the measures once five cards are done.
 * It reloads every three seconds; the app's tick pulls cards under the limit.
 */
export function Board({workspaceId, workspaces, onWorkspace, onOpenTask, plan, onAllPlans}: {
  workspaceId: string; workspaces: {workspaceId: string; workspaceName: string}[]; onWorkspace: (id: string) => void; onOpenTask: (taskId: string) => void;
  /** Opened from a plan's page: only that plan's cards, with a way to see every card. The limits stay the board's. */
  plan?: {id: string; title: string} | null; onAllPlans?: () => void;
}) {
  const loaded = useGateway(api => api.board(workspaceId), [workspaceId], 3_000);
  const gateway = useGateway(api => api.gatewayStatus(), [], 5_000);
  const board = loaded.data;
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [editingLimit, setEditingLimit] = useState<'running' | 'checking' | null>(null);
  const [dropping, setDropping] = useState<BoardCard | null>(null);
  const act = (f: () => Promise<unknown>) => { setBusy(true); setError(null); f().then(() => { setBusy(false); loaded.reload(); gatewayChanged(); }, e => { setError(errorText(e)); setBusy(false); }); };
  if (loaded.error && !board) return <div className="page"><ErrorNote>{loaded.error}</ErrorNote>
    <button type="button" className="link-button" onClick={loaded.reload}>{t('Try Again')}</button></div>;
  if (!board) return <div className="page"><Spinner label={t('Loading the board…')}/></div>;
  const cards = board.cards.filter(c => c.lane !== 'discarded' && (!plan || c.brainstormId === plan.id));
  const count = (lane: BoardLane) => cards.filter(c => c.lane === lane).length;
  const limitOf = (lane: BoardLane) => lane === 'running' ? board.limits.wipRunning : lane === 'checking' ? board.limits.wipChecking : null;
  const header = (lane: BoardLane, as: 'h3' | 'div') => {
    const limit = limitOf(lane);
    const inner = <>
      <span>{t(LANE_NAMES[lane])}</span>
      <span className="lim">{limit != null ? `${count(lane)} / ${limit}` : count(lane)}</span>
      {limit != null && (editingLimit === lane
        ? <label className="limit-field"><input className="text-input" type="number" min={1} defaultValue={limit} autoFocus aria-label={t('{lane} limit: Enter saves, Escape keeps {n}', {lane: t(LANE_NAMES[lane]), n: limit})}
            onKeyDown={e => { if (e.key === 'Escape') { e.currentTarget.value = String(limit); e.currentTarget.blur(); } else if (e.key === 'Enter') e.currentTarget.blur(); }}
            onBlur={e => { const n = Math.max(1, Number(e.target.value) || 1); setEditingLimit(null); if (n === limit) return;
              act(() => call(api => api.boardSetLimits(board.workspaceId, lane === 'running' ? n : board.limits.wipRunning, lane === 'checking' ? n : board.limits.wipChecking))); }}/></label>
        : <button type="button" className="link-button" onClick={() => setEditingLimit(lane as 'running' | 'checking')}>{t('Limit…')}</button>)}
    </>;
    return as === 'h3' ? <h3>{inner}</h3> : <div className="board-column-head">{inner}</div>;
  };
  const m = board.measures;
  return <div className="board-page">
    {plan && <p className="group-footnote board-plan-note">{t('Showing the cards of the plan “{title}”. The running limit counts every plan’s cards.', {title: plan.title})}{' '}
      {onAllPlans && <button type="button" className="link-button" onClick={onAllPlans}>{t('Show every card')}</button>}</p>}
    <div className="board-head">
      {workspaces.length > 1
        ? <select className="popup-button" value={board.workspaceId} onChange={e => onWorkspace(e.target.value)} aria-label={t('Workspace')}>
            {workspaces.map(w => <option key={w.workspaceId} value={w.workspaceId}>{w.workspaceName}</option>)}</select>
        : <strong>{board.workspaceName}</strong>}
      <span className="counts">{t('Running {r}/{l} · Blocked {b} · Ready {y} · Review {v} · Done {d}', {r: count('running'), l: board.limits.wipRunning, b: count('blocked'), y: count('ready'), v: count('review'), d: count('done')})}</span>
      <details><summary>{t('Measures')}</summary>
        <span className="brain-meta"> {m.measured
          ? t('cycle time median {c} · {n} done today · blocked today {b} · known money per kept result {money} · from the cards’ times and their tasks’ ledger', {c: m.cycleTimeMedianMs != null ? (m.cycleTimed < m.done ? t('{t} ({k} of {n} cards timed)', {t: span(m.cycleTimeMedianMs), k: m.cycleTimed, n: m.done}) : span(m.cycleTimeMedianMs)) : '—', n: m.doneToday, b: span(m.blockedTodayMs),
              money: m.money.length ? m.money.map(r => `${r.role} ${r.usdPerKept != null ? `≈$${r.usdPerKept.toFixed(r.usdPerKept < 0.1 ? 3 : 2)}` : t('unknown')}${r.unknownCalls ? ` (${t('{n} unpriced', {n: r.unknownCalls})})` : ''}`).join(', ') : t('none kept')})
          : t('measures appear after 5 cards are done ({n} so far)', {n: m.done})}</span></details>
      {board.limits.own && <button type="button" className="link-button" disabled={busy} onClick={() => act(() => call(api => api.boardSetLimits(board.workspaceId, null, null)))}>{t('Use the default limits')}</button>}
      {gateway.data && gateway.data.state !== 'running' && <span className="engine-state"><span className="warn-text">{gateway.data.state === 'failed' ? t('Gateway failed: {error}', {error: gateway.data.error ?? ''}) : t('Gateway stopped')}{' · '}{t('cards run through it, so Ready waits')}</span>
        <button type="button" className="button prominent" disabled={busy} onClick={() => act(() => call(api => api.gatewayStart()).then(() => gateway.reload()))}>{t('Start')}</button></span>}
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
    {cards.length === 0 && <p className="group-footnote">{t('No cards yet. Start a plan with New plan on the project page (or from your coding agent with the model pygg/brain), then approve it.')}</p>}
    <div className="board">
      <div className="board-columns">
        {BOARD_LANES.map(lane => <div key={lane} className={`board-column ${lane}`}>
          {header(lane, 'div')}
          {cards.filter(c => c.lane === lane).map(c => <Card key={c.id} card={c} busy={busy} onAct={a => act(() => call(api => api.boardAct(c.id, a)))} onOpenTask={onOpenTask} onDrop={() => setDropping(c)}/>)}
          {count(lane) === 0 && <p className="board-empty">{t('Nothing here')}</p>}
        </div>)}
      </div>
      <div className="board-list">
        {LIST_ORDER.map(lane => <section key={lane} className={`board-group ${lane}`}>
          {header(lane, 'h3')}
          {cards.filter(c => c.lane === lane).map(c => <Card key={c.id} card={c} busy={busy} onAct={a => act(() => call(api => api.boardAct(c.id, a)))} onOpenTask={onOpenTask} onDrop={() => setDropping(c)}/>)}
          {count(lane) === 0 && <p className="board-empty">{t('Nothing here')}</p>}
        </section>)}
      </div>
    </div>
    {dropping && <ConfirmSheet title={t('Drop “{title}”?', {title: dropping.title})} confirmLabel={t('Drop')}
      message={<p>{t('The card leaves the board and never runs. Cards that depend on it will block, naming it.')}</p>}
      onClose={() => setDropping(null)} onConfirm={async () => { const id = dropping.id; setDropping(null); await call(api => api.boardAct(id, 'drop')); loaded.reload(); gatewayChanged(); }}/>}
  </div>;
}

function Card({card: c, busy, onAct, onOpenTask, onDrop}: {card: BoardCard; busy: boolean; onAct: (a: BoardAction) => void; onOpenTask: (id: string) => void; onDrop: () => void}) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => { const id = setInterval(() => setNow(Date.now()), 30_000); return () => clearInterval(id); }, []);
  const blocked = c.lane === 'blocked' ? c.blockedMs + (c.blockedSince ? now - c.blockedSince : 0) : 0;
  const result = c.task && (c.lane === 'review' || c.lane === 'done') ? c.task.attempts.at(-1) : null;
  const locked = c.lane === 'discovery' || (c.lane === 'blocked' && !c.taskId);
  return <div className={`board-card${locked ? ' locked' : ''}`}>
    <div className="title">{c.class === 'expedite' && <ArrowUp size={12} aria-label={t('Expedite')}/>}{c.title}</div>
    <div className="foot"><span className={`role-chip ${c.role}`}>{c.role}</span>{c.task && <AttemptSequence attempts={c.task.attempts}/>}</div>
    <span className="brain-id">{c.ladder[0] ?? '—'}{c.ladder.length > 1 && t(', then {n} more', {n: c.ladder.length - 1})}</span>
    {c.blockedReason && <div className="reason">{c.blockedReason}</div>}
    {c.runner === 'ide' && !c.taskId && c.lane !== 'done' && <div className="ide-note">{t('Your coding agent does this card; Pygg does not see its progress yet. Hand it to Pygg to run it here instead.')}</div>}
    <div className="foot">
      {c.lane === 'blocked' && <span>{t('blocked {time}', {time: span(blocked)})}</span>}
      {c.task?.state === 'waiting_approval' && <span className="waits-you">{t('waits for your approval')}</span>}
      {c.lane === 'running' && c.startedAt != null && c.task?.state !== 'waiting_approval' && <span>{t('{phase} · {time}', {phase: c.task ? phaseLabel(c.task.phase) : t('running'), time: elapsed(now - c.startedAt)})}</span>}
      {c.lane === 'discovery' && <span>{c.held ? t('held by you') : t('waits for {deps}', {deps: c.dependsOn.join(', ') || '—'})}</span>}
      {c.lane === 'ready' && <span>{t('next in line')}</span>}
      {result && <span>{result.outcome === 'passed' ? t('checks passed') : t('finished')}</span>}
    </div>
    <div className="actions">
      {c.taskId && <button type="button" className="button" aria-label={t('{action}: {title}', {action: c.lane === 'blocked' || c.lane === 'review' ? t('Review') : t('Open'), title: c.title})} onClick={() => onOpenTask(c.taskId!)}>{c.task?.state === 'waiting_approval' ? t('Approve…') : c.lane === 'blocked' || c.lane === 'review' ? t('Review') : t('Open')}</button>}
      {c.lane === 'ready' && <>
        <button type="button" className="button" disabled={busy} onClick={() => onAct(c.class === 'expedite' ? 'standard' : 'expedite')}>{c.class === 'expedite' ? t('Standard') : t('Expedite')}</button>
        <button type="button" className="button" disabled={busy} onClick={() => onAct('hold')}>{t('Back to Discovery')}</button>
      </>}
      {c.lane === 'discovery' && c.held && <button type="button" className="button" disabled={busy} onClick={() => onAct('release')}><Play size={12} aria-hidden/>{t('Move to Ready')}</button>}
      {!c.taskId && c.lane !== 'done' && <button type="button" className="button" disabled={busy} aria-label={t('Drop {title}', {title: c.title})} onClick={onDrop}>{t('Drop')}</button>}
      {c.runner === 'ide' && !c.taskId && <button type="button" className="button" disabled={busy} onClick={() => onAct('engine')}>{t('Hand to Pygg')}</button>}
      {c.lane === 'blocked' && c.taskId && c.task && ['check_failed', 'failed', 'blocked', 'canceled'].includes(c.task.state) &&
        <button type="button" className="button" disabled={busy} aria-label={t('Run again: {title}', {title: c.title})} onClick={() => onAct('rerun')}><Play size={12} aria-hidden/>{t('Run again')}</button>}
    </div>
  </div>;
}
