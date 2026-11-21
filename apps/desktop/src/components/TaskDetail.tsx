import { useEffect, useRef, useState } from 'react';
import { Check, ChevronLeft, FileDown, GitBranch, Play, RotateCcw, Square, Trash2, X } from 'lucide-react';
import { call, errorText, gatewayChanged } from '../lib/api';
import { taskCost } from '../lib/format';
import { t } from '../lib/i18n';
import { engineText } from '../lib/engineText';
import { tokens } from '../lib/money';
import { AGENT_NAMES, type Activity, type ApprovalView, type AttemptView, type TaskDetail as Detail } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { billingLabel, outcomeLabel } from './AttemptSequence';
import { DiffView } from './DiffView';
import { StateIcon, statusLine } from './TaskList';
import { Badge, ConfirmSheet, CopyButton, ErrorNote, FormRow, Segmented, Spinner } from './ui';
import './projects.css';

/**
 * Task detail (SCREENS.md §5.2): what the task is waiting on, its result, and every attempt with its checks.
 * Reloads every two seconds while the task runs.
 */
type Tab = 'overview' | 'activity' | 'changes';

export function TaskDetail({id, onBack, backTo}: {id: string; onBack: () => void; backTo?: string}) {
  const [live, setLive] = useState(true);
  const loaded = useGateway(api => api.task(id).then(d => { setLive(d.state === 'running'); return d; }), [id], live ? 2_000 : 10_000);
  const task = loaded.data;
  const [tab, setTab] = useState<Tab>('overview');
  const hasChanges = !!task && !task.answer && task.attemptDetails.some(a => (a.filesChanged ?? 0) > 0);
  const running = task?.attemptDetails.find(a => a.outcome === 'running');
  return <div className="detail">
    <button type="button" className="back-button" onClick={onBack}>
      <ChevronLeft size={15} strokeWidth={2} aria-hidden/>{backTo ?? t('Tasks')}
    </button>
    {loaded.error && <ErrorNote>{loaded.error}</ErrorNote>}
    {!task ? <Spinner/> : <>
      <header className="detail-header">
        <StateIcon state={task.state}/>
        <div>
          <h2 className="selectable">{task.title}</h2>
          <p>{task.answer ? `${t('Ask')} · ` : ''}{statusLine(task)} · {task.workspaceName}{task.source ? ` · ${t('from {client}', {client: task.source})}` : ''}</p>
        </div>
      </header>
      <div className="page-bar detail-tabs">
        <Segmented<Tab> label={t('Show')} value={tab} onChange={setTab}
          options={[{value: 'overview', label: t('Overview')},
            {value: 'activity', label: `${t('Activity')}${task.attemptDetails.length ? ` ${task.attemptDetails.length}` : ''}`},
            ...(hasChanges ? [{value: 'changes' as Tab, label: t('Changes')}] : [])]}/>
      </div>

      {tab === 'overview' && <>
      <Evidence task={task}/>
      <Actions task={task} onChanged={() => { loaded.reload(); gatewayChanged(); }}/>
      {running && <section className="group">
        <h2>{t('Now')}</h2>
        <div className="group-box now-box">
          <p className="secondary">{t('Step {n}', {n: running.step + 1})} · {engineText(running.label)}</p>
          <LiveActivity attemptId={running.id} agent={running.agent}/>
        </div>
      </section>}
      <section className="group">
        <h2>{t('Steps')}</h2>
        {task.attemptDetails.length === 0 ? <p className="group-footnote">{t('The first step is starting.')}</p>
          : <ol className="group-box step-summary">
            {task.attemptDetails.map(a => <li key={a.id} className="group-row">
              <span className={`attempt-number ${a.outcome}`}>{a.number}</span>
              <span className="step-summary-text"><span className="mono">{engineText(a.label)}</span>
                {a.escalationReason && <small className="secondary">{engineText(a.escalationReason)}</small>}</span>
              <span className="receipt-cost">{stepCost(a)}</span>
              <span className="value">{outcomeLabel(a.outcome)}</span>
            </li>)}
          </ol>}
        <p className="receipt-total"><strong>{t('Total: {cost}', {cost: taskCost(task.cost)})}</strong>
          {task.policy.capMicroUsd ? ` · ${t('cap {usd}', {usd: `$${(task.policy.capMicroUsd / 1e6).toFixed(2)}`})}` : ''}
          {task.result === 'kept' ? ` · ${t('for one kept result')}` : task.state === 'answered' ? ` · ${t('for one answer')}` : ''}</p>
        <p className="group-footnote">
          {task.cost.unknownCalls > 0 && `${task.attemptDetails.some(a => (a.unpricedRequests ?? 0) > 0)
            ? t('Some requests through Pygg used a model without a price: enter it in Connections.')
            : t('Agents run with their own sign-in, so their cost is unknown to Pygg.')}`}
          {' '}<button type="button" className="link-button" onClick={() => setTab('activity')}>{t('Every step in full')}</button></p>
      </section>

      <section className="group">
        <div className="group-heading"><h2>{t('Task')}</h2><span className="spacer"/><BriefButton taskId={task.id}/></div>
        <div className="group-box">
          <Prompt text={task.prompt}/>
          <dl className="detail-list">
            <dt>{t('Steps')}</dt>
            <dd>{task.policy.steps.map((s, i) => `${i + 1}. ${AGENT_NAMES[s.agent] ?? s.agent}${s.model ? ` · ${s.model}` : ''} (T${s.tier}, ${billingLabel(s.billing)})`).join('  ')}</dd>
            <dt>{t('Allowed')}</dt>
            <dd>{[task.policy.allowShell ? t('shell') : null, task.policy.allowWeb ? t('web') : null].filter(Boolean).join(', ') || t('editing files in its copy only')}</dd>
            <dt>{t('Checks')}</dt>
            <dd className="mono">{task.policy.checks.length ? task.policy.checks.map(c => c.command).join(' → ') : t('None approved')}</dd>
            <dt>{t('Started from')}</dt><dd className="mono selectable">{task.baseRevision?.slice(0, 10) ?? '—'} · {task.workspacePath}</dd>
          </dl>
        </div>
      </section>
      </>}

      {tab === 'activity' && <section className="group">
        <h2>{t('Every step, in full')}</h2>
        {task.attemptDetails.length === 0 ? <p className="group-footnote">{t('The first step is starting.')}</p>
          : <ol className="group-box attempt-rows">
            {task.attemptDetails.map(a => <AttemptRow key={a.id} attempt={a}/>)}
          </ol>}
        <p className="group-footnote">{t('A finished step keeps the end of its agent’s output; the live lines last while the app runs.')}</p>
      </section>}

      {tab === 'changes' && <Changes taskId={task.id} kept={task.result === 'kept'} open={!task.result}/>}
    </>}
  </div>;
}

/** The task's changes as one diff: the result waiting for a decision, or the branch it was kept as. */
function Changes({taskId, kept, open}: {taskId: string; kept: boolean; open: boolean}) {
  const [diff, setDiff] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    call(api => api.taskDiff(taskId)).then(d => { if (live) setDiff(d); }, e => { if (live) setError(errorText(e)); });
    return () => { live = false; };
  }, [taskId]);
  return <section className="group">
    <div className="diff-section-head"><h2>{t('Changes')}</h2><span className="spacer"/>{diff !== null && diff !== '' && <CopyButton text={diff} label={t('Copy Diff')}/>}</div>
    {error ? <p className="group-footnote">{kept ? t('This result was kept on its plan’s branch, which holds the other cards too: see the plan’s Output.') : open ? error
        : t('Nothing to show: the result was discarded, or the copy it was made in is gone.')}</p>
      : diff === null ? <Spinner label={t('Loading changes…')}/> : diff.trim() === '' ? <p className="group-footnote">{t('No changes.')}</p> : <DiffView diff={diff}/>}
  </section>;
}

/** The task's text, folded to its first lines when long (a card's prompt carries the whole requirement). */
function Prompt({text}: {text: string}) {
  const lines = text.split('\n');
  const long = lines.length > 8 || text.length > 900;
  const [open, setOpen] = useState(false);
  const shown = !long || open ? text : lines.slice(0, 6).join('\n').slice(0, 700) + '…';
  return <>
    <p className="agent-message selectable">{shown}</p>
    {long && <button type="button" className="link-button" onClick={() => setOpen(o => !o)}>
      {open ? t('Show less') : t('Show the whole task')}</button>}
  </>;
}

function Actions({task, onChanged}: {task: Detail; onChanged: () => void}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [diff, setDiff] = useState<string | null>(null);
  const [title, setTitle] = useState('');
  const [discarding, setDiscarding] = useState(false);
  const run = (f: () => Promise<unknown>, done?: (value: unknown) => void) => {
    setBusy(true);
    setError(null);
    f().then(v => { done?.(v); setBusy(false); onChanged(); }, e => { setError(errorText(e)); setBusy(false); });
  };
  const result = task.result === null && (task.state === 'checks_passed' || task.state === 'needs_review')
    ? task.attemptDetails.find(a => a.open) : undefined;
  // A result waiting for a decision is reviewed first, so its changes load and show at once.
  const resultId = result?.id;
  useEffect(() => {
    if (!resultId) return;
    let live = true;
    call(api => api.taskDiff(task.id)).then(d => { if (live) setDiff(d); }, x => { if (live) setError(errorText(x)); });
    return () => { live = false; };
  }, [task.id, resultId]);

  return <section className="group task-actions">
    {task.reason && <p className={task.state === 'checks_passed' || task.state === 'manually_accepted' || task.state === 'answered' ? 'group-footnote' : 'warning-note'}>{engineText(task.reason)}</p>}
    {task.state === 'answered' && <Answer task={task}/>}
    {task.result === 'kept' && task.resultBranch && <div className="group-box task-output">
      <div className="agent-result-head"><Badge tone="ok">{t('Output')}</Badge>
        <span className="secondary">{task.brainstormId && task.resultBranch.startsWith('str/brain-')
          ? t('This card’s work is on its plan’s branch, with the cards before it.')
          : task.brainstormId ? t('This card ran beside another and did not apply cleanly on its plan’s branch, so it is on a branch of its own: merge it by hand.')
          : t('Kept as a branch. Your checkout was not changed.')}</span></div>
      <p className="output-line"><GitBranch size={14} aria-hidden/><code className="selectable">{task.resultBranch}</code>
        <CopyButton text={task.resultBranch} label={t('Copy')}/></p>
      <pre className="log-tail selectable">{`git log --oneline ${task.resultBranch}\ngit diff HEAD...${task.resultBranch}\ngit merge ${task.resultBranch}`}</pre>
      <p className="group-footnote">{t('Review the branch, then merge it into yours when you are happy. Pygg never merges for you.')}</p>
    </div>}
    {task.result === 'discarded' && <p className="group-footnote">{t('The result was discarded.')}</p>}
    {task.brainstormId && !task.result && (task.state === 'check_failed' || task.state === 'blocked' || task.state === 'interrupted' || task.state === 'canceled') &&
      <p className="group-footnote">{t('This is a card of a plan. To try it again, after raising a limit or changing its models, use Run again on the card in the project’s board.')}</p>}
    {message && <p className="ok-note">{message}</p>}

    {result && <div className="group-box agent-result">
      <div className="agent-result-head">
        {task.state === 'checks_passed' ? <Badge tone="ok">{t('Checks passed')}</Badge> : <Badge tone="warn">{t('Needs your review')}</Badge>}
        <span className="secondary">{t('{files} files, +{ins} −{del}', {files: result.filesChanged ?? 0, ins: result.insertions ?? 0, del: result.deletions ?? 0})}</span>
      </div>
      {result.summary && <p className="agent-message">{result.summary}</p>}
      <div className="diff-section">
        <div className="diff-section-head">
          <h3>{t('Review Changes')}</h3>
          <span className="spacer"/>
          {diff !== null && <CopyButton text={diff} label={t('Copy Diff')}/>}
        </div>
        {diff === null ? <Spinner label={t('Loading changes…')}/> : <DiffView diff={diff}/>}
      </div>
      <div className="form-actions">
        <input className="text-input" value={title} onChange={e => setTitle(e.target.value)} placeholder={task.title}
          aria-label={t('Branch title')} disabled={busy}/>
        <button type="button" className="button prominent" disabled={busy}
          onClick={() => run(() => call(api => api.finishTask(task.id, true, title.trim() || null)))}>
          <GitBranch size={13} aria-hidden/>{t('Keep as Branch')}</button>
        <button type="button" className="button" disabled={busy} onClick={() => setDiscarding(true)}>
          <Trash2 size={13} aria-hidden/>{t('Discard…')}</button>
        <button type="button" className="button" disabled={busy} title={t('Save the changes as a file git apply takes; the result stays here')}
          onClick={() => run(() => call(api => api.exportPatch(task.id)),
            path => { if (path) setMessage(t('Saved as {path}. The result is still here to keep or discard.', {path: String(path)})); })}>
          <FileDown size={13} aria-hidden/>{t('Export Patch…')}</button>
      </div>
      <p className="group-footnote">{t('Keep as Branch saves the result as a new branch in the repository. The folder you have open, and its branch, are not changed.')}</p>
    </div>}
    {discarding && <ConfirmSheet title={t('Discard this result?')} confirmLabel={t('Discard Result')}
      message={<p>{t('The changes and the copy of the repository they were made in are deleted. This cannot be undone. Export Patch first to keep a copy.')}</p>}
      onClose={() => setDiscarding(false)}
      onConfirm={async () => { await call(api => api.finishTask(task.id, false, null)); setMessage(t('Discarded.')); onChanged(); }}/>}

    {task.approval && <ApprovalPanel key={`${task.approval.step}-${task.approval.estimate.highMicroUsd}`} approval={task.approval}
      busy={busy} onDecide={(approve, cap) => run(() => call(api => api.decideTask(task.id, approve, cap)))}/>}

    <div className="form-actions">
      {task.state === 'interrupted' && <button type="button" className="button prominent" disabled={busy}
        onClick={() => run(() => call(api => api.retryTask(task.id)))}><RotateCcw size={13} aria-hidden/>{t('Retry')}</button>}
      {(task.state === 'running' || task.state === 'waiting_approval' || task.state === 'interrupted') &&
        <button type="button" className="button" disabled={busy} onClick={() => run(() => call(api => api.cancelTask(task.id)))}>
          <Square size={12} aria-hidden/>{t('Stop Task')}</button>}
      {busy && <Spinner/>}
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </section>;
}

/**
 * A paid step waiting for the user (ADR-010): what would run, what this agent cost before by its own report, and
 * the cap this one approval allows. Nothing is approved automatically.
 */
function ApprovalPanel({approval: a, busy, onDecide}: {approval: ApprovalView; busy: boolean;
  onDecide: (approve: boolean, capUsd: number | null) => void}) {
  const [cap, setCap] = useState(a.suggestedCapUsd.toFixed(2));
  const amount = Number(cap.replace(',', '.'));
  const valid = Number.isFinite(amount) && amount > 0;
  const e = a.estimate;
  const money = (micro: number) => `$${(micro / 1e6).toFixed(2)}`;
  if (!a.paid) return <div className="group-box approval-panel">
    <div className="agent-result-head"><Badge tone="warn">{t('Sensitive workspace')}</Badge>
      <span className="mono">{t('Step {n}', {n: a.step + 1})} · {engineText(a.label)}</span></div>
    <p className="secondary">{t('This step would let the agent read and change this workspace’s code. Pygg cannot yet check where an agent sends it, so it asks first.')}</p>
    <div className="form-actions">
      <button type="button" className="button prominent" disabled={busy} onClick={() => onDecide(true, null)}>
        <Check size={13} aria-hidden/>{t('Allow This Step')}</button>
      <button type="button" className="button" disabled={busy} onClick={() => onDecide(false, null)}>
        <X size={13} aria-hidden/>{t('Decline')}</button>
    </div>
  </div>;
  return <div className="group-box approval-panel">
    <div className="agent-result-head"><Badge tone="warn">{t('Paid step')}</Badge>
      <span className="mono">{t('Step {n}', {n: a.step + 1})} · {engineText(a.label)}</span></div>
    <p className="secondary">{e.highMicroUsd === null || e.lowMicroUsd === null
      ? t('No estimate: this agent and model have no earlier run that reported a cost.')
      : e.lowMicroUsd === e.highMicroUsd
        ? t('Its last {n} runs cost {amount} each, by the agent’s own list-price estimate.', {n: e.runs, amount: money(e.highMicroUsd)})
        : t('Its last {n} runs cost {low} to {high}, by the agent’s own list-price estimate.', {n: e.runs, low: money(e.lowMicroUsd), high: money(e.highMicroUsd)})}
      {a.remainingUsd !== null && ` ${t('Left of today’s budget for paid steps: {amount}.', {amount: `$${a.remainingUsd.toFixed(2)}`})}`}</p>
    <FormRow label={a.enforcedByAgent ? t('Spending cap') : t('Stop further steps above')} htmlFor="approval-cap"
      hint={a.enforcedByAgent ? t('The agent stops at this amount itself.')
        : t('This agent cannot stop at a cap. If the cost it reports afterwards is over it, Pygg tries no further step.')}>
      <span className={valid ? 'money-input' : 'money-input invalid'}><span aria-hidden>$</span>
        <input id="approval-cap" inputMode="decimal" value={cap} onChange={ev => setCap(ev.target.value)} disabled={busy}/></span>
    </FormRow>
    <div className="form-actions">
      <button type="button" className="button prominent" disabled={busy || !valid} onClick={() => onDecide(true, amount)}>
        <Check size={13} aria-hidden/>{a.enforcedByAgent
          ? t('Approve up to {amount}', {amount: valid ? `$${amount.toFixed(2)}` : '—'})
          : t('Approve Step')}</button>
      <button type="button" className="button" disabled={busy} onClick={() => onDecide(false, null)}>
        <X size={13} aria-hidden/>{t('Decline')}</button>
    </div>
  </div>;
}

function AttemptRow({attempt: a}: {attempt: AttemptView}) {
  const failedCheck = a.checks.find(c => !c.passed);
  return <li className="group-row attempt-detail">
    <span className={`attempt-number ${a.outcome}`}>{a.number}</span>
    <div className="attempt-body">
      <div className="attempt-line">
        <span className="mono selectable">{engineText(a.label)}</span>
        <small className="secondary">{t('Step {n}', {n: a.step + 1})} · T{a.tier} · {billingLabel(a.billing)}
          {a.agentModel ? ` · ${a.agentModel}` : ''}</small>
        <span className="spacer"/>
        <span className="value">{a.outcome === 'running' ? <><Play size={11} aria-hidden/> {outcomeLabel(a.outcome)}</> : outcomeLabel(a.outcome)}</span>
      </div>
      {a.escalationReason && <p className="secondary">{engineText(a.escalationReason)}</p>}
      {(a.filesChanged !== null || a.inputTokens !== null) && <p className="secondary">
        {a.filesChanged !== null && t('{files} files, +{ins} −{del}', {files: a.filesChanged, ins: a.insertions ?? 0, del: a.deletions ?? 0})}
        {a.inputTokens !== null && ` · ${t('{input} in · {output} out', {input: tokens(a.inputTokens), output: tokens(a.outputTokens ?? 0)})}`}
        {a.startedAt && a.finishedAt ? ` · ${t('{s} s', {s: ((a.finishedAt - a.startedAt) / 1000).toFixed(1)})}` : ''}
        {a.costSource === 'pygg' && a.agentEstimateUsd !== null
          ? ` · ${t('Pygg’s ledger {amount}', {amount: `$${a.agentEstimateUsd.toFixed(a.agentEstimateUsd < 0.01 ? 4 : 2)}`})}`
          : a.agentEstimateUsd !== null && a.agentEstimateUsd > 0 ? ` · ${t('agent’s estimate {amount}', {amount: `$${a.agentEstimateUsd.toFixed(2)}`})}` : ''}
        {(a.unpricedRequests ?? 0) > 0 ? ` · ${t('{n} requests through Pygg without a price', {n: a.unpricedRequests ?? 0})}` : ''}
        {a.capUsd !== null ? ` · ${t('cap {amount}', {amount: `$${a.capUsd.toFixed(2)}`})}` : ''}
      </p>}
      {a.checks.length > 0 && <ul className="check-results">
        {a.checks.map(c => <li key={c.command} className={c.passed ? 'passed' : 'failed'}>
          {c.passed ? <Check size={12} aria-hidden/> : <X size={12} aria-hidden/>}
          <span className="mono">{c.command}</span><small className="secondary">{engineText(c.detail)} · {t('{s} s', {s: (c.durationMs / 1000).toFixed(1)})}</small>
        </li>)}
      </ul>}
      {a.outcome === 'running' && <LiveActivity attemptId={a.id} agent={a.agent}/>}
      {a.handoffSent && <details><summary>{t('What it was told about earlier attempts')}</summary>
        <pre className="log-tail selectable">{a.handoffSent}</pre></details>}
      {failedCheck?.outputTail && <details><summary>{t('Check output')}</summary><pre className="log-tail">{failedCheck.outputTail}</pre></details>}
      {a.logTail && <details><summary>{t('Agent output')}</summary><pre className="log-tail">{a.logTail}</pre></details>}
    </div>
  </li>;
}

/** What the agent is doing now, line by line (live.rs): read files, commands it ran, what the model said. Polled every
 *  1.5 s while the attempt runs; it follows the newest line unless the reader scrolled up. */
function LiveActivity({attemptId, agent}: {attemptId: string; agent: string}) {
  const [activity, setActivity] = useState<Activity>({known: false, lines: [], dropped: 0, done: false});
  const box = useRef<HTMLPreElement>(null);
  const follow = useRef(true);
  useEffect(() => {
    let live = true;
    let last = 0;
    let busy = false;
    const tick = () => {
      if (busy) return;   // one request at a time, so lines never arrive twice or out of order
      busy = true;
      call(api => api.taskActivity(attemptId, last)).then(a => {
        busy = false;
        if (!live) return;
        const fresh = a.lines.filter(l => l.seq > last);
        if (fresh.length) last = fresh[fresh.length - 1].seq;
        setActivity(prev => ({...a, lines: [...prev.lines, ...fresh].slice(-500)}));
      }, () => { busy = false; });
    };
    void tick();
    const timer = setInterval(tick, 1500);
    return () => { live = false; clearInterval(timer); };
  }, [attemptId]);
  useEffect(() => { if (follow.current && box.current) box.current.scrollTop = box.current.scrollHeight; }, [activity.lines.length]);
  const streams = agent === 'opencode' || agent === 'codex';
  return <div className="live-activity">
    <div className="live-head"><span className="live-dot" aria-hidden/>{t('Live activity')}
      {activity.dropped > 0 && <small className="secondary">{t('{n} earlier lines not kept', {n: activity.dropped})}</small>}</div>
    {activity.lines.length === 0
      ? <p className="secondary">{streams ? t('Waiting for the agent’s first output…') : t('This agent reports what it did when it finishes; its output appears below then.')}</p>
      : <pre ref={box} className="log-tail live-log selectable" aria-live="polite"
          onScroll={e => { const el = e.currentTarget; follow.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24; }}>
          {activity.lines.map(l => `${new Date(l.atMs).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'})}  ${l.text}`).join('\n')}
        </pre>}
  </div>;
}

/** An answer task's result: the agent's last message, as it wrote it (nothing was changed or kept). */
function Answer({task}: {task: Detail}) {
  const done = [...task.attemptDetails].reverse().find(a => a.outcome === 'done');
  const text = done?.summary?.trim() ?? '';
  return <div className="group-box task-output">
    <div className="agent-result-head"><Badge tone="ok">{t('Answer')}</Badge>
      <span className="secondary">{done ? done.label : ''}</span><span className="spacer"/>
      {text && <CopyButton text={text} label={t('Copy')}/>}</div>
    {text ? <div className="answer-text selectable">{text}</div> : <p className="secondary">{t('The agent left no text.')}</p>}
  </div>;
}

/** A step's line on the receipt: what it cost by Pygg's ledger or the agent's estimate, or how it is paid. */
function stepCost(a: AttemptView): string {
  if (a.agentEstimateUsd !== null && a.agentEstimateUsd > 0) return `$${a.agentEstimateUsd.toFixed(a.agentEstimateUsd < 0.01 ? 4 : 2)}`;
  if (a.billing === 'free' || a.billing === 'local') return t('free');
  if (a.billing === 'subscription') return t('quota');
  if (a.outcome === 'skipped' || a.outcome === 'waiting') return '—';
  return a.agentEstimateUsd === 0 ? '$0.00' : t('unknown');
}

/** Paths that look like tests, so the card can say whether the change came with any. */
const TEST_PATH = /(^|[\/_.-])(tests?|spec|__tests__)([\/_.-]|$)/i;

/**
 * The review evidence (the gap study): for a result to decide on, or one kept, what was asked, where it started, what
 * changed, which checks ran on it, and what nothing checked, so a reviewer starts from facts instead of rebuilding them.
 */
function Evidence({task}: {task: Detail}) {
  const result = [...task.attemptDetails].reverse().find(a => a.outcome === 'passed' || a.outcome === 'done' || a.open);
  const plan = useGateway(api => task.brainstormId ? api.brainstorm(task.brainstormId) : Promise.resolve(null), [task.brainstormId ?? '']);
  if (!result || task.answer || task.state === 'running' || task.state === 'waiting_approval') return null;
  if (!(task.state === 'checks_passed' || task.state === 'needs_review' || task.result === 'kept' || task.state === 'manually_accepted')) return null;
  const card = plan.data?.proposal.cards.find(c => c.id === task.cardId);
  const paths = result.changedPaths ?? [];
  const tests = paths.filter(p => TEST_PATH.test(p));
  const failed = result.checks.filter(c => !c.passed);
  const unchecked: string[] = [];
  if (task.policy.checks.length === 0) unchecked.push(t('Nothing checked this result: the project has no approved checks.'));
  else {
    unchecked.push(t('The checks test only what their commands run; behaviour they do not cover is unchecked.'));
    if (paths.length > 0 && tests.length === 0) unchecked.push(t('No test file changed in this result.'));
  }
  if (card) unchecked.push(t('The card’s done criterion, to confirm yourself: {done}', {done: card.done}));
  const firstLine = task.prompt.split('\n').map(l => l.trim()).find(Boolean) ?? '';
  return <section className="group" aria-labelledby="evidence-heading">
    <h2 id="evidence-heading">{t('Evidence')}</h2>
    <dl className="group-box evidence">
      <dt>{t('Asked')}</dt><dd className="selectable">{firstLine.length > 240 ? `${firstLine.slice(0, 240)}…` : firstLine}</dd>
      <dt>{t('Started from')}</dt><dd className="mono selectable">{task.baseRevision?.slice(0, 10) ?? '—'} · {task.workspaceName}</dd>
      <dt>{t('Changed')}</dt><dd>{t('{files} files, +{ins} −{del}', {files: result.filesChanged ?? paths.length, ins: result.insertions ?? 0, del: result.deletions ?? 0})}
        {tests.length > 0 && ` · ${t('{n} test files', {n: tests.length})}`}
        {paths.length > 0 && <details><summary>{t('Files')}</summary><ul className="evidence-files mono">{paths.slice(0, 60).map(p => <li key={p}>{p}</li>)}</ul></details>}</dd>
      <dt>{t('Checked by')}</dt><dd>{result.checks.length === 0 ? <span className="warn-text">{t('no check ran')}</span>
        : <ul className="check-results">{result.checks.map(c => <li key={c.command} className={c.passed ? 'passed' : 'failed'}>
            {c.passed ? <Check size={12} aria-hidden/> : <X size={12} aria-hidden/>}<span className="mono">{c.command}</span></li>)}</ul>}
        {failed.length > 0 && <span className="warn-text">{t('{n} failed', {n: failed.length})}</span>}</dd>
      <dt>{t('Not checked')}</dt><dd><ul className="evidence-unchecked">{unchecked.map(u => <li key={u}>{u}</li>)}</ul></dd>
    </dl>
  </section>;
}

/** The task as a brief for another agent (goal, base, attempts and why they stopped, checks, where it stands), shown
 *  before it is copied: key-shaped text and passwords in URLs are masked, but the task's own words go as written. */
function BriefButton({taskId}: {taskId: string}) {
  const [text, setText] = useState<string | null>(null);
  const [status, setStatus] = useState('');
  const [error, setError] = useState<string | null>(null);
  // Throws when the copy fails, so the sheet stays open with the reason and the text to select by hand.
  const copy = async (value: string) => {
    if (!navigator.clipboard) throw t('Could not copy: select the text and copy it yourself.');
    try { await navigator.clipboard.writeText(value); } catch { throw t('Could not copy: select the text and copy it yourself.'); }
    setStatus(t('Brief copied.'));
  };
  return <>
    <button type="button" className="button" onClick={() => { setError(null); setStatus('');
      call(api => api.taskBrief(taskId)).then(setText, e => setError(errorText(e))); }}>{t('Brief for another agent…')}</button>
    {error && <ErrorNote>{error}</ErrorNote>}
    {text !== null && <ConfirmSheet destructive={false} title={t('Brief for another agent')} confirmLabel={t('Copy')}
      message={<>
        <p className="secondary">{t('Read it before sharing it. Key-shaped text and passwords in URLs are masked; the task’s own words and file names go as written.')}</p>
        <pre className="log-tail selectable brief-preview">{text}</pre>
        <p role="status" aria-live="polite">{status}</p></>}
      onClose={() => setText(null)} onConfirm={() => copy(text)}/>}
  </>;
}
