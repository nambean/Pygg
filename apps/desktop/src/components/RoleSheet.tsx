import { useMemo, useState } from 'react';
import { call, errorText, type AutoPlan } from '../lib/api';
import { usd } from '../lib/format';
import { t } from '../lib/i18n';
import { evidenceLine, factLine, type BrainRole, type FactsStatus, type RoleLine } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { Badge, ErrorNote, Sheet, Spinner } from './ui';
import './brain.css';

/**
 * The role sheet (BRAIN.md §4.5): every connected model for a role, one evidence line each (the layer that won:
 * your runs, a fact with its source and date, or the tier), **Why this model?** with the whole chain and the other
 * sources, and **Use** on a row. With a brainstorm, the rows say which models its client's policy rules out and why,
 * and the estimate for one run; without one (the Brain Storming screen), every enabled model is listed.
 */
export function RoleSheet({role, brainstormId, current, exclude = [], savedPick, onPick, onClose}: {
  role: BrainRole; brainstormId?: string | null; current?: string | null; exclude?: string[]; savedPick?: string | null;
  onPick: (model: string) => void | Promise<unknown>; onClose: () => void;
}) {
  const lines = useGateway(api => api.modelFactsRole(role), [role]);
  const facts = useGateway(api => api.modelFactsStatus());
  const plan = useGateway(api => brainstormId ? api.brainstormCandidates(brainstormId, role) : Promise.resolve(null as AutoPlan | null), [brainstormId, role]);
  const [query, setQuery] = useState('');
  const [scope, setScope] = useState<'eligible' | 'all'>(brainstormId ? 'eligible' : 'all');
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const eligible = useMemo(() => new Map((plan.data?.ranked ?? []).map(r => [`${r.providerId}/${r.modelId}`, r])), [plan.data]);
  const excluded = useMemo(() => new Map((plan.data?.excluded ?? []).map(e => [`${e.providerId}/${e.modelId}`, e])), [plan.data]);
  const q = query.trim().toLowerCase();
  const rows = (lines.data ?? []).filter(l => !exclude.includes(l.model) && (!q || l.model.toLowerCase().includes(q)))
    .filter(l => scope === 'all' || !brainstormId || eligible.has(l.model));
  const use = (model: string) => { setBusy(true); setError(null); Promise.resolve(onPick(model)).then(() => { setBusy(false); onClose(); }, e => { setError(errorText(e)); setBusy(false); }); };
  const f = facts.data;
  const subtitle = f ? (f.enabled ? (f.built ? t('facts as of {date}{stale}', {date: f.built.slice(0, 10), stale: f.stale ? ` · ${t('could not refresh')}` : ''}) : t('no facts yet; ranked by tier')) : t('ranked by tier; facts off')) : '';
  return <Sheet title={t('Models for {role}', {role})} subtitle={subtitle} onClose={onClose} busy={busy} className="role-sheet-dialog"
    footer={<>
      {f && !f.enabled && <button type="button" className="button" disabled={busy} onClick={() => { setBusy(true); call(api => api.modelFactsSetEnabled(true)).then(() => { setBusy(false); facts.reload(); lines.reload(); }, e => { setError(errorText(e)); setBusy(false); }); }}>{t('Turn facts on')}</button>}
      {f?.enabled && <button type="button" className="button" disabled={busy} onClick={() => { setBusy(true); call(api => api.modelFactsRefresh()).then(() => { setBusy(false); facts.reload(); lines.reload(); }, e => { setError(errorText(e)); setBusy(false); }); }}>{t('Refresh facts')}</button>}
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
    </>}>
    <div className="role-sheet">
      <div className="role-sheet-bar">
        <input className="text-input" type="search" value={query} onChange={e => setQuery(e.target.value)} placeholder={t('Find a model')} aria-label={t('Find a model')}/>
        {brainstormId && <div className="role-scope" role="group" aria-label={t('Which models')}>
          <button type="button" aria-pressed={scope === 'eligible'} className={scope === 'eligible' ? 'button prominent' : 'button'} onClick={() => setScope('eligible')}>{t('Eligible under my policy')}</button>
          <button type="button" aria-pressed={scope === 'all'} className={scope === 'all' ? 'button prominent' : 'button'} onClick={() => setScope('all')}>{t('All connections')}</button>
        </div>}
      </div>
      {(lines.error || plan.error) && <ErrorNote>{lines.error ?? plan.error}</ErrorNote>}
      {error && <ErrorNote>{error}</ErrorNote>}
      {!lines.data || (brainstormId && !plan.data && !plan.error) ? <Spinner/> : rows.length === 0
        ? <p className="group-footnote">{q ? t('No model matches “{q}”.', {q: query}) : scope === 'eligible'
            ? t('Nothing eligible for {role} under this client’s policy: connect a provider or loosen the policy. All connections shows why each is out.', {role})
            : t('No connected model is enabled.')}</p>
        : <ul className="role-rows">
          {rows.map(l => {
            const r = eligible.get(l.model);
            const x = brainstormId && !r ? excluded.get(l.model) : null;
            const out = !!brainstormId && !r;
            return <li key={l.model} className={`role-row${out ? ' out' : ''}${l.model === current ? ' current' : ''}`}>
              <div className="role-main">
                <div className="brain-head"><span className="brain-id">{l.model}</span>
                  <span className="brain-meta">{l.tier ?? t('no tier')} · {l.billing}{r ? ` · ≈${usd(r.estimateMicroUsd / 1e6)} ${t('per run')}` : l.turnMicroUsd != null ? ` · ${l.turnMicroUsd === 0 ? t('nothing a request') : `≈${usd(l.turnMicroUsd / 1e6)} ${t('a typical turn')}`}` : ` · ${t('price unknown')}`}</span>
                  {l.model === current && <Badge tone="info">{t('current')}</Badge>}
                  {savedPick === l.model && <Badge tone="ok">{t('your saved pick')}</Badge>}</div>
                <div className="brain-evidence">{evidenceLine(l)}{l.layer === 'feed' && l.fact?.url && <> · <a href={l.fact.url} target="_blank" rel="noreferrer">{l.fact.sourceName}</a></>}</div>
                {x && <div className="warn-text" style={{fontSize: 11}}>{t('Not eligible: {reason}', {reason: x.reason})}</div>}
                {out && !x && <div className="warn-text" style={{fontSize: 11}}>{t('Not in this client’s route')}</div>}
                <button type="button" className="link-button" aria-expanded={why === l.model} aria-label={t('Why {model}?', {model: l.model})} onClick={() => setWhy(why === l.model ? null : l.model)}>{t('Why this model?')}</button>
                {why === l.model && <Why line={l} facts={f ?? null} saved={savedPick === l.model}/>}
              </div>
              <div className="role-actions">
                <button type="button" className="button prominent" disabled={busy || out || l.model === current} aria-label={t('Use {model}', {model: l.model})} onClick={() => use(l.model)}>{t('Use')}</button>
              </div>
            </li>;
          })}
        </ul>}
      {brainstormId && scope === 'eligible' && excluded.size > 0 && rows.length > 0 && <p className="group-footnote">{t('{n} connected models are not eligible under this client’s policy; All connections lists them with the reason.', {n: excluded.size})}</p>}
    </div>
  </Sheet>;
}

/** The chain of BRAIN.md §1 with the layer that won marked, then every source line the feed has for this model. */
function Why({line: l, facts, saved}: {line: RoleLine; facts: FactsStatus | null; saved: boolean}) {
  const layers: {key: string; label: string; text: string; wins: boolean}[] = [
    {key: 'saved', label: t('your saved pick'), text: saved ? t('saved on Brain Storming; wins over everything below') : t('none for this role'), wins: saved},
    {key: 'own', label: t('your runs (10+ attempts)'), text: l.own ? t('{kept}/{n} kept · {passed}/{n} checks passed{ranks}', {kept: l.own.kept, n: l.own.attempts, passed: l.own.checksPassed, ranks: l.own.ranks ? '' : ` · ${t('under ten attempts, not ranking yet')}`}) : t('none yet'), wins: !saved && l.layer === 'own'},
    {key: 'feed', label: t('facts feed'), text: l.fact ? factLine(l.fact) : facts && !facts.enabled ? t('facts off') : t('no fact for this role'), wins: !saved && l.layer === 'feed'},
    {key: 'tier', label: t('tier'), text: l.tier ? t('{tier} from {source}', {tier: l.tier, source: l.tierSource ?? t('Pygg’s table')}) : t('none'), wins: !saved && l.layer === 'tier'},
  ];
  const sources = [...(l.fact ? [l.fact] : []), ...l.others];
  return <div className="role-why">
    <ol className="role-chain">
      {layers.map(x => <li key={x.key} className={x.wins ? 'wins' : ''}><span className="role-layer">{x.label}</span> <span>{x.text}</span>{x.wins && <Badge tone="ok">{t('wins')}</Badge>}</li>)}
    </ol>
    {sources.length > 0 && <ul className="role-sources">
      {sources.map((s, i) => <li key={`${s.source}-${i}`}>
        <span>{factLine(s)}{s.agent ? ` · ${t('run with {agent}', {agent: s.agent})}` : ''}{s.effort ? ` · ${s.effort}` : ''}{s.votes != null ? ` · ${t('{n} votes', {n: s.votes})}` : ''}{!s.ranks ? ` · ${t('older than a year: shown, not ranking')}` : ''}</span>
        <span className="brain-meta"> — {s.measures}{s.url && <> · <a href={s.url} target="_blank" rel="noreferrer">{s.sourceName}</a></>} · {s.licence}</span>
      </li>)}
    </ul>}
    {sources.length === 0 && <p className="brain-meta">{t('No public fact names this model for this role.')}</p>}
  </div>;
}
