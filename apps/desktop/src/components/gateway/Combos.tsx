import { useEffect, useMemo, useState } from 'react';
import { ArrowDown, ArrowRight, ArrowUp, GripVertical, LayoutTemplate, ListOrdered, Trash2, X } from 'lucide-react';
import { call, errorText, gatewayChanged, type Combo, type Connection, type Model, type Target } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { billingLabel } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { ConfirmSheet, CopyButton, Empty, ErrorNote, FormRow, Segmented, Sheet, Spinner } from '../ui';
import { AutoRouting } from './AutoRouting';
import { ComboTemplates } from './ComboTemplates';

const key = (m: Target) => `${m.providerId}/${m.modelId}`;

/**
 * Combos: named, ordered model lists, as in 9router and OmniRoute. A request for the name tries each model in turn on
 * hard errors. Templates build one from rules over the user's models; the second tab explains and simulates `auto`.
 */
export function Combos({intent, onIntentDone, onConnect}: {intent?: 'create' | 'auto'; onIntentDone: () => void; onConnect: () => void}) {
  // A link named "How auto chooses" opens that tab; the sidebar opens the combos (Codex review 7).
  const [tab, setTab] = useState<'combos' | 'auto'>(intent === 'auto' ? 'auto' : 'combos');
  const [templating, setTemplating] = useState(false);
  const combos = useGateway(api => api.combos());
  const connections = useGateway(api => api.connections());
  const models = useGateway(api => api.models());
  const [editing, setEditing] = useState<Combo | 'new' | null>(intent === 'create' ? 'new' : null);
  const [deleting, setDeleting] = useState<Combo | null>(null);
  useEffect(() => {
    if (intent === 'create') { setEditing('new'); onIntentDone(); }
    if (intent === 'auto') { setTab('auto'); onIntentDone(); }
  }, [intent, onIntentDone]);

  if (combos.loading && !combos.data) return <div className="page"><Spinner label={t('Loading combos…')}/></div>;
  const list = combos.data ?? [];
  const modelOf = (m: Target) => models.data?.find(x => x.providerId === m.providerId && x.modelId === m.modelId);
  return <div className="page">
    <div className="page-bar">
      <Segmented label={t('Show')} value={tab} onChange={setTab}
        options={[{value: 'combos', label: t('Combos')}, {value: 'auto', label: t('How auto chooses')}]}/>
      <span className="spacer"/>
      {tab === 'combos' && <button type="button" className="button" onClick={() => setTemplating(true)}>
        <LayoutTemplate size={13} aria-hidden/>{t('From Template…')}</button>}
    </div>
    {tab === 'auto' && <AutoRouting/>}
    {tab === 'combos' && <>
    <p className="page-intro">{t('Using auto? You don’t need a combo. Make one when you want to choose the models and their fallback order yourself; the next model is tried only before an answer starts.')}</p>
    {combos.error && <ErrorNote>{combos.error}</ErrorNote>}
    {list.length === 0
      ? <Empty icon={<ListOrdered size={40} strokeWidth={1.4}/>} title={t('No Combos Yet')}
          action={<span className="row-actions">
            <button type="button" className="button prominent" onClick={() => setTemplating(true)}>{t('Start from a Template…')}</button>
            <button type="button" className="button" onClick={() => setEditing('new')}>{t('New Combo…')}</button></span>}>
          {t('One model name, a fallback order you control. A combo such as “coding” stands for a list of models: if the first is out of quota or unavailable, the next one answers. Templates such as Free first or Use my plans first build one from your models.')}
        </Empty>
      : <section className="group">
        <h2>{t('Combos')}</h2>
        <p className="group-intro">{t('One model name, a fallback order you control. Clients send the name as the model.')}</p>
        <ul className="group-box combo-list">
          {list.map(combo => <li key={combo.id} className="combo-row">
            <div className="combo-head">
              <span className="combo-name mono selectable">{combo.name}</span>
              <span className="secondary">{combo.strategy === 'cheapest' ? t('Cheapest first') : t('Ordered fallback')} · {plural(combo.members.length, '{n} model', '{n} models')}
                {(() => {
                  const off = combo.members.filter(m => !connections.data?.find(c => c.id === m.providerId)?.enabled).length;
                  const missing = combo.members.filter(m => !modelOf(m)).length;
                  return off || missing ? <span className="warn-text"> · {[off && plural(off, '{n} off', '{n} off'),
                    missing && plural(missing, '{n} missing', '{n} missing')].filter(Boolean).join(' · ')}</span> : null;
                })()}</span>
              <span className="spacer"/>
              <CopyButton text={combo.name} label={t('Copy Name')}/>
              <ComboTestButton name={combo.name}/>
              <button type="button" className="button" onClick={() => setEditing(combo)}>{t('Edit…')}</button>
              <button type="button" className="button icon-only" aria-label={t('Delete {name}', {name: combo.name})} title={t('Delete')}
                onClick={() => setDeleting(combo)}><Trash2 size={14} aria-hidden/></button>
            </div>
            {combo.strategy === 'cheapest' && <p className="chain-note">{t('Ordered for each request by its estimated cost: free, local and plan members first, unknown prices last.')}</p>}
            <ol className="chain" aria-label={t('Order')}>
              {combo.members.map((m, i) => {
                const connection = connections.data?.find(c => c.id === m.providerId);
                const off = !connection?.enabled;
                const pool = combo.strategy === 'cheapest';
                return <li key={`${key(m)}-${i}`} className={off ? 'chain-step off' : 'chain-step'}>
                  <span className="chain-chip mono" title={off ? t('Skipped: its connection is disabled or removed') : undefined}>
                    {!pool && <span className="chain-index">{i + 1}</span>}{key(m)}
                    {modelOf(m) && <span className="chain-meta">{[billingLabel(modelOf(m)!.billing), modelOf(m)!.capabilities.tier,
                      modelOf(m)!.capabilities.supportsTools ? t('tools') : null].filter(Boolean).join(' · ')}</span>}</span>
                  {!pool && i < combo.members.length - 1 && <ArrowRight size={12} className="chain-arrow" aria-hidden/>}
                </li>;
              })}
              {combo.members.length === 0 && <li className="secondary">{t('Empty: its models’ connections were removed.')}</li>}
            </ol>
          </li>)}
        </ul>
        <details className="group-footnote fallback-details">
          <summary>{t('Tries members in order when a key is rejected, quota is exhausted, or a model is unavailable. Once an answer starts streaming, Pygg does not switch models.')}</summary>
          {t('After a timeout or a server error the provider may already have run (and billed) the request, so the gateway stops there unless the API key allows paid retries (Endpoint → API Keys → Limits). A malformed request is returned as it is.')}
        </details>
      </section>}
    </>}
    {templating && <ComboTemplates models={models.data ?? []} connections={connections.data ?? []} existing={list}
      onClose={() => setTemplating(false)} onConnect={() => { setTemplating(false); onConnect(); }}/>}

    {editing && <ComboSheet combo={editing === 'new' ? null : editing} connections={connections.data ?? []} models={models.data ?? []}
      existing={list} onClose={() => setEditing(null)}/>}
    {deleting && <ConfirmSheet title={t('Delete {name}?', {name: deleting.name})} confirmLabel={t('Delete Combo')}
      message={<p>{t('Clients that ask for “{name}” will get “model not found” until a combo with that name exists again.', {name: deleting.name})}</p>}
      onClose={() => setDeleting(null)}
      onConfirm={async () => { await call(api => api.deleteCombo(deleting.id)); gatewayChanged(); }}/>}
  </div>;
}

/**
 * Sends one short request to the combo by its name, through its real route: says which member answered and whether
 * members before it failed. It asks first, as a paid member may answer; nothing is sent when the editor opens.
 */
function ComboTestButton({name}: {name: string}) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [ok, setOk] = useState(true);
  const run = () => {
    if (!window.confirm(t('Send one short request through {name}? A paid model may answer, and fallback may try more than one model. Charges are recorded in Usage.', {name}))) return;
    setBusy(true);
    setResult(null);
    call(api => api.testCombo(name)).then(r => {
      setOk(r.ok);
      setResult(r.ok ? (r.attempts && r.attempts > 1
        ? t('Answered by {model} in {ms} ms, after {n} members failed', {model: r.answeredBy ?? '', ms: String(r.latencyMs), n: String(r.attempts - 1)})
        : t('Answered by {model} in {ms} ms', {model: r.answeredBy ?? '', ms: String(r.latencyMs)})) : r.text);
    }, e => { setOk(false); setResult(errorText(e)); }).finally(() => setBusy(false));
  };
  return <>
    <button type="button" className="button" disabled={busy} onClick={run}>{busy ? t('Testing…') : t('Test')}</button>
    {result && <small className={ok ? 'source combo-test' : 'warn-text combo-test'} role="status">{result}</small>}
  </>;
}

/** Connections in a combo that share a quota group (review GW-1), one list of labels per group. */
function sharedGroups(members: Target[], connections: Connection[]): string[][] {
  const groups = new Map<string, Set<string>>();
  for (const m of members) {
    const c = connections.find(x => x.id === m.providerId);
    if (!c || c.separateQuota) continue;
    groups.set(c.quotaGroup, (groups.get(c.quotaGroup) ?? new Set()).add(c.label));
  }
  return [...groups.values()].filter(s => s.size > 1).map(s => [...s]);
}

function ComboSheet({combo, connections, models, existing, onClose}: {
  combo: Combo | null; connections: Connection[]; models: Model[]; existing: Combo[]; onClose: () => void;
}) {
  const [name, setName] = useState(combo?.name ?? (existing.length ? '' : 'coding'));
  const [members, setMembers] = useState<Target[]>(combo?.members ?? []);
  const [strategy, setStrategy] = useState<'order' | 'cheapest'>(combo?.strategy ?? 'order');
  const [adding, setAdding] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const options = useMemo(() => connections.filter(c => c.enabled).map(c => ({connection: c,
    models: models.filter(m => m.providerId === c.id && m.enabled)})).filter(g => g.models.length), [connections, models]);
  const nameError = !name.trim() ? null : /[\s/]/.test(name.trim()) ? t('No spaces or slashes.')
    : existing.some(c => c.name === name.trim() && c.id !== combo?.id) ? t('Another combo has this name.') : null;

  const move = (i: number, by: number) => setMembers(list => {
    const next = [...list];
    [next[i], next[i + by]] = [next[i + by], next[i]];
    return next;
  });
  // Drag a member to a new place; Move up and Move down do the same from the keyboard.
  const [dragged, setDragged] = useState<number | null>(null);
  const [over, setOver] = useState<number | null>(null);
  const drop = (to: number) => {
    if (dragged === null || dragged === to) { setDragged(null); setOver(null); return; }
    setMembers(list => {
      const next = [...list];
      const [item] = next.splice(dragged, 1);
      next.splice(to, 0, item);
      return next;
    });
    setDragged(null);
    setOver(null);
  };
  const add = (value: string) => {
    const [providerId, ...rest] = value.split('/');
    if (providerId && rest.length) setMembers(list => [...list, {providerId, modelId: rest.join('/')}]);
    setAdding('');
  };
  const save = () => {
    setBusy(true);
    call(async api => {
      const saved = await api.saveCombo(combo?.id ?? null, name.trim(), members);
      if ((saved.strategy ?? 'order') !== strategy) await api.setComboStrategy(saved.id, strategy);
    }).then(() => { gatewayChanged(); onClose(); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  const describe = (m: Target) => {
    const model = models.find(x => x.providerId === m.providerId && x.modelId === m.modelId);
    const connection = connections.find(c => c.id === m.providerId);
    if (!connection) return t('Connection removed');
    if (!connection.enabled) return t('Connection disabled · skipped');
    const wire = connection.wire === 'anthropic' ? ` · ${t('Anthropic clients only')}` : '';
    if (!model) return t('Not in the model list') + wire;
    const caps = model.capabilities;
    return [billingLabel(model.billing), model.price ? t('priced') : null, caps.tier,
      caps.supportsTools === true ? t('tools') : caps.supportsTools === false ? t('no tools') : null,
      caps.contextTokens ? `${Math.round(caps.contextTokens / 1000)}k` : null].filter(Boolean).join(' · ') + wire;
  };

  return <Sheet title={combo ? t('Edit {name}', {name: combo.name}) : t('New Combo')}
    subtitle={t('Requests for this name try the models from the top down.')} onClose={onClose} busy={busy}
    footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={save} disabled={busy || !name.trim() || !!nameError || !members.length}>
        {combo ? t('Save Combo') : t('Create Combo')}</button>
    </>}>
    <div className="form">
      <FormRow label={t('Name')} htmlFor="combo-name" hint={nameError ?? t('What clients put in the model field, e.g. coding or cheap.')}>
        <input id="combo-name" className="text-input mono" value={name} onChange={e => setName(e.target.value)} spellCheck={false}
          autoFocus aria-invalid={!!nameError}/>
      </FormRow>
      <FormRow label={t('Order')} hint={strategy === 'cheapest'
        ? t('For each request, the members by its estimated cost: free, local and plan members first, unknown prices last. The next is tried on the same errors as always.')
        : t('The members in the order below.')}>
        <Segmented<'order' | 'cheapest'> label={t('Order')} value={strategy} onChange={setStrategy}
          options={[{value: 'order', label: t('As listed')}, {value: 'cheapest', label: t('Cheapest first')}]}/>
      </FormRow>
    </div>
    <ol className="member-list" aria-label={t('Models in order')}>
      {members.map((m, i) => <li key={`${key(m)}-${i}`} className={`member${dragged === i ? ' dragging' : ''}${over === i && dragged !== i ? ' drop-target' : ''}`}
        draggable onDragStart={e => { setDragged(i); e.dataTransfer.effectAllowed = 'move'; }}
        onDragOver={e => { e.preventDefault(); setOver(i); }} onDragLeave={() => setOver(o => o === i ? null : o)}
        onDrop={e => { e.preventDefault(); drop(i); }} onDragEnd={() => { setDragged(null); setOver(null); }}>
        <span className="drag-handle" aria-hidden title={t('Drag to reorder')}><GripVertical size={14}/></span>
        <span className="chain-index">{i + 1}</span>
        <span className="member-text"><span className="mono">{key(m)}</span><small>{describe(m)}</small></span>
        <button type="button" className="button small icon-only" onClick={() => move(i, -1)} disabled={i === 0}
          aria-label={t('Move up')} title={t('Move up')}><ArrowUp size={13} aria-hidden/></button>
        <button type="button" className="button small icon-only" onClick={() => move(i, 1)} disabled={i === members.length - 1}
          aria-label={t('Move down')} title={t('Move down')}><ArrowDown size={13} aria-hidden/></button>
        <button type="button" className="button small icon-only" onClick={() => setMembers(list => list.filter((_, j) => j !== i))}
          aria-label={t('Remove')} title={t('Remove')}><X size={13} aria-hidden/></button>
      </li>)}
      {members.length === 0 && <li className="member empty secondary">{t('Add at least one model.')}</li>}
    </ol>
    {sharedGroups(members, connections).map(labels => <p key={labels.join()} className="group-footnote warn-text" role="note">
      {t('{connections} share one quota: when one is refused for its quota or rate limit, the gateway does not try the others for that model, since they would be refused too. If they are separate accounts, confirm that in Providers.',
        {connections: labels.join(', ')})}</p>)}
    <div className="form">
      <FormRow label={t('Add model')} htmlFor="combo-add">
        <select id="combo-add" className="popup-button wide" value={adding} onChange={e => add(e.target.value)} disabled={!options.length}>
          <option value="">{options.length ? t('Choose a model…') : t('Connect a provider first')}</option>
          {options.map(g => <optgroup key={g.connection.id} label={g.connection.label}>
            {g.models.map(m => <option key={m.modelId} value={`${m.providerId}/${m.modelId}`}>{m.modelId}</option>)}
          </optgroup>)}
        </select>
      </FormRow>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}
