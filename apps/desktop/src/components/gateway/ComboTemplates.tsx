import { useMemo, useState } from 'react';
import { LayoutTemplate } from 'lucide-react';
import { TIERS, call, errorText, gatewayChanged, type Combo, type Connection, type Model, type Tier } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd } from '../../lib/money';
import { REFERENCE, TEMPLATES, resolve, type Protocol, type Template } from '../../lib/comboTemplates';
import { ErrorNote, FormRow, Segmented, Sheet } from '../ui';

/**
 * A combo from a template (lib/comboTemplates.ts): pick one, see which of your models it picks and why, name it,
 * create it. Nothing is sent: a test is a separate, explicit step.
 */
export function ComboTemplates({models, connections, existing, onClose, onConnect}: {
  models: Model[]; connections: Connection[]; existing: Combo[]; onClose: () => void; onConnect: () => void;
}) {
  const [template, setTemplate] = useState<Template | null>(null);
  const [tools, setTools] = useState(true);
  const [minTier, setMinTier] = useState<Tier>('T2');
  const [protocol, setProtocol] = useState<Protocol>('anthropic');
  const [allowUntiered, setAllowUntiered] = useState(false);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const resolved = useMemo(() => template && resolve(template, {tools, minTier, protocol, allowUntiered}, models, connections),
    [template, tools, minTier, protocol, allowUntiered, models, connections]);
  const trimmed = name.trim();
  const nameError = !trimmed ? null : /[\s/]/.test(trimmed) ? t('No spaces or slashes.')
    : existing.some(c => c.name === trimmed) ? t('Another combo has this name.') : null;

  const pick = (tpl: Template) => {
    setTemplate(tpl);
    let n = tpl.defaultName, i = 2;
    while (existing.some(c => c.name === n)) n = `${tpl.defaultName}-${i++}`;
    setName(n);
  };
  const create = () => {
    if (!resolved) return;
    setBusy(true);
    call(async api => {
      const saved = await api.saveCombo(null, trimmed, resolved.members.map(m => m.target));
      if (template?.strategy) await api.setComboStrategy(saved.id, template.strategy);
    }).then(() => { gatewayChanged(); onClose(); },
      e => { setError(errorText(e)); setBusy(false); });
  };

  return <Sheet title={template ? t('{name} Template', {name: t(template.name)}) : t('New Combo from a Template')}
    subtitle={template ? t('This creates a saved list from your models as they are now. Models you connect later are not added by themselves.')
      : t('Each template picks members from your connected models by rules: billing, tier, tools and context.')}
    onClose={onClose} busy={busy} footer={<>
      {template && <button type="button" className="button" onClick={() => setTemplate(null)} disabled={busy}>{t('Other Templates')}</button>}
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      {template && <button type="button" className="button prominent" onClick={create}
        disabled={busy || !resolved?.members.length || resolved.paidOnly || !trimmed || !!nameError}>{t('Create Combo')}</button>}
    </>}>
    {!template && <div className="template-grid">
      {TEMPLATES.map(tpl => <button key={tpl.id} type="button" className="template-card" onClick={() => pick(tpl)}>
        <span className="template-icon" aria-hidden><LayoutTemplate size={16}/></span>
        <span className="template-text"><span className="template-name">{t(tpl.name)}</span>
          <small>{t(tpl.purpose)}</small>
          <small className="secondary">{tpl.stages.join(' → ')}{tpl.minContext ? ` · ≥${tpl.minContext / 1000}k` : ''}</small></span>
      </button>)}
    </div>}
    {template && resolved && <>
      <div className="form">
        <FormRow label={t('Clients')} hint={t('OpenAI-format clients cannot use Anthropic-format connections; Claude Code and Anthropic SDKs can use every connection.')}>
          <Segmented<Protocol> label={t('Clients')} value={protocol} onChange={setProtocol}
            options={[{value: 'anthropic', label: t('Claude Code / Anthropic')}, {value: 'openai', label: t('OpenAI format')}]}/>
        </FormRow>
        <FormRow label={t('Requirements')} hint={t('Unknown counts as missing: a model whose tools or tier are not known is left out.')}>
          <span className="inline-field">
            <label className="checkbox"><input type="checkbox" checked={tools} onChange={e => setTools(e.target.checked)}/>{t('Takes tools')}</label>
            <span>{t('Tier at least')}</span>
            <select className="popup-button" value={minTier} onChange={e => setMinTier(e.target.value as Tier)} aria-label={t('Tier at least')}>
              {TIERS.map(x => <option key={x} value={x}>{x}</option>)}
            </select>
            <label className="checkbox"><input type="checkbox" checked={allowUntiered} onChange={e => setAllowUntiered(e.target.checked)}/>
              {t('Include free and local models without a tier')}</label>
          </span>
        </FormRow>
      </div>
      {resolved.paidOnly && <div className="template-empty warn-box" role="alert">
        <p><b>{t('No free, local or plan model qualified, so this would be paid models only.')}</b> {t('Pygg won’t create a “{name}” combo that always costs money.', {name: t(template.name)})}</p>
        {resolved.stageReasons.filter(r => r.total > 0).map(r => <p key={r.stage} className="secondary">
          {t('{stage} models left out: {reasons}.', {stage: r.stage, reasons: r.reasons.map(([why, n]) => `${n} ${why}`).join(', ')})}</p>)}
        {resolved.stageReasons.every(r => r.total === 0) && <p className="secondary">{t('You have no free, local or plan models connected.')}</p>}
        <span className="row-actions">
          {minTier !== 'T0' && <button type="button" className="button small" onClick={() => setMinTier(minTier === 'T2' || minTier === 'T3' || minTier === 'T4' ? 'T1' : 'T0')}>
            {t('Allow tier {tier}', {tier: minTier === 'T1' ? 'T0' : 'T1'})}</button>}
          {!allowUntiered && resolved.stageReasons.some(r => r.reasons.some(([why]) => why === 'tier not set')) &&
            <button type="button" className="button small" onClick={() => setAllowUntiered(true)}>{t('Include models without a tier')}</button>}
          {tools && <button type="button" className="button small" onClick={() => setTools(false)}>{t('Don’t require tools')}</button>}
          <button type="button" className="button small" onClick={onConnect}>{t('Connect a Free Provider…')}</button>
        </span>
        <p className="secondary">{t('Many free models are listed without their tier or tool support: Providers → Check & Refresh Models, or pygg gateway lookup, fills in what is known. A lower tier lets weaker models answer; without tools, coding agents may fail.')}</p>
      </div>}
      {resolved.members.length > 0 ? <>
        <ol className="member-list" aria-label={t('Models in order')}>
          {resolved.members.map((m, i) => <li key={`${m.target.providerId}/${m.target.modelId}`} className="member">
            <span className="chain-index">{i + 1}</span>
            <span className="member-text"><span className="mono">{m.target.providerId}/{m.target.modelId}</span>
              <small>{m.why}{m.referenceMicroUsd !== null && m.referenceMicroUsd > 0 ? ` · ${t('≈{amount} per reference request', {amount: microUsd(m.referenceMicroUsd)})}` : ''}</small></span>
          </li>)}
        </ol>
        <p className="group-footnote">
          {resolved.members.length === 1 ? t('One member matched: this combo has no backup.') + ' ' : ''}
          {resolved.missingStages.length > 0 && t('Nothing matched for: {stages}.', {stages: resolved.missingStages.join(', ')}) + ' '}
          {resolved.capped > 0 && plural(resolved.capped, '{n} more model qualified; a combo keeps the first 6.', '{n} more models qualified; a combo keeps the first 6.') + ' '}
          {t('Prices are compared on a reference request of {input} tokens in and {output} out.', {input: REFERENCE.input.toLocaleString(), output: REFERENCE.output.toLocaleString()})}
        </p>
      </> : <div className="template-empty">
        <p>{t('None of your models matches this template.')}</p>
        <p className="secondary">{t('Connect a provider, check & refresh models so their facts are known, or change the requirements above. Pygg never lowers them or adds a paid model by itself.')}</p>
        <button type="button" className="button" onClick={onConnect}>{t('Connect Provider…')}</button>
      </div>}
      {resolved.excluded.length > 0 && <details className="template-excluded">
        <summary>{plural(resolved.excluded.length, '{n} model left out', '{n} models left out')}</summary>
        <ul>{resolved.excluded.slice(0, 60).map(x => <li key={`${x.target.providerId}/${x.target.modelId}`}>
          <span className="mono">{x.target.providerId}/{x.target.modelId}</span> <span className="secondary">· {x.why}</span></li>)}</ul>
      </details>}
      {resolved.members.length > 0 && <div className="form">
        <FormRow label={t('Name')} htmlFor="template-name" hint={nameError ?? t('Clients send this as the model.')}>
          <input id="template-name" className="text-input mono" value={name} onChange={e => setName(e.target.value)} spellCheck={false}
            aria-invalid={!!nameError}/>
        </FormRow>
      </div>}
    </>}
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}
