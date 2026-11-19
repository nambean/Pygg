import { useState } from 'react';
import { call, errorText } from '../lib/api';
import { t } from '../lib/i18n';
import type { BoardLimits, BrainPick, BrainRole, BrainSettings } from '../lib/tasks';
import { useGateway } from '../hooks/useGateway';
import { ErrorNote, Sheet, Spinner } from './ui';
import { RoleSheet } from './RoleSheet';
import './brain.css';

const ROLES: BrainRole[] = ['plan', 'code', 'test', 'review'];

/**
 * A project's overrides (BRAIN.md §4.7b, 4.7c): the picks by role and the WIP limits that differ from the Brain
 * Storming defaults, and nothing else. A pick is saved as it is chosen; the limits are saved with their button.
 */
export function BrainOverride({workspace, picks, settings, onClose, onChanged}: {
  workspace: {id: string; name: string}; picks: BrainPick[]; settings: BrainSettings | null; onClose: () => void; onChanged: () => void;
}) {
  const limits = useGateway(api => api.boardLimits(workspace.id), [workspace.id]);
  const [sheet, setSheet] = useState<BrainRole | null>(null);
  const [own, setOwn] = useState<boolean | null>(null);
  const [running, setRunning] = useState<number | null>(null);
  const [checking, setChecking] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const act = (f: () => Promise<unknown>, done?: () => void) => { setBusy(true); setError(null); f().then(() => { setBusy(false); onChanged(); done?.(); }, e => { setError(errorText(e)); setBusy(false); }); };
  const l: BoardLimits | null = limits.data;
  const ownNow = own ?? l?.own ?? false;
  const runningNow = running ?? l?.wipRunning ?? settings?.wipRunning ?? 1;
  const checkingNow = checking ?? l?.wipChecking ?? settings?.wipChecking ?? 1;
  const pickOf = (scope: string, role: BrainRole) => picks.find(p => p.scope === scope && p.role === role)?.model ?? null;
  const saveLimits = () => act(() => call(api => api.boardSetLimits(workspace.id, ownNow ? Math.max(1, runningNow) : null, ownNow ? Math.max(1, checkingNow) : null)), limits.reload);
  const changed = l ? ownNow !== l.own || (ownNow && (runningNow !== l.wipRunning || checkingNow !== l.wipChecking)) : false;
  return <Sheet title={t('{name}: what differs', {name: workspace.name})} subtitle={t('Only what is set here differs from Brain Storming; everything else follows it.')} onClose={onClose} busy={busy}
    footer={<><span className="spacer"/><button type="button" className="button" onClick={onClose} disabled={busy}>{t('Done')}</button></>}>
    {error && <ErrorNote>{error}</ErrorNote>}
    <div className="group-box" style={{marginTop: 6}}>
      {ROLES.map(role => {
        const mine = pickOf(workspace.id, role);
        const def = pickOf('global', role);
        return <div key={role} className="group-row" style={{alignItems: 'flex-start'}}>
          <div className="group-label" style={{flex: '0 0 110px'}}><span className={`role-chip ${role}`}>{role}</span></div>
          <div style={{flex: 1, minWidth: 0, fontSize: 12}}>
            <div><span className="brain-meta">{t('Default')}: </span><span className="brain-id">{def ?? t('by the rules')}</span></div>
            <div><span className="brain-meta">{t('This project')}: </span>{mine ? <span className="brain-id">{mine}</span> : <span className="secondary">{t('the default')}</span>}</div>
          </div>
          <div className="row-buttons" style={{display: 'flex', gap: 6, flexShrink: 0}}>
            <button type="button" className="button" disabled={busy} aria-label={t('Override the {role} model for {name}', {role, name: workspace.name})} onClick={() => setSheet(role)}>{mine ? t('Change…') : t('Override…')}</button>
            {mine && <button type="button" className="button" disabled={busy} aria-label={t('Use the default {role} model for {name}', {role, name: workspace.name})} onClick={() => act(() => call(api => api.setBrainPick(workspace.id, role, null)))}>{t('Use default')}</button>}
          </div>
        </div>;
      })}
      {!l ? <div className="group-row"><Spinner/></div> : <div className="group-row" style={{alignItems: 'flex-start'}}>
        <div className="group-label" style={{flex: '0 0 110px'}}><label>{t('WIP limits')}</label><small>{t('Default: Running {r} · Checking {c}', {r: settings?.wipRunning ?? 1, c: settings?.wipChecking ?? 1})}</small></div>
        <div style={{flex: 1, minWidth: 0, fontSize: 12, display: 'flex', flexWrap: 'wrap', gap: '6px 14px', alignItems: 'center'}}>
          <label className="inline-field"><input type="checkbox" checked={ownNow} disabled={busy} onChange={e => setOwn(e.target.checked)}/> {t('Own limits for this project')}</label>
          <label className="inline-field">{t('Running')} <input className="text-input" type="number" min={1} style={{width: 52, textAlign: 'center'}} value={runningNow} disabled={busy || !ownNow} aria-label={t('Running limit for {name}', {name: workspace.name})} onChange={e => setRunning(Math.max(1, Number(e.target.value) || 1))}/></label>
          <label className="inline-field">{t('Checking')} <input className="text-input" type="number" min={1} style={{width: 52, textAlign: 'center'}} value={checkingNow} disabled={busy || !ownNow} aria-label={t('Checking limit for {name}', {name: workspace.name})} onChange={e => setChecking(Math.max(1, Number(e.target.value) || 1))}/></label>
        </div>
        <div className="row-buttons" style={{flexShrink: 0}}>
          <button type="button" className="button prominent" disabled={busy || !changed} onClick={saveLimits}>{t('Save limits')}</button>
        </div>
      </div>}
    </div>
    {sheet && <RoleSheet role={sheet} current={pickOf(workspace.id, sheet)} savedPick={pickOf(workspace.id, sheet) ?? pickOf('global', sheet)} onClose={() => setSheet(null)}
      onPick={model => act(() => call(api => api.setBrainPick(workspace.id, sheet, model)))}/>}
  </Sheet>;
}
