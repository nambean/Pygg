import { useEffect, useState } from 'react';
import { Check, ChevronLeft, FileDown, GitBranch, Play, RotateCcw, Square, Trash2, X } from 'lucide-react';
import { call, errorText, gatewayChanged } from '../lib/api';
import { taskCost } from '../lib/format';
import { t } from '../lib/i18n';
import { tokens } from '../lib/money';
import { AGENT_NAMES, type ApprovalView, type AttemptView, type TaskDetail as Detail } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { billingLabel, outcomeLabel } from './AttemptSequence';
import { DiffView } from './DiffView';
import { StateIcon, statusLine } from './TaskList';
import { Badge, ConfirmSheet, CopyButton, ErrorNote, FormRow, Spinner } from './ui';

/**
 * Task detail (SCREENS.md §5.2): what the task is waiting on, its result, and every attempt with its checks.
 * Reloads every two seconds while the task runs.
 */
export function TaskDetail({id, onBack}: {id: string; onBack: () => void}) {
  const [live, setLive] = useState(true);
  const loaded = useGateway(api => api.task(id).then(d => { setLive(d.state === 'running'); return d; }), [id], live ? 2_000 : undefined);
  const task = loaded.data;
  return <div className="detail">
    <button type="button" className="back-button" onClick={onBack}>
      <ChevronLeft size={15} strokeWidth={2} aria-hidden/>{t('Tasks')}
    </button>
    {loaded.error && <ErrorNote>{loaded.error}</ErrorNote>}
    {!task ? <Spinner/> : <>
      <header className="detail-header">
        <StateIcon state={task.state}/>
        <div>
          <h2 className="selectable">{task.title}</h2>
          <p>{statusLine(task)} · {task.workspaceName}</p>
        </div>
      </header>
      <Actions task={task} onChanged={() => { loaded.reload(); gatewayChanged(); }}/>

      <section className="group">
        <h2>{t('Attempts')}</h2>
        {task.attemptDetails.length === 0 ? <p className="group-footnote">{t('The first step is starting.')}</p>
          : <ol className="group-box attempt-rows">
            {task.attemptDetails.map(a => <AttemptRow key={a.id} attempt={a}/>)}
          </ol>}
        <p className="group-footnote">{t('Cost so far: {cost}', {cost: taskCost(task.cost)})}
          {task.cost.unknownCalls > 0 && ` · ${t('Agents run with their own sign-in, so their cost is unknown to Pygg.')}`}</p>
      </section>

      <section className="group">
        <h2>{t('Task')}</h2>
        <div className="group-box">
          <p className="agent-message selectable">{task.prompt}</p>
          <dl className="detail-list">
            <dt>{t('Steps')}</dt>
            <dd>{task.policy.steps.map((s, i) => `${i + 1}. ${AGENT_NAMES[s.agent] ?? s.agent}${s.model ? ` · ${s.model}` : ''} (T${s.tier}, ${billingLabel(s.billing)})`).join('  ')}</dd>
            <dt>{t('Checks')}</dt>
            <dd className="mono">{task.policy.checks.length ? task.policy.checks.map(c => c.command).join(' → ') : t('None approved')}</dd>
            <dt>{t('Started from')}</dt><dd className="mono selectable">{task.baseRevision?.slice(0, 10) ?? '—'} · {task.workspacePath}</dd>
          </dl>
        </div>
      </section>
    </>}
  </div>;
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
    {task.reason && <p className={task.state === 'checks_passed' || task.state === 'manually_accepted' ? 'group-footnote' : 'warning-note'}>{task.reason}</p>}
    {task.result === 'kept' && task.resultBranch && <p className="ok-note">
      {t('Kept as branch {branch}. Your checkout was not changed.', {branch: task.resultBranch})}</p>}
    {task.result === 'discarded' && <p className="group-footnote">{t('The result was discarded.')}</p>}
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
      <span className="mono">{t('Step {n}', {n: a.step + 1})} · {a.label}</span></div>
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
      <span className="mono">{t('Step {n}', {n: a.step + 1})} · {a.label}</span></div>
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
        <span className="mono selectable">{a.label}</span>
        <small className="secondary">{t('Step {n}', {n: a.step + 1})} · T{a.tier} · {billingLabel(a.billing)}
          {a.agentModel ? ` · ${a.agentModel}` : ''}</small>
        <span className="spacer"/>
        <span className="value">{a.outcome === 'running' ? <><Play size={11} aria-hidden/> {outcomeLabel(a.outcome)}</> : outcomeLabel(a.outcome)}</span>
      </div>
      {a.escalationReason && <p className="secondary">{a.escalationReason}</p>}
      {(a.filesChanged !== null || a.inputTokens !== null) && <p className="secondary">
        {a.filesChanged !== null && t('{files} files, +{ins} −{del}', {files: a.filesChanged, ins: a.insertions ?? 0, del: a.deletions ?? 0})}
        {a.inputTokens !== null && ` · ${t('{input} in · {output} out', {input: tokens(a.inputTokens), output: tokens(a.outputTokens ?? 0)})}`}
        {a.startedAt && a.finishedAt ? ` · ${t('{s} s', {s: ((a.finishedAt - a.startedAt) / 1000).toFixed(1)})}` : ''}
        {a.agentEstimateUsd !== null && a.agentEstimateUsd > 0 ? ` · ${t('agent’s estimate {amount}', {amount: `$${a.agentEstimateUsd.toFixed(2)}`})}` : ''}
        {a.capUsd !== null ? ` · ${t('cap {amount}', {amount: `$${a.capUsd.toFixed(2)}`})}` : ''}
      </p>}
      {a.checks.length > 0 && <ul className="check-results">
        {a.checks.map(c => <li key={c.command} className={c.passed ? 'passed' : 'failed'}>
          {c.passed ? <Check size={12} aria-hidden/> : <X size={12} aria-hidden/>}
          <span className="mono">{c.command}</span><small className="secondary">{c.detail} · {t('{s} s', {s: (c.durationMs / 1000).toFixed(1)})}</small>
        </li>)}
      </ul>}
      {failedCheck?.outputTail && <details><summary>{t('Check output')}</summary><pre className="log-tail">{failedCheck.outputTail}</pre></details>}
      {a.logTail && <details><summary>{t('Agent output')}</summary><pre className="log-tail">{a.logTail}</pre></details>}
    </div>
  </li>;
}
