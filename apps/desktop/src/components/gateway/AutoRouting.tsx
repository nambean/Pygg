import { useState } from 'react';
import { ChevronDown, ChevronRight, Wand2 } from 'lucide-react';
import { AUTO_ALIASES, type AutoAlias } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { billingLabel, microUsd } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { CopyButton, ErrorNote, Segmented, Spinner } from '../ui';

/**
 * Automatic routing (ROADMAP P0-5): what `auto` and `auto/cheap` mean, and a simulator that shows which
 * connected models a request would go to, in what order and why, and every model left out with its reason.
 * The simulator sends nothing.
 */
export function AutoRouting() {
  const clients = useGateway(api => api.clients());
  const [alias, setAlias] = useState<AutoAlias>('auto');
  const [clientId, setClientId] = useState<string>('');
  const [anthropic, setAnthropic] = useState(false);
  const [tools, setTools] = useState(false);
  const [vision, setVision] = useState(false);
  const [prompt, setPrompt] = useState('2000');
  const [maxTokens, setMaxTokens] = useState('');
  const [thinking, setThinking] = useState<'adaptive' | 'enabled' | 'disabled' | ''>('');
  const [effort, setEffort] = useState('');
  const [showExcluded, setShowExcluded] = useState(false);
  const inputTokens = /^\d+$/.test(prompt.trim()) ? Number(prompt) : 0;
  const limit = /^\d+$/.test(maxTokens.trim()) ? Number(maxTokens) : null;
  const plan = useGateway(api => api.autoSimulate(clientId || null, alias,
    {surfaceAnthropic: anthropic, tools, vision, inputTokens, maxTokens: limit,
      thinking: anthropic && thinking ? thinking : null, effort: anthropic && effort ? effort : null}),
    [alias, clientId, anthropic, tools, vision, inputTokens, limit, thinking, effort]);
  const active = (clients.data ?? []).filter(c => !c.revokedAt);
  const route = plan.data?.route ?? [];
  const more = (plan.data?.ranked.length ?? 0) - route.length;

  return <section className="group" aria-label={t('Automatic Routing')}>
    <div className="group-heading">
      <h2>{t('Automatic Routing')}</h2>
      <span className="heading-actions"><CopyButton text={alias} label={t('Copy Name')}/></span>
    </div>
    <div className="group-box">
      <div className="group-row stacked">
        <p className="auto-intro"><Wand2 size={15} aria-hidden/>{t('Clients can ask for “auto” or “auto/cheap” instead of a model. The gateway then picks from your connected models when the request arrives, by fixed rules, and says why in the response.')}</p>
        <ul className="auto-rules">
          <li><span className="mono">auto</span>{t(' — the lowest tier at or above the client’s minimum (T2 unless you change it in Endpoint → API Keys → Limits…), then local, free, subscription and paid in that order, then the lower estimated cost. This is the lowest eligible tier under your policy, not a promise of the cheapest answer.')}</li>
          <li><span className="mono">auto/cheap</span>{t(' — the lowest estimated cost of this request among the models that can take it. No tier needed.')}</li>
          <li><span className="mono">auto/plan, auto/code, auto/test, auto/review</span>{t(' — the best fact for that role from the model facts feed first (a public benchmark, named with its date in the response), then the models with no fact by tier. A fact older than a year does not rank. Facts off, or none kept: by tier alone.')}</li>
          <li>{t('An Anthropic request that asks for thinking or an effort level goes only to Anthropic-API models known to take it; their model list says so (Providers → Check & Refresh Models). Claude Code asks for adaptive thinking.')}</li>
          <li>{t('A model takes part only for what it is known to support: set tiers, context sizes and the rest under Providers → Auto… (OpenRouter’s list fills most of it in). Unknown counts as missing. A session stays on the model that first answered it.')}</li>
        </ul>
      </div>
      <div className="group-row stacked auto-controls">
        <Segmented<AutoAlias> label={t('Alias')} value={alias} onChange={setAlias}
          options={AUTO_ALIASES.map(a => ({value: a, label: a}))}/>
        <select className="popup-button" value={clientId} onChange={e => setClientId(e.target.value)} aria-label={t('Client')}>
          <option value="">{t('A new client (defaults)')}</option>
          {active.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
        </select>
        <Segmented<'openai' | 'anthropic'> label={t('Request format')} value={anthropic ? 'anthropic' : 'openai'}
          onChange={v => setAnthropic(v === 'anthropic')}
          options={[{value: 'openai', label: t('OpenAI format')}, {value: 'anthropic', label: t('Anthropic (Claude Code)')}]}/>
        <label className="check-row"><input type="checkbox" checked={tools} onChange={e => setTools(e.target.checked)}/>{t('With tools')}</label>
        <label className="check-row"><input type="checkbox" checked={vision} onChange={e => setVision(e.target.checked)}/>{t('With images')}</label>
        <label className="inline-field">{t('Prompt tokens')}
          <input className="text-input narrow-input" inputMode="numeric" value={prompt} onChange={e => setPrompt(e.target.value)}/></label>
        {anthropic && <select className="popup-button" value={thinking} aria-label={t('Thinking')}
          onChange={e => setThinking(e.target.value as 'adaptive' | 'enabled' | 'disabled' | '')}>
          <option value="">{t('No thinking field')}</option><option value="adaptive">{t('Adaptive thinking')}</option>
          <option value="enabled">{t('Thinking budget')}</option><option value="disabled">{t('Thinking disabled')}</option>
        </select>}
        {anthropic && <select className="popup-button" value={effort} aria-label={t('Effort')} onChange={e => setEffort(e.target.value)}>
          <option value="">{t('No effort')}</option>
          {['low', 'medium', 'high', 'xhigh', 'max'].map(l => <option key={l} value={l}>{t('Effort: {level}', {level: l})}</option>)}
        </select>}
        <label className="inline-field">{t('Output limit')}
          <input className="text-input narrow-input" inputMode="numeric" value={maxTokens} placeholder={t('None')}
            onChange={e => setMaxTokens(e.target.value)}/></label>
      </div>
    </div>
    {plan.error && <ErrorNote>{plan.error}</ErrorNote>}
    {!plan.data && plan.loading && <Spinner label={t('Simulating…')}/>}
    {plan.data && <>
      {route.length === 0
        ? <p className="group-footnote error-text">{t('No connected model is eligible: the request would be refused with the reasons below.')}</p>
        : <div className="group-box table-box auto-route">
          <table className="data-table" aria-label={t('Automatic Routing')}>
            <thead><tr>
              <th scope="col" className="narrow">#</th><th scope="col">{t('Model')}</th><th scope="col">{t('Tier')}</th>
              <th scope="col">{t('Billing')}</th><th scope="col">{t('Estimate')}</th><th scope="col">{t('Why')}</th>
            </tr></thead>
            <tbody>
              {route.map((r, i) => <tr key={`${r.providerId}/${r.modelId}`}>
                <td className="narrow secondary">{i + 1}</td>
                <td className="mono selectable">{r.providerId}/{r.modelId}</td>
                <td>{r.tier ?? '—'}</td>
                <td>{billingLabel(r.billing)}</td>
                <td>{r.estimateMicroUsd === 0 ? t('$0 (as set)') : t('about {amount}', {amount: microUsd(r.estimateMicroUsd)})}</td>
                <td className="secondary small-text">{r.reason}</td>
              </tr>)}
            </tbody>
          </table>
        </div>}
      <p className="group-footnote">
        {route.length > 0 && t('The request goes to the first; the others are its fallbacks, under the usual fallback rules.')}
        {more > 0 && ' ' + plural(more, '{n} more model is eligible but beyond the route.', '{n} more models are eligible but beyond the route.')}
      </p>
      {plan.data.excluded.length > 0 && <div className="auto-excluded">
        <button type="button" className="link-button" onClick={() => setShowExcluded(v => !v)} aria-expanded={showExcluded}>
          {showExcluded ? <ChevronDown size={13} aria-hidden/> : <ChevronRight size={13} aria-hidden/>}
          {plural(plan.data.excluded.length, '{n} model left out', '{n} models left out')}
        </button>
        {showExcluded && <ul className="group-box excluded-list">
          {plan.data.excluded.map(e => <li key={`${e.providerId}/${e.modelId}`}>
            <span className="mono">{e.providerId}/{e.modelId}</span><span className="secondary">{e.reason}</span>
          </li>)}
        </ul>}
      </div>}
    </>}
  </section>;
}
