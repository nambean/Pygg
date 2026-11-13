import { useState } from 'react';
import type { TokenMix as Mix, TokenMixGroup, TokenParts } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd, tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { Segmented } from '../ui';

type Kind = keyof TokenParts;
const KINDS: Kind[] = ['newInput', 'cacheRead', 'cacheWrite', 'output'];
const label = (k: Kind) => ({newInput: t('New input'), cacheRead: t('Cache reads'), cacheWrite: t('Cache writes'), output: t('Output')}[k]);
const hint = (k: Kind) => ({
  newInput: t('Prompt tokens the provider read in full'),
  cacheRead: t('Prompt tokens served from the provider’s cache, at its cached rate'),
  cacheWrite: t('Prompt tokens written to the cache, which some providers charge extra for'),
  output: t('Tokens the model generated'),
}[k]);
const sum = (p: TokenParts) => p.newInput + p.cacheRead + p.cacheWrite + p.output;
const pct = (part: number, whole: number) => whole > 0 ? Math.round((part / whole) * 100) : 0;

/**
 * Usage → Token Mix (Codex's fifth discussion, T1): which tokens were new, read from cache, written to cache or
 * generated, as tokens or as the known cost split among them. Read from the ledger; it changes nothing, and it is not a
 * saving of Pygg's: the provider's cache does the discounting.
 */
export function TokenMix({days}: {days: number}) {
  const mix = useGateway(api => api.usageTokenMix(days), [days], 15_000);
  const [mode, setMode] = useState<'tokens' | 'cost'>('tokens');
  const m: Mix | null = mix.data ?? null;
  if (!m || m.requests === 0) return null;
  const parts = mode === 'tokens' ? m.tokens : m.cost;
  const whole = sum(parts);
  const value = (n: number) => mode === 'tokens' ? tokens(n) : microUsd(n);
  const input = m.tokens.newInput + m.tokens.cacheRead + m.tokens.cacheWrite;
  const summary = KINDS.map(k => `${label(k)} ${value(parts[k])} (${pct(parts[k], whole)}%)`).join(', ');

  return <section className="group">
    <div className="group-heading">
      <h2>{t('Token Mix')}</h2>
      <span className="heading-actions">
        <Segmented label={t('Show the token mix as')} value={mode} onChange={setMode}
          options={[{value: 'tokens', label: t('Tokens')}, {value: 'cost', label: t('Estimated Cost')}]}/>
      </span>
    </div>
    <div className="group-box token-mix">
      <p className="group-intro">{t('See which tokens were new, read from cache, written to cache, or generated.')}</p>
      {m.withTokens === 0
        ? <p className="secondary">{t('No token counts in this period: the providers did not report any with their answers.')}</p>
        : mode === 'cost' && whole === 0
          ? <p className="secondary">{t('No known cost to split in this period: the answers were free, local, on a plan, or of unknown cost.')}</p>
          : <>
            <dl className="savings-figures token-figures">
              {KINDS.map(k => <div key={k} title={hint(k)}>
                <dt><span className={`mix-dot tok-${k}`} aria-hidden/>{label(k)}</dt>
                <dd>{value(parts[k])} <small className="secondary">{pct(parts[k], whole)}%</small></dd>
              </div>)}
            </dl>
            <div className="mix-bar" role="img" aria-label={summary}>
              {KINDS.filter(k => parts[k] > 0).map(k => <span key={k} className={`tok-${k}`}
                style={{width: `${(parts[k] / whole) * 100}%`}} title={`${label(k)}: ${value(parts[k])}`}/>)}
            </div>
          </>}
      <p className="group-footnote">
        {m.withTokens > 0 && <>{t('Input served from cache: {share}%.', {share: pct(m.tokens.cacheRead, input)})}{' '}</>}
        {t('Token counts on {n} of {total} requests.', {n: m.withTokens, total: m.requests})}{' '}
        {mode === 'cost'
          ? <>{t('Each request’s recorded cost, split by its model’s current prices.')}{' '}
            {m.unknownCostRequests > 0 && <span className="warn-text">{plural(m.unknownCostRequests,
              '{n} request with unknown cost is not in it.', '{n} requests with unknown cost are not in it.')} </span>}
            {m.unsplitMicroUsd > 0 && t('{amount} could not be split: its model has no price now.', {amount: microUsd(m.unsplitMicroUsd)})}</>
          : t('A provider that reports no cache figures shows its whole prompt as new input. The cache’s discount is the provider’s, not a saving of Pygg’s.')}
      </p>
      {m.withTokens > 0 && <details className="token-mix-details">
        <summary>{t('By client and model')}</summary>
        <div className="table-scroll"><MixTable title={t('Client')} groups={m.byClient}/></div>
        <div className="table-scroll"><MixTable title={t('Model')} groups={m.byModel} mono/></div>
      </details>}
    </div>
  </section>;
}

function MixTable({title, groups, mono}: {title: string; groups: TokenMixGroup[]; mono?: boolean}) {
  return <table className="data-table" aria-label={title}>
    <thead><tr>
      <th scope="col">{title}</th>
      {KINDS.map(k => <th key={k} scope="col" className="num">{label(k)}</th>)}
      <th scope="col" className="num">{t('From cache')}</th><th scope="col" className="num">{t('Known cost')}</th>
    </tr></thead>
    <tbody>
      {groups.map(g => {
        const input = g.tokens.newInput + g.tokens.cacheRead + g.tokens.cacheWrite;
        return <tr key={g.key}>
          <td className={mono ? 'mono' : ''} title={g.key}>{g.key || t('No client')}</td>
          {KINDS.map(k => <td key={k} className="num">{tokens(g.tokens[k])}</td>)}
          <td className="num">{input > 0 ? `${pct(g.tokens.cacheRead, input)}%` : '—'}</td>
          <td className="num">{microUsd(g.knownMicroUsd)}</td>
        </tr>;
      })}
    </tbody>
  </table>;
}
