import { useEffect, useState } from 'react';
import { ChevronLeft, Lightbulb } from 'lucide-react';
import { call, errorText, gatewayChanged, type AutoPlan } from '../lib/api';
import { t } from '../lib/i18n';
import { AGENT_NAMES, factLine, type Brainstorm, type BrainCard, type BrainRole, type RoleLine } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { Badge, ConfirmSheet, ErrorNote, Spinner } from './ui';
import './brain.css';

/**
 * A brainstorm (BRAIN.md §4.4): the planner pick when the rule's pick costs money, the questions (answered in the
 * coding agent, never here), the cards by wave with the model Pygg picked and why, and approval into tasks.
 * Nothing on this screen sends anything to a model except the pick, which the user makes.
 */
export function BrainstormDetail({id, onBack, onOpenTask}: {id: string; onBack: () => void; onOpenTask: (taskId: string) => void}) {
  const [live, setLive] = useState(true);
  const loaded = useGateway(api => api.brainstorm(id).then(b => { setLive(b.state === 'thinking' || b.state === 'waiting_pick'); return b; }),
    [id], live ? 2_000 : undefined);
  const b = loaded.data;
  const changed = () => { loaded.reload(); gatewayChanged(); };
  return <div className="detail">
    <button type="button" className="back-button" onClick={onBack}>
      <ChevronLeft size={15} strokeWidth={2} aria-hidden/>{t('Tasks')}
    </button>
    {loaded.error && <ErrorNote>{loaded.error}</ErrorNote>}
    {!b ? <Spinner/> : <>
      <header className="detail-header">
        <span className="state-icon waiting_approval"><Lightbulb size={18} strokeWidth={1.9} aria-hidden/></span>
        <div style={{minWidth: 0, flex: 1}}>
          <h2 className="selectable">{b.requirement.split('\n')[0]}</h2>
          <p><StateBadge state={b.state} revision={b.revision}/>{b.clientId ? ` · ${t('from a coding agent')}` : ''} · {t('session {session}', {session: b.session})}</p>
          <p>{t('Planner')}: <span className="brain-id">{b.planner ?? t('no planner yet')}</span> · {b.promptVersion}</p>
        </div>
      </header>

      {b.state === 'waiting_pick' && <PickPlanner b={b} onChanged={changed}/>}
      {b.state === 'thinking' && <p className="group-footnote"><Spinner label={t('The planner is reading the requirement…')}/></p>}
      {b.state === 'failed' && <section className="group">
        <ErrorNote>{b.error ?? t('The planner failed.')}</ErrorNote>
        {b.rawReply && <details className="form-disclosure"><summary>{t('What the planner answered')}</summary>
          <pre className="code-block selectable">{b.rawReply}</pre></details>}
        <PickPlanner b={b} onChanged={changed} retry/>
      </section>}
      {b.state === 'declined' && <p className="group-footnote">{t('Declined. Nothing was started.')}</p>}
      {b.state === 'approved' && <p className="ok-note">{t('Approved: its cards are tasks of this workspace now. Paid steps still wait for your approval when they start.')}</p>}

      {(b.state === 'proposed' || b.state === 'approved') && <>
        <Questions b={b}/>
        <Cards b={b} onChanged={changed}/>
        {b.state === 'proposed' && <Approve b={b} onChanged={changed} onOpenTask={onOpenTask}/>}
      </>}
    </>}
  </div>;
}

function StateBadge({state, revision}: {state: Brainstorm['state']; revision: number}) {
  switch (state) {
    case 'waiting_pick': return <Badge tone="warn">{t('Waits for your pick')}</Badge>;
    case 'thinking': return <Badge tone="info">{t('Thinking')}</Badge>;
    case 'proposed': return <Badge tone="info">{t('Proposed · revision {n}', {n: revision})}</Badge>;
    case 'approved': return <Badge tone="ok">{t('Approved')}</Badge>;
    case 'declined': return <Badge tone="warn">{t('Declined')}</Badge>;
    case 'failed': return <Badge tone="error">{t('Planner failed')}</Badge>;
  }
}

/** The inline pick (BRAIN.md 4.4): the plan role's ranking under the client's policy, the rule's line first. */
function PickPlanner({b, onChanged, retry}: {b: Brainstorm; onChanged: () => void; retry?: boolean}) {
  const candidates = useGateway(api => api.brainstormCandidates(b.id), [b.id]);
  const [choice, setChoice] = useState<string | null>(null);
  const [remember, setRemember] = useState(true);
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
            return <li key={name} className={picked === name ? 'selected' : ''}>
              <input type="radio" name="planner" checked={picked === name} onChange={() => setChoice(name)} aria-label={name}/>
              <div>
                <div className="brain-head"><span className="brain-id">{name}</span>
                  <span className="brain-meta">{r.tier ?? t('no tier')} · {r.billing}</span></div>
                <div className="brain-evidence">{i === 0 && <><span className="wins">{t('the rule’s pick')}</span> · </>}{r.reason}</div>
              </div>
            </li>;
          })}
        </ul>}
      {error && <ErrorNote>{error}</ErrorNote>}
      <div className="form-actions" style={{margin: '14px 10px 4px'}}>
        <label className="inline-field"><input type="checkbox" checked={remember} onChange={e => setRemember(e.target.checked)}/> {t('Remember this planner for every project')}</label>
        <span className="spacer"/>
        {!retry && <button type="button" className="button" disabled={busy} onClick={() => act(() => call(api => api.brainstormDecline(b.id)))}>{t('Decline')}</button>}
        <button type="button" className="button prominent" disabled={busy || !picked}
          onClick={() => picked && act(() => call(api => api.brainstormPickPlanner(b.id, picked, remember)))}>
          {picked ? t('Use {model}', {model: picked}) : t('Use')}
        </button>
      </div>
    </div>
    <p className="group-footnote">{t('The candidates are the plan role’s ranking: your runs, then the model facts feed, then the tier. Each line says which.')}</p>
  </section>;
}

function Questions({b}: {b: Brainstorm}) {
  const qs = b.proposal.questions;
  const answered = qs.filter(q => q.answered).length;
  return <section className="group brain-questions">
    <div className="group-heading"><h2>{t('Questions · {n}, {m} answered', {n: qs.length, m: answered})}</h2>
      <span className="brain-meta">{t('Answered in your coding agent, not here')}</span></div>
    {qs.length === 0 ? <p className="group-footnote">{t('The planner asked nothing.')}</p> : <div className="group-box">
      {qs.map((q, i) => <div key={q.id} className="q-row"><span className="n">{i + 1}.</span><span>{q.text}</span>
        {q.answered ? <span className="brain-meta">{t('answered')}</span> : <Badge tone="warn">{t('open')}</Badge>}</div>)}
    </div>}
  </section>;
}

const ROLE_NAMES: Record<BrainRole, string> = {plan: 'plan', code: 'code', test: 'test', review: 'review'};

function Cards({b, onChanged}: {b: Brainstorm; onChanged: () => void}) {
  const cards = b.proposal.cards;
  const waves = [...new Set(cards.map(c => c.wave))].sort((x, y) => x - y);
  const kept = cards.filter(c => !c.dropped).length;
  return <section className="group brain-cards">
    <div className="group-heading"><h2>{t('Cards · {kept} of {total}', {kept, total: cards.length})}{b.proposal.droppedCards > 0 && ` (${t('{n} left out: not in the schema', {n: b.proposal.droppedCards})})`}</h2>
      <span className="brain-meta">{t('Models picked by Pygg’s rules; the planner wrote the cards')}</span></div>
    <div className="group-box">
      {waves.map(wave => <div key={wave}>
        <div className="wave-head">{wave === 1 ? t('Wave 1 · depends on nothing') : t('Wave {n} · after wave {m}', {n: wave, m: wave - 1})}</div>
        {cards.filter(c => c.wave === wave).map(c => <CardRow key={c.id} b={b} card={c} onChanged={onChanged}/>)}
      </div>)}
    </div>
  </section>;
}

function CardRow({b, card, onChanged}: {b: Brainstorm; card: BrainCard; onChanged: () => void}) {
  const [changing, setChanging] = useState(false);
  const [adding, setAdding] = useState(false);
  const [lines, setLines] = useState<RoleLine[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editable = b.state === 'proposed';
  const act = (f: () => Promise<unknown>) => { setBusy(true); setError(null); return f().then(() => { setBusy(false); onChanged(); }, e => { setError(errorText(e)); setBusy(false); }); };
  useEffect(() => {
    if (!changing || lines) return;
    call(api => api.modelFactsRole(card.role)).then(setLines, e => setError(errorText(e)));
  }, [changing, lines, card.role]);
  const deps = card.dependsOn.filter(d => d.startsWith('q'));
  return <div className={`card-row${card.dropped ? ' dropped' : ''}`}>
    <div className="card-main">
      <div className="row-title"><span className="name">{card.title}</span><span className={`role-chip ${card.role}`}>{ROLE_NAMES[card.role]}</span>
        {deps.length > 0 && <span className="brain-meta">{t('needs {q} answered', {q: deps.join(', ')})}</span>}</div>
      <div className="card-line"><span className="brain-id">{card.model ?? t('nothing eligible')}</span>
        {changing && <select className="popup-button" aria-label={t('Model for this card')} disabled={busy || !lines}
          value={card.model ?? ''} onChange={e => { const model = e.target.value; const all = window.confirm(t('Apply {model} to every {role} card? Cancel applies it to this card only.', {model, role: card.role}));
            act(() => call(api => api.brainstormSetModel(b.id, card.id, model, all))).then(() => setChanging(false)); }}>
          {!lines && <option value={card.model ?? ''}>{t('Loading…')}</option>}
          {lines?.map(l => <option key={l.model} value={l.model}>{l.model} · {l.fact ? factLine(l.fact) : l.tier ? t('tier {tier} only', {tier: l.tier}) : t('no tier')}</option>)}
        </select>}
      </div>
      <div className="brain-evidence">{card.reason ?? ''}</div>
      {card.model && <ul className="brain-fallbacks" aria-label={t('Fallbacks')}>
        {card.fallbacks.map((f, i) => <li key={f.model}>
          <span className="brain-meta">{i === 0 ? t('then') : t('then')}</span> <span className="brain-id">{f.model}</span>
          <span className="brain-evidence"> · {f.reason}</span>
          {editable && <button type="button" className="link-button" disabled={busy} aria-label={t('Remove fallback {model}', {model: f.model})}
            onClick={() => act(() => call(api => api.brainstormSetCard(b.id, card.id, {fallbacks: card.fallbacks.filter(x => x.model !== f.model).map(x => x.model)})))}>{t('Remove')}</button>}
        </li>)}
        {editable && card.fallbacks.length < 5 && <li>
          {adding
            ? <select className="popup-button" aria-label={t('Add a fallback')} disabled={busy || !lines} value=""
                onChange={e => { const model = e.target.value; if (!model) return;
                  act(() => call(api => api.brainstormSetCard(b.id, card.id, {fallbacks: [...card.fallbacks.map(x => x.model), model]}))).then(() => setAdding(false)); }}>
                <option value="">{lines ? t('Pick a fallback…') : t('Loading…')}</option>
                {lines?.filter(l => l.model !== card.model && !card.fallbacks.some(f => f.model === l.model)).map(l =>
                  <option key={l.model} value={l.model}>{l.model} · {l.fact ? factLine(l.fact) : l.tier ? t('tier {tier} only', {tier: l.tier}) : t('no tier')}</option>)}
              </select>
            : <button type="button" className="link-button" disabled={busy} onClick={() => { setAdding(true); if (!lines) call(api => api.modelFactsRole(card.role)).then(setLines, e => setError(errorText(e))); }}>{t('Add a fallback…')}</button>}
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
        ? <button type="button" className="button" disabled={busy} onClick={() => act(() => call(api => api.brainstormSetCard(b.id, card.id, {dropped: false})))}>{t('Restore')}</button>
        : <>
          <button type="button" className="button" disabled={busy} onClick={() => setChanging(v => !v)}>{changing ? t('Keep') : t('Change…')}</button>
          <button type="button" className="button" disabled={busy} onClick={() => act(() => call(api => api.brainstormSetCard(b.id, card.id, {dropped: true})))}>{t('Drop')}</button>
        </>}
    </div>}
  </div>;
}

function Approve({b, onChanged, onOpenTask}: {b: Brainstorm; onChanged: () => void; onOpenTask: (id: string) => void}) {
  const workspaces = useGateway(api => api.workspaces());
  const agents = useGateway(api => api.agents());
  const [workspace, setWorkspace] = useState<string>(b.workspaceId ?? '');
  const [agent, setAgent] = useState<string>('');
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [made, setMade] = useState<string[] | null>(null);
  useEffect(() => { if (!workspace && workspaces.data?.[0]) setWorkspace(workspaces.data[0].id); }, [workspaces.data, workspace]);
  const installed = (agents.data ?? []).filter(a => a.installed);
  useEffect(() => { if (!agent && installed.length) setAgent(installed.find(a => a.id === 'opencode')?.id ?? installed[0].id); }, [installed, agent]);
  const kept = b.proposal.cards.filter(c => !c.dropped);
  const unpicked = kept.filter(c => !c.model).length;
  const ready = kept.length > 0 && unpicked === 0 && !!workspace && !!agent;
  if (made) return <div className="approve-bar">
    <span>{t('{n} tasks started', {n: made.length})}</span>
    {made.map((id, i) => <button key={id} type="button" className="link-button" onClick={() => onOpenTask(id)}>{t('Open task {n}', {n: i + 1})}</button>)}
  </div>;
  return <>
    <div className="approve-bar">
      <span>{t('{n} cards', {n: kept.length})}{unpicked > 0 && <span className="warn-text"> · {t('{n} without an eligible model', {n: unpicked})}</span>}</span>
      <label className="inline-field">{t('Workspace')}
        <select className="popup-button" value={workspace} onChange={e => setWorkspace(e.target.value)} aria-label={t('Workspace')}>
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
    {error && <ErrorNote>{error}</ErrorNote>}
    {confirming && <ConfirmSheet destructive={false} title={t('Approve {n} cards?', {n: kept.length})} confirmLabel={t('Approve')}
      message={<p>{t('Each card becomes a task of {workspace}, run by {agent} on the model shown, one step each, judged by the workspace’s approved checks. A paid step still waits for your approval when it starts.',
        {workspace: workspaces.data?.find(w => w.id === workspace)?.name ?? workspace, agent: AGENT_NAMES[agent] ?? agent})}</p>}
      onClose={() => setConfirming(false)}
      onConfirm={async () => { const ids = await call(api => api.brainstormApprove(b.id, workspace, agent)); setMade(ids); onChanged(); }}/>}
  </>;
}
