import { useEffect, useState } from 'react';
import { ChevronLeft, Lightbulb } from 'lucide-react';
import { call, errorText, gatewayChanged, type AutoPlan } from '../lib/api';
import { t } from '../lib/i18n';
import { AGENT_NAMES, type Brainstorm, type BrainCard, type BrainRole } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { Badge, ConfirmSheet, ErrorNote, Spinner } from './ui';
import { RoleSheet } from './RoleSheet';
import { usd } from '../lib/format';
import './brain.css';

/**
 * A brainstorm (BRAIN.md §4.4): the planner pick when the rule's pick costs money, the questions (answered here for
 * a plan started in the app, in the coding agent for one started there), the cards by wave with the model Pygg picked
 * and why, and approval into tasks. Only the pick and the user's answers send anything to a model.
 */
export function BrainstormDetail({id, onBack, onOpenBoard}: {id: string; onBack: () => void; onOpenBoard: (workspaceId: string) => void}) {
  const settings = useGateway(api => api.brainSettings());
  const [live, setLive] = useState(true);
  const loaded = useGateway(api => api.brainstorm(id).then(b => { setLive(b.state === 'thinking' || b.state === 'waiting_pick'); return b; }),
    [id], live ? 2_000 : undefined);
  const b = loaded.data;
  const changed = () => { loaded.reload(); gatewayChanged(); };
  return <div className="detail">
    <button type="button" className="back-button" onClick={onBack}>
      <ChevronLeft size={15} strokeWidth={2} aria-hidden/>{t('Tasks')}
    </button>
    {loaded.error && <ErrorNote>{loaded.error}{' '}<button type="button" className="link-button" onClick={loaded.reload} disabled={loaded.loading}>{loaded.loading ? t('Trying again…') : t('Try Again')}</button></ErrorNote>}
    {!b ? (loaded.error ? null : <Spinner/>) : <>
      <header className="detail-header">
        <span className="state-icon waiting_approval"><Lightbulb size={18} strokeWidth={1.9} aria-hidden/></span>
        <div style={{minWidth: 0, flex: 1}}>
          <h2 className="selectable">{b.requirement.split('\n')[0]}</h2>
          <p><StateBadge state={b.state} revision={b.revision}/>{b.session.startsWith('app-') ? ` · ${t('started here')}` : b.clientId ? ` · ${t('from a coding agent')}` : ''} · {t('session {session}', {session: b.session})}</p>
          <p>{t('Planner')}: <span className="brain-id">{b.planner ?? t('no planner yet')}</span> · {b.promptVersion}</p>
        </div>
      </header>

      {b.state === 'waiting_pick' && <PickPlanner b={b} onChanged={changed} remember={settings.data?.rememberPlanner ?? true}/>}
      {b.state === 'thinking' && <p className="group-footnote"><Spinner label={t('The planner is reading the requirement…')}/></p>}
      {b.state === 'failed' && <section className="group">
        <ErrorNote>{b.error ?? t('The planner failed.')}</ErrorNote>
        {b.rawReply && <details className="form-disclosure"><summary>{t('What the planner answered')}</summary>
          <pre className="code-block selectable">{b.rawReply}</pre></details>}
        {b.planner && <RetrySame b={b} onChanged={changed}/>}
        <PickPlanner b={b} onChanged={changed} retry remember={settings.data?.rememberPlanner ?? true}/>
      </section>}
      {b.state === 'declined' && <p className="group-footnote">{t('Declined. Nothing was started.')}</p>}
      {b.state === 'approved' && <div className="approve-bar"><span className="ok-note" style={{margin: 0}}>{t('Approved: its cards are on the workspace’s board. Paid steps still wait for your approval when they start.')}</span>
        <span className="spacer"/>{b.workspaceId && <button type="button" className="button prominent" onClick={() => onOpenBoard(b.workspaceId!)}>{t('Open Board')}</button>}</div>}

      {(b.state === 'proposed' || b.state === 'approved') && <>
        <Questions b={b} onChanged={changed}/>
        <Cards b={b} onChanged={changed} changeScope={settings.data?.changeScope ?? 'ask'}/>
        {b.state === 'proposed' && <Approve b={b} onChanged={changed} onOpenBoard={onOpenBoard}/>}
      </>}
    </>}
  </div>;
}

/** Runs the planner again as it is (BRAIN.md 4.4, Retry); the pick below is the other way out. */
function RetrySame({b, onChanged}: {b: Brainstorm; onChanged: () => void}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return <div className="form-actions" style={{margin: '10px 0 4px'}}>
    <button type="button" className="button prominent" disabled={busy}
      onClick={() => { setBusy(true); setError(null); call(api => api.brainstormRetry(b.id)).then(() => { setBusy(false); onChanged(); }, e => { setError(errorText(e)); setBusy(false); }); }}>
      {t('Retry with {model}', {model: b.planner ?? ''})}</button>
    <span className="brain-meta">{t('or pick another planner below')}</span>
    {error && <ErrorNote>{error}</ErrorNote>}
  </div>;
}

function StateBadge({state, revision}: {state: Brainstorm['state']; revision: number}) {
  switch (state) {
    case 'waiting_pick': return <Badge tone="warn">{t('Waits for your pick')}</Badge>;
    case 'thinking': return <Badge tone="info">{t('Thinking')}</Badge>;
    case 'proposed': return <Badge tone="info">{t('Proposed · revision {n}', {n: revision})}</Badge>;
    case 'approved': return <Badge tone="ok">{t('Approved · board')}</Badge>;
    case 'declined': return <Badge tone="warn">{t('Declined')}</Badge>;
    case 'failed': return <Badge tone="error">{t('Planner failed')}</Badge>;
  }
}

/** The inline pick (BRAIN.md 4.4): the plan role's ranking under the client's policy, the rule's line first. */
function PickPlanner({b, onChanged, retry, remember: rememberDefault}: {b: Brainstorm; onChanged: () => void; retry?: boolean; remember: boolean}) {
  const candidates = useGateway(api => api.brainstormCandidates(b.id), [b.id]);
  const [choice, setChoice] = useState<string | null>(null);
  const [sheet, setSheet] = useState(false);
  const [remember, setRemember] = useState(rememberDefault);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const plan: AutoPlan | null = candidates.data;
  const picked = choice ?? (plan?.ranked[0] ? `${plan.ranked[0].providerId}/${plan.ranked[0].modelId}` : null);
  const act = (f: () => Promise<unknown>) => { setBusy(true); setError(null); f().then(() => { setBusy(false); onChanged(); }, e => { setError(errorText(e)); setBusy(false); }); };
  return <section className="group brain-pick-section">
    <h2>{retry ? t('Pick another planner') : t('Pick a planner')}</h2>
    <div className="group-box" style={{padding: 12}}>
      <p className="secondary" style={{fontSize: 12}}>
        {t('The brain needs a model to read your requirement. This is the first paid request Pygg would start on its own, so it waits for you.')}
        {b.note ? ` ${b.note}.` : ''}
      </p>
      {candidates.error && <ErrorNote>{candidates.error}</ErrorNote>}
      {!plan ? <Spinner/> : plan.ranked.length === 0
        ? <p className="warn-text">{t('No connected model is eligible as a planner under this client’s policy.')} {plan.excluded.slice(0, 3).map(e => `${e.providerId}/${e.modelId}: ${e.reason}`).join(' · ')}</p>
        : <ul className="brain-pick" role="radiogroup" aria-label={t('Planner')}>
          {plan.ranked.map((r, i) => {
            const name = `${r.providerId}/${r.modelId}`;
            return <li key={name} className={picked === name ? 'selected' : ''}><label>
              <input type="radio" name="planner" checked={picked === name} onChange={() => setChoice(name)}/>
              <div>
                <div className="brain-head"><span className="brain-id">{name}</span>
                  <span className="brain-meta">{r.tier ?? t('no tier')} · {r.billing}{r.estimateMicroUsd != null && ` · ≈${usd(r.estimateMicroUsd / 1e6)} ${t('per brainstorm')}`}</span></div>
                <div className="brain-evidence">{i === 0 && <><span className="wins">{t('the rule’s pick')}</span> · </>}{r.reason}</div>
              </div>
            </label></li>;
          })}
        </ul>}
      {error && <ErrorNote>{error}</ErrorNote>}
      <button type="button" className="link-button" style={{margin: '8px 10px 0'}} onClick={() => setSheet(true)}>{t('Why these models? Every model for plan, with its evidence…')}</button>
      {sheet && <RoleSheet role="plan" brainstormId={b.id} current={picked} onPick={model => setChoice(model)} onClose={() => setSheet(false)}/>}
      <div className="form-actions" style={{margin: '14px 10px 4px'}}>
        <label className="inline-field"><input type="checkbox" checked={remember} onChange={e => setRemember(e.target.checked)}/> {t('Remember this planner for every project')}</label>
        <span className="spacer"/>
        <button type="button" className="button" disabled={busy} onClick={() => act(() => call(api => api.brainstormDecline(b.id)))}>{t('Decline')}</button>
        <button type="button" className="button prominent" disabled={busy || !picked}
          onClick={() => picked && act(() => call(api => api.brainstormPickPlanner(b.id, picked, remember)))}>
          {picked ? t('Use {model}', {model: picked}) : t('Use')}
        </button>
      </div>
    </div>
    <p className="group-footnote">{t('The candidates are the plan role’s ranking: your runs, then the model facts feed, then the tier. Each line says which.')}</p>
  </section>;
}

function Questions({b, onChanged}: {b: Brainstorm; onChanged: () => void}) {
  const qs = b.proposal.questions;
  const answered = qs.filter(q => q.answered).length;
  const here = b.session.startsWith('app-') && b.state === 'proposed';
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [other, setOther] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const text = [...qs.filter(q => !q.answered && answers[q.id]?.trim()).map(q => `${q.id}: ${answers[q.id].trim()}`),
    ...(other.trim() ? [`Changes: ${other.trim()}`] : [])].join('\n');
  const send = () => {
    setBusy(true); setError(null);
    call(api => api.brainstormRefine(b.id, text)).then(() => { setBusy(false); setAnswers({}); setOther(''); onChanged(); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  return <section className="group brain-questions">
    <div className="group-heading"><h2>{t('Questions · {n}, {m} answered', {n: qs.length, m: answered})}</h2>
      {!here && <span className="brain-meta">{t('Answered in your coding agent, not here')}</span>}</div>
    {qs.length === 0 ? <p className="group-footnote">{t('The planner asked nothing.')}</p> : <div className="group-box">
      {qs.map(q => <div key={q.id} className="q-row"><span className="n">{q.id}</span><span>{q.text}
        {here && !q.answered && <input className="q-answer" aria-label={t('Answer to {q}', {q: q.id})} value={answers[q.id] ?? ''}
          placeholder={t('Your answer')} onChange={e => setAnswers(a => ({...a, [q.id]: e.target.value}))}/>}</span>
        {q.answered ? <span className="brain-meta">{t('answered')}</span> : <Badge tone="warn">{t('open')}</Badge>}</div>)}
    </div>}
    {here && <div className="brain-refine">
      <label htmlFor={`refine-${b.id}`}>{t('Anything else to change in the plan')}</label>
      <textarea id={`refine-${b.id}`} rows={2} value={other} onChange={e => setOther(e.target.value)}
        placeholder={t('For example: one card for the API, another for the page; leave out the export.')}/>
      <div className="form-actions"><button type="button" className="button prominent" disabled={busy || !text} onClick={send}>
        {busy ? t('Sending…') : t('Send to the planner')}</button>
        <span className="brain-meta">{t('The planner writes a new revision; open questions you leave blank stay open.')}</span></div>
      {error && <ErrorNote>{error}</ErrorNote>}
    </div>}
  </section>;
}

const ROLE_NAMES: Record<BrainRole, string> = {plan: 'plan', code: 'code', test: 'test', review: 'review'};

function Cards({b, onChanged, changeScope}: {b: Brainstorm; onChanged: () => void; changeScope: 'ask' | 'card' | 'role'}) {
  const cards = b.proposal.cards;
  const waves = [...new Set(cards.map(c => c.wave))].sort((x, y) => x - y);
  const kept = cards.filter(c => !c.dropped).length;
  return <section className="group brain-cards">
    <div className="group-heading"><h2>{t('Cards · {kept} kept of {total}', {kept, total: cards.length})}{b.proposal.droppedCards > 0 && ` · ${t('{n} more rejected by the schema', {n: b.proposal.droppedCards})}`}</h2>
      <span className="brain-meta">{t('Models picked by Pygg’s rules; the planner wrote the cards')}</span></div>
    <div className="group-box">
      {waves.map(wave => <div key={wave}>
        <div className="wave-head">{wave === 1 ? t('Wave 1 · depends on nothing') : t('Wave {n} · after wave {m}', {n: wave, m: wave - 1})}</div>
        {cards.filter(c => c.wave === wave).map(c => <CardRow key={c.id} b={b} card={c} onChanged={onChanged} changeScope={changeScope}/>)}
      </div>)}
    </div>
  </section>;
}

function CardRow({b, card, onChanged, changeScope}: {b: Brainstorm; card: BrainCard; onChanged: () => void; changeScope: 'ask' | 'card' | 'role'}) {
  const [sheet, setSheet] = useState<'change' | 'fallback' | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editable = b.state === 'proposed';
  const act = (f: () => Promise<unknown>) => { setBusy(true); setError(null); return f().then(() => { setBusy(false); onChanged(); }, e => { setError(errorText(e)); setBusy(false); }); };
  const apply = (model: string, all: boolean) => act(() => call(api => api.brainstormSetModel(b.id, card.id, model, all))).then(() => setPending(null));
  const deps = card.dependsOn.filter(d => d.startsWith('q'));
  return <div className={`card-row${card.dropped ? ' dropped' : ''}`}>
    <div className="card-main">
      <div className="row-title"><span className="name">{card.title}</span><span className={`role-chip ${card.role}`}>{ROLE_NAMES[card.role]}</span>
        {deps.length > 0 && <span className="brain-meta">{t('needs {q} answered', {q: deps.join(', ')})}</span>}</div>
      <div className="card-line"><span className="brain-id">{card.model ?? t('nothing eligible')}</span></div>
      {sheet === 'change' && <RoleSheet role={card.role} brainstormId={b.id} current={card.model} onClose={() => setSheet(null)}
        onPick={model => { if (changeScope === 'ask') { setPending(model); return; } return apply(model, changeScope === 'role'); }}/>}
      {sheet === 'fallback' && <RoleSheet role={card.role} brainstormId={b.id} exclude={[card.model ?? '', ...card.fallbacks.map(f => f.model)]} onClose={() => setSheet(null)}
        onPick={model => act(() => call(api => api.brainstormSetCard(b.id, card.id, {fallbacks: [...card.fallbacks.map(x => x.model), model]})))}/>}
      {pending && <div className="scope-choice" role="group" aria-label={t('Apply {model} to', {model: pending})}>
        <span>{t('Apply {model} to', {model: pending})}</span>
        <button type="button" className="button" disabled={busy} onClick={() => apply(pending, false)}>{t('This card')}</button>
        <button type="button" className="button" disabled={busy} onClick={() => apply(pending, true)}>{t('All {role} cards', {role: card.role})}</button>
        <button type="button" className="link-button" disabled={busy} onClick={() => setPending(null)}>{t('Cancel')}</button>
      </div>}
      <div className="brain-evidence">{card.reason ?? ''}{card.model && card.estimateMicroUsd == null && ` · ${t('unpriced')}`}</div>
      {card.model && <ul className="brain-fallbacks" aria-label={t('Fallbacks')}>
        {card.fallbacks.map((f, i) => <li key={f.model}>
          <span className="brain-meta">{i === 0 ? t('then') : t('then')}</span> <span className="brain-id">{f.model}</span>
          <span className="brain-evidence"> · {f.reason}</span>
          {editable && <button type="button" className="link-button" disabled={busy} aria-label={t('Remove fallback {model}', {model: f.model})}
            onClick={() => act(() => call(api => api.brainstormSetCard(b.id, card.id, {fallbacks: card.fallbacks.filter(x => x.model !== f.model).map(x => x.model)})))}>{t('Remove')}</button>}
        </li>)}
        {editable && card.fallbacks.length < 5 && <li>
          <button type="button" className="link-button" disabled={busy} aria-label={t('Add a fallback to {title}', {title: card.title})} onClick={() => setSheet('fallback')}>{t('Add a fallback…')}</button>
          <span className="brain-meta"> {t('tried in order when an attempt fails: a connection error, a spent quota, a failed check')}</span>
        </li>}
      </ul>}
      <details>
        <summary>{t('Details')}</summary>
        <dl>
          <dt>{t('Done when')}</dt><dd>{card.done}</dd>
          <dt>{t('Depends on')}</dt><dd>{card.dependsOn.length ? card.dependsOn.join(', ') : t('nothing')}</dd>
          <dt>{t('Runner')}</dt><dd>{editable
            ? <select className="popup-button" value={card.runner} aria-label={t('Runner')} disabled={busy}
                onChange={e => act(() => call(api => api.brainstormSetCard(b.id, card.id, {runner: e.target.value as 'engine' | 'ide'})))}>
                <option value="engine">{t('Pygg engine (default)')}</option><option value="ide">{t('My IDE agent')}</option></select>
            : card.runner === 'engine' ? t('Pygg engine') : t('My IDE agent')}</dd>
          <dt>{t('Class')}</dt><dd>{card.class}</dd>
        </dl>
      </details>
      {error && <ErrorNote>{error}</ErrorNote>}
    </div>
    {editable && <div className="card-actions">
      {card.dropped
        ? <button type="button" className="button" disabled={busy} aria-label={t('Restore {title}', {title: card.title})} onClick={() => act(() => call(api => api.brainstormSetCard(b.id, card.id, {dropped: false})))}>{t('Restore')}</button>
        : <>
          <button type="button" className="button" disabled={busy} aria-label={t('Change the model of {title}', {title: card.title})} onClick={() => { setPending(null); setSheet('change'); }}>{t('Change…')}</button>
          <button type="button" className="button" disabled={busy} aria-label={t('Drop {title}', {title: card.title})} onClick={() => act(() => call(api => api.brainstormSetCard(b.id, card.id, {dropped: true})))}>{t('Drop')}</button>
        </>}
    </div>}
  </div>;
}

function Approve({b, onChanged, onOpenBoard}: {b: Brainstorm; onChanged: () => void; onOpenBoard: (workspaceId: string) => void}) {
  const workspaces = useGateway(api => api.workspaces());
  const agents = useGateway(api => api.agents());
  const budget = useGateway(api => api.taskBudget());
  const boards = useGateway(api => api.boards());
  const settings = useGateway(api => api.brainSettings());
  const [workspace, setWorkspace] = useState<string>(b.workspaceId ?? '');
  const [agent, setAgent] = useState<string>('');
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [made, setMade] = useState<string[] | null>(null);
  const installed = (agents.data ?? []).filter(a => a.installed);
  useEffect(() => { if (!agent && installed.length) setAgent(installed.find(a => a.id === 'opencode')?.id ?? installed[0].id); }, [installed, agent]);
  const kept = b.proposal.cards.filter(c => !c.dropped);
  const unpicked = kept.filter(c => !c.model).length;
  const known = kept.reduce((sum, c) => sum + (c.estimateMicroUsd ?? 0), 0) / 1e6;
  const unpriced = kept.filter(c => c.model && c.estimateMicroUsd == null).length;
  const runningLimit = boards.data?.find(x => x.workspaceId === workspace)?.limits?.wipRunning ?? settings.data?.wipRunning ?? 1;
  const left = budget.data ? (budget.data.limitUsd == null ? null : Math.max(0, budget.data.limitUsd - budget.data.recordedUsd - budget.data.reservedUsd)) : undefined;
  const estimateLine = [
    unpriced === kept.length ? t('Estimate: Unknown (no card is priced)') : t('Known estimate ≈{usd} for one run of every card', {usd: usd(known)}) + (unpriced ? ` · ${t('unpriced cards: {n}', {n: unpriced})}` : ''),
    left === undefined ? null : left === null ? t('no daily task limit') : t('today’s task budget left {usd}', {usd: usd(left)}),
    t('Running limit {n}', {n: runningLimit}),
  ].filter(Boolean).join(' · ');
  const ready = kept.length > 0 && unpicked === 0 && !!workspace && !!agent;
  if (made) return <div className="approve-bar">
    <span>{t('{n} cards on the board; the first ready ones are being pulled into Running', {n: made.length})}</span>
    <span className="spacer"/>
    <button type="button" className="button prominent" onClick={() => onOpenBoard(workspace)}>{t('Open Board')}</button>
  </div>;
  return <>
    <div className="approve-bar sticky">
      <span className="estimate brain-meta">{estimateLine}</span>
      <span>{t('{n} cards', {n: kept.length})}{unpicked > 0 && <span className="warn-text"> · {t('{n} without an eligible model', {n: unpicked})}</span>}</span>
      <label className="inline-field">{t('Project')}
        <select className="popup-button" value={workspace} onChange={e => setWorkspace(e.target.value)} aria-label={t('Project')}>
          {!workspace && <option value="">{t('Choose a project…')}</option>}
          {(workspaces.data ?? []).map(w => <option key={w.id} value={w.id}>{w.name}</option>)}
        </select></label>
      <label className="inline-field">{t('Agent')}
        <select className="popup-button" value={agent} onChange={e => setAgent(e.target.value)} aria-label={t('Agent')}>
          {installed.map(a => <option key={a.id} value={a.id}>{AGENT_NAMES[a.id] ?? a.name}</option>)}
        </select></label>
      <span className="spacer"/>
      <button type="button" className="button" disabled={busy} onClick={() => { setBusy(true); call(api => api.brainstormDecline(b.id)).then(() => { setBusy(false); onChanged(); }, e => { setError(errorText(e)); setBusy(false); }); }}>{t('Decline brainstorm')}</button>
      <button type="button" className="button prominent" disabled={!ready || busy} onClick={() => setConfirming(true)}>{t('Approve {n} cards…', {n: kept.length})}</button>
    </div>
    {agent && agent !== 'opencode' && <p className="group-footnote">{t('Only OpenCode through Pygg runs a card on the model Pygg picked. {agent} runs with its own sign-in and model; the pick is told to it.', {agent: AGENT_NAMES[agent] ?? agent})}</p>}
    {installed.length === 0 && agents.data && <p className="warn-text">{t('No agent is installed; install one on Agents to run the cards.')}</p>}
    {workspaces.data?.length === 0 && <p className="warn-text">{t('No project yet: add the repository under Projects first, then approve here.')}</p>}
    {!workspace && (workspaces.data?.length ?? 0) > 0 && <p className="group-footnote">{t('Choose the project (repository) this plan is for; its cards run there.')}</p>}
    {error && <ErrorNote>{error}</ErrorNote>}
    {confirming && <ConfirmSheet destructive={false} title={t('Approve {n} cards?', {n: kept.length})} confirmLabel={t('Approve')}
      message={<p>{agent === 'opencode'
          ? t('Each card becomes a card on the board of {workspace}; when pulled, {agent} runs it on the model shown, then its fallbacks, judged by the workspace’s approved checks.', {workspace: workspaces.data?.find(w => w.id === workspace)?.name ?? workspace, agent: AGENT_NAMES[agent] ?? agent})
          : t('Each card becomes a card on the board of {workspace}; when pulled, {agent} runs it with its own sign-in and model (the pick is told to it), judged by the workspace’s approved checks.', {workspace: workspaces.data?.find(w => w.id === workspace)?.name ?? workspace, agent: AGENT_NAMES[agent] ?? agent})}
        {' '}{estimateLine}. {t('A paid step still waits for your approval when it starts.')}</p>}
      onClose={() => setConfirming(false)}
      onConfirm={async () => { const ids = await call(api => api.brainstormApprove(b.id, workspace, agent)); setMade(ids); onChanged(); }}/>}
  </>;
}
