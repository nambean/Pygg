import { useEffect, useMemo, useState } from 'react';
import { ArrowDown, ArrowRight, ArrowUp, ListOrdered, Trash2, X } from 'lucide-react';
import { call, errorText, gatewayChanged, type Combo, type Connection, type Model, type Target } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { billingLabel } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { ConfirmSheet, CopyButton, Empty, ErrorNote, FormRow, Sheet, Spinner } from '../ui';

const key = (m: Target) => `${m.providerId}/${m.modelId}`;

/** Combos: named, ordered model lists. A request for the name tries each model in turn on hard errors. */
export function Combos({intent, onIntentDone}: {intent?: 'create'; onIntentDone: () => void}) {
  const combos = useGateway(api => api.combos());
  const connections = useGateway(api => api.connections());
  const models = useGateway(api => api.models());
  const [editing, setEditing] = useState<Combo | 'new' | null>(intent === 'create' ? 'new' : null);
  const [deleting, setDeleting] = useState<Combo | null>(null);
  useEffect(() => { if (intent === 'create') { setEditing('new'); onIntentDone(); } }, [intent, onIntentDone]);

  if (combos.loading && !combos.data) return <div className="page"><Spinner label={t('Loading combos…')}/></div>;
  const list = combos.data ?? [];
  return <div className="page">
    {combos.error && <ErrorNote>{combos.error}</ErrorNote>}
    {list.length === 0
      ? <Empty icon={<ListOrdered size={40} strokeWidth={1.4}/>} title={t('No Combos Yet')}
          action={<button type="button" className="button prominent" onClick={() => setEditing('new')}>{t('New Combo…')}</button>}>
          {t('A combo is a model name your clients can use, such as “coding”, that stands for a list of models. If the first is out of quota or unavailable, the next one answers.')}
        </Empty>
      : <section className="group">
        <h2>{t('Combos')}</h2>
        <ul className="group-box combo-list">
          {list.map(combo => <li key={combo.id} className="combo-row">
            <div className="combo-head">
              <span className="combo-name mono selectable">{combo.name}</span>
              <span className="secondary">{plural(combo.members.length, '{n} model', '{n} models')}</span>
              <span className="spacer"/>
              <CopyButton text={combo.name} label={t('Copy Name')}/>
              <button type="button" className="button" onClick={() => setEditing(combo)}>{t('Edit…')}</button>
              <button type="button" className="button icon-only" aria-label={t('Delete {name}', {name: combo.name})} title={t('Delete')}
                onClick={() => setDeleting(combo)}><Trash2 size={14} aria-hidden/></button>
            </div>
            <ol className="chain" aria-label={t('Order')}>
              {combo.members.map((m, i) => {
                const connection = connections.data?.find(c => c.id === m.providerId);
                const off = !connection?.enabled;
                return <li key={`${key(m)}-${i}`} className={off ? 'chain-step off' : 'chain-step'}>
                  {i > 0 && <ArrowRight size={12} className="chain-arrow" aria-hidden/>}
                  <span className="chain-chip mono" title={off ? t('Skipped: its connection is disabled or removed') : undefined}>
                    <span className="chain-index">{i + 1}</span>{key(m)}</span>
                </li>;
              })}
              {combo.members.length === 0 && <li className="secondary">{t('Empty: its models’ connections were removed.')}</li>}
            </ol>
          </li>)}
        </ul>
        <p className="group-footnote">{t('The next model is tried on a rejected key, an exhausted quota, an unavailable model, a timeout or a server error, and only before any of the answer has reached the client. A malformed request is returned as it is.')}</p>
      </section>}

    {editing && <ComboSheet combo={editing === 'new' ? null : editing} connections={connections.data ?? []} models={models.data ?? []}
      existing={list} onClose={() => setEditing(null)}/>}
    {deleting && <ConfirmSheet title={t('Delete {name}?', {name: deleting.name})} confirmLabel={t('Delete Combo')}
      message={<p>{t('Clients that ask for “{name}” will get “model not found” until a combo with that name exists again.', {name: deleting.name})}</p>}
      onClose={() => setDeleting(null)}
      onConfirm={async () => { await call(api => api.deleteCombo(deleting.id)); gatewayChanged(); }}/>}
  </div>;
}

function ComboSheet({combo, connections, models, existing, onClose}: {
  combo: Combo | null; connections: Connection[]; models: Model[]; existing: Combo[]; onClose: () => void;
}) {
  const [name, setName] = useState(combo?.name ?? (existing.length ? '' : 'coding'));
  const [members, setMembers] = useState<Target[]>(combo?.members ?? []);
  const [adding, setAdding] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const options = useMemo(() => connections.map(c => ({connection: c,
    models: models.filter(m => m.providerId === c.id && m.enabled)})).filter(g => g.models.length), [connections, models]);
  const nameError = !name.trim() ? null : /[\s/]/.test(name.trim()) ? t('No spaces or slashes.')
    : existing.some(c => c.name === name.trim() && c.id !== combo?.id) ? t('Another combo has this name.') : null;

  const move = (i: number, by: number) => setMembers(list => {
    const next = [...list];
    [next[i], next[i + by]] = [next[i + by], next[i]];
    return next;
  });
  const add = (value: string) => {
    const [providerId, ...rest] = value.split('/');
    if (providerId && rest.length) setMembers(list => [...list, {providerId, modelId: rest.join('/')}]);
    setAdding('');
  };
  const save = () => {
    setBusy(true);
    call(api => api.saveCombo(combo?.id ?? null, name.trim(), members)).then(() => { gatewayChanged(); onClose(); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  const describe = (m: Target) => {
    const model = models.find(x => x.providerId === m.providerId && x.modelId === m.modelId);
    const connection = connections.find(c => c.id === m.providerId);
    if (!connection) return t('Connection removed');
    if (!connection.enabled) return t('Connection disabled · skipped');
    const wire = connection.wire === 'anthropic' ? ` · ${t('Anthropic clients only')}` : '';
    return (model ? billingLabel(model.billing) + (model.price ? ` · ${t('priced')}` : '') : t('Not in the model list')) + wire;
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
    </div>
    <ol className="member-list" aria-label={t('Models in order')}>
      {members.map((m, i) => <li key={`${key(m)}-${i}`} className="member">
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
