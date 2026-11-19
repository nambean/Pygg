import { useState } from 'react';
import { call, errorText } from '../lib/api';
import { ago } from '../lib/format';
import { t } from '../lib/i18n';
import { evidenceLine, type BrainPick, type BrainRole, type BrainSettings } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { CodeBlock, ErrorNote, Spinner, Switch } from './ui';
import './brain.css';

const ROLES: BrainRole[] = ['plan', 'code', 'test', 'review'];

/**
 * Brain Storming (BRAIN.md §4.7b): set up once, applied to every project. The picks by role, the defaults for a new
 * brainstorm, the model facts feed and the brain prompt, the projects that differ, and how a brainstorm starts.
 * Settings only: nothing here sends anything to a model.
 */
export function BrainStorming({onSkills, onWorkspaces}: {onSkills: () => void; onWorkspaces: () => void}) {
  const settings = useGateway(api => api.brainSettings());
  const picks = useGateway(api => api.brainPicks());
  const facts = useGateway(api => api.modelFactsStatus());
  const workspaces = useGateway(api => api.workspaces());
  const boards = useGateway(api => api.boards());
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const act = (f: () => Promise<unknown>, done?: () => void) => { setBusy(true); setError(null); f().then(() => { setBusy(false); done?.(); }, e => { setError(errorText(e)); setBusy(false); }); };
  const s = settings.data;
  const save = (change: Partial<BrainSettings>) => s && act(() => call(api => api.setBrainSettings({...s, ...change})), settings.reload);
  return <div className="page">
    <p className="page-intro secondary" style={{fontSize: 12, marginBottom: 16}}>{t('Set up once; every project uses it unless the project overrides it below. The LLM writes the cards; these picks and the rules choose the models; you approve before money is spent.')}</p>
    {error && <ErrorNote>{error}</ErrorNote>}

    <section className="group">
      <div className="group-heading"><h2>{t('Models by role')}</h2>
        <span className="brain-meta">{t('A saved pick wins over your runs, the facts feed and the tier. Clear it to let the rules decide.')}</span></div>
      <div className="group-box">
        {ROLES.map(role => <RoleRow key={role} role={role} pick={picks.data?.find(p => p.scope === 'global' && p.role === role) ?? null} busy={busy}
          onSet={model => act(() => call(api => api.setBrainPick('global', role, model)), picks.reload)}/>)}
      </div>
    </section>

    <section className="group">
      <h2>{t('Defaults for a new brainstorm')}</h2>
      {!s ? <Spinner/> : <div className="group-box">
        <Row label={t('Runner')} hint={t('Who runs a card unless the card says otherwise')}>
          <select className="popup-button" value={s.runner} disabled={busy} aria-label={t('Runner')} onChange={e => save({runner: e.target.value as 'engine' | 'ide'})}>
            <option value="engine">{t('Pygg engine')}</option><option value="ide">{t('My IDE agent')}</option></select></Row>
        <Row label={t('WIP limit · Running')} hint={t('Paid steps in flight at once; never below 1')}>
          <input className="text-input" type="number" min={1} style={{width: 52, textAlign: 'center'}} defaultValue={s.wipRunning} disabled={busy} aria-label={t('WIP limit · Running')}
            onBlur={e => { const n = Math.max(1, Number(e.target.value) || 1); if (n !== s.wipRunning) save({wipRunning: n}); }}/></Row>
        <Row label={t('WIP limit · Checking')} hint={t('Runs whose agent is done wait before their checks while Checking is full')}>
          <input className="text-input" type="number" min={1} style={{width: 52, textAlign: 'center'}} defaultValue={s.wipChecking} disabled={busy} aria-label={t('WIP limit · Checking')}
            onBlur={e => { const n = Math.max(1, Number(e.target.value) || 1); if (n !== s.wipChecking) save({wipChecking: n}); }}/></Row>
        <Row label={t('Remember a planner picked in a brainstorm')} hint={t('Saves it as the plan role above')}>
          <Switch checked={s.rememberPlanner} disabled={busy} label={t('Remember a planner picked in a brainstorm')} onChange={v => save({rememberPlanner: v})}/></Row>
        <Row label={t('Changing a card’s model')} hint={t('What Change… offers first')}>
          <select className="popup-button" value={s.changeScope} disabled={busy} aria-label={t('Changing a card’s model')} onChange={e => save({changeScope: e.target.value as BrainSettings['changeScope']})}>
            <option value="ask">{t('Ask: this card or all of its role')}</option><option value="card">{t('This card only')}</option><option value="role">{t('All cards of its role')}</option></select></Row>
      </div>}
    </section>

    <section className="group">
      <h2>{t('Facts and prompt')}</h2>
      {!facts.data ? <Spinner/> : <div className="group-box">
        <Row label={t('Model facts')} hint={facts.data.built
            ? t('Role ranks from public benchmarks (ADR-040) · built {built}{fetched}{stale}{err}', {built: facts.data.built.slice(0, 10), fetched: facts.data.fetchedAt ? ` · ${t('fetched {ago}', {ago: ago(facts.data.fetchedAt)})}` : '',
                stale: facts.data.stale ? ` · ${t('older than the weekly cadence')}` : '', err: facts.data.lastError ? ` · ${t('last fetch failed: {e}', {e: facts.data.lastError})}` : ''})
            : t('No feed kept yet; roles rank by tier{err}', {err: facts.data.lastError ? ` · ${t('last fetch failed: {e}', {e: facts.data.lastError})}` : ''})}>
          <Switch checked={facts.data.enabled} disabled={busy} label={t('Model facts')} onChange={v => act(() => call(api => api.modelFactsSetEnabled(v)), facts.reload)}/>
          <button type="button" className="button" disabled={busy} onClick={() => act(() => call(api => api.modelFactsRefresh()), facts.reload)}>{t('Refresh')}</button></Row>
        <div className="group-row" style={{alignItems: 'flex-start'}}>
          <div className="group-label"><label>{t('Sources')}</label><small>{t('Each with its licence; a fact older than a year is shown, never ranked')}</small></div>
          <div className="group-control" style={{maxWidth: '70%', flexDirection: 'column', alignItems: 'flex-start', gap: 2, fontSize: 12}}>
            {facts.data.sources.length === 0 && <span className="secondary">{t('none kept')}</span>}
            {facts.data.sources.map(src => <span key={src.id} className="selectable">{src.name} · {src.roles.join(', ')} · {src.licence} · {src.rows ?? '?'} {t('rows')} · {src.observed ?? '—'}{src.status !== 'ok' ? ` · ${t('carried forward')}` : ''}</span>)}
          </div>
        </div>
        <Row label={t('Brain prompt')} hint={t('What the planner is asked; a prompt in Skills & Prompts')}>
          <span className="mono">brain-v1</span><button type="button" className="button" onClick={onSkills}>{t('Open')}</button></Row>
      </div>}
    </section>

    <section className="group">
      <div className="group-heading"><h2>{t('Projects that differ')}</h2>
        <span className="brain-meta">{t('{n} workspaces', {n: workspaces.data?.length ?? 0})}</span></div>
      <div className="group-box">
        {(workspaces.data ?? []).map(w => {
          const own = (picks.data ?? []).filter(p => p.scope === w.id);
          const limits = boards.data?.find(b => b.workspaceId === w.id)?.limits ?? null;
          const differs = [...own.map(p => `${p.role} → ${p.model}`), ...(limits ? [t('Running {r} · Checking {c}', {r: limits.wipRunning, c: limits.wipChecking})] : [])];
          return <div key={w.id} className="group-row">
            <div className="group-label"><label>{w.name}</label><small>{differs.length ? differs.join(' · ') : t('Uses the defaults above')}</small></div>
            <div className="group-control">
              {own.length > 0 && <button type="button" className="button" disabled={busy} onClick={() => act(async () => { for (const p of own) await call(api => api.setBrainPick(w.id, p.role, null)); }, picks.reload)}>{t('Use defaults')}</button>}
              <button type="button" className="button" onClick={onWorkspaces}>{t('Workspaces')}</button>
            </div>
          </div>;
        })}
        {workspaces.data?.length === 0 && <p className="group-footnote">{t('No workspace yet; a brainstorm from an IDE adds its repository by itself.')}</p>}
      </div>
      <p className="group-footnote">{t('A project’s own picks come from Change… in its brainstorms and its WIP limits from Limit… on its board; Use defaults clears the picks, and the board’s Use the default limits clears the limits.')}</p>
    </section>

    <section className="group">
      <h2>{t('How it starts')}</h2>
      <div className="group-box">
        <div className="group-row" style={{alignItems: 'flex-start'}}>
          <div className="group-label"><label>{t('Start a brainstorm from your coding agent')}</label>
            <small>{t('There is no text box here. Start the agent through Pygg on the model pygg/brain (set up on CLI Tools) and type the requirement; the proposal shows up under Tasks.')}</small></div>
          <div className="group-control brain-start" style={{maxWidth: '60%', minWidth: 0, flex: 1}}>
            <CodeBlock code={'pygg run --model pygg/brain claude\ncodex --profile pygg -m pygg/brain\npygg run opencode   # /models → Pygg → pygg/brain'} label={t('Start a brainstorm')}/></div>
        </div>
      </div>
    </section>
  </div>;
}

function Row({label, hint, children}: {label: string; hint: string; children: React.ReactNode}) {
  return <div className="group-row">
    <div className="group-label"><label>{label}</label><small>{hint}</small></div>
    <div className="group-control">{children}</div>
  </div>;
}

function RoleRow({role, pick, busy, onSet}: {role: BrainRole; pick: BrainPick | null; busy: boolean; onSet: (model: string | null) => void}) {
  const [changing, setChanging] = useState(false);
  const lines = useGateway(api => api.modelFactsRole(role), [role]);
  const line = pick ? lines.data?.find(l => l.model === pick.model) ?? null : lines.data?.[0] ?? null;
  return <div className="group-row" style={{alignItems: 'flex-start'}}>
    <div className="group-label" style={{flex: '0 0 120px'}}><span className={`role-chip ${role}`}>{role}</span>
      <small style={{marginTop: 4}}>{pick ? t('Your saved pick') : t('By the rules today (no saved pick)')}</small></div>
    <div style={{flex: 1, minWidth: 0}}>
      <div className="card-line" style={{marginTop: 0}}><span className="brain-id">{pick?.model ?? (lines.data ? lines.data[0]?.model ?? t('nothing eligible: connect a provider or loosen the policy') : t('Loading…'))}</span>
        {line && <span className="brain-meta">{line.tier ?? t('no tier')} · {line.billing}</span>}</div>
      {line && <div className="brain-evidence">{evidenceLine(line)}</div>}
      {pick && lines.data && !line && <div className="warn-text" style={{fontSize: 11}}>{t('No longer eligible: not connected, turned off, or below the policy. Pick again or clear.')}</div>}
      {changing && <select className="popup-button" style={{marginTop: 6, maxWidth: '100%'}} aria-label={t('Model for {role}', {role})} disabled={busy || !lines.data} value={pick?.model ?? ''}
        onChange={e => { onSet(e.target.value || null); setChanging(false); }}>
        <option value="">{lines.data ? t('By the rules') : t('Loading…')}</option>
        {lines.data?.map(l => <option key={l.model} value={l.model}>{l.model} · {evidenceLine(l)}</option>)}
      </select>}
    </div>
    <div className="row-buttons" style={{display: 'flex', gap: 6, flexShrink: 0}}>
      <button type="button" className="button" disabled={busy} onClick={() => setChanging(v => !v)}>{changing ? t('Cancel') : t('Change…')}</button>
      {pick && <button type="button" className="button" disabled={busy} onClick={() => onSet(null)}>{t('Clear')}</button>}
    </div>
  </div>;
}
