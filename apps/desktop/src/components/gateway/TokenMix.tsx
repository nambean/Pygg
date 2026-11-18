import { useState } from 'react';
import type { CacheMissCause, CacheMissGroup, ClassGroup, PrefixMeter, PrefixSession, RequestClasses, TokenMix as Mix, TokenMixGroup, TokenParts } from '../../lib/api';
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

/** Why an answer missed the cache, short and in full (store::cache_miss). The ledger's best reading, not the provider's word. */
export const cacheMissLabel = (cause: CacheMissCause | string) => ({
  not_reported: t('Not reported'), below_minimum: t('Prompt too short'), marker_dropped: t('Cache hints dropped'),
  first_request: t('First request of a session'), model_switched: t('Model switched'), not_cached_here: t('Model does not cache'),
  cache_expired: t('Cache expired'), prefix_changed: t('Prompt start changed'), no_session: t('No session'),
} as Record<string, string>)[cause] ?? cause;
export const cacheMissText = (cause: CacheMissCause | string) => ({
  not_reported: t('The provider did not say how much of the prompt came from its cache.'),
  below_minimum: t('Under about 1,024 tokens, which providers do not cache.'),
  marker_dropped: t('The client marked the prompt for caching, but this provider takes no hints in this format.'),
  first_request: t('Nothing was cached for this session on this model yet.'),
  model_switched: t('The session’s last answer came from another model, whose cache this one cannot read.'),
  not_cached_here: t('This model has read nothing from its cache in the last 7 days.'),
  cache_expired: t('More than 5 minutes since this session last used this model.'),
  prefix_changed: t('The same model answered minutes ago, so the start of the prompt (system prompt, tools or early history) likely changed.'),
  no_session: t('The request named no session, so Pygg cannot tell why.'),
} as Record<string, string>)[cause] ?? cause;

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
      {m.misses.length > 0 && <details className="token-mix-details">
        <summary>{plural(m.misses.reduce((n, g) => n + g.requests, 0),
          'Why the cache was missed: {n} answer', 'Why the cache was missed: {n} answers')}</summary>
        <div className="table-scroll"><MissTable groups={m.misses}/></div>
        <p className="group-footnote">{t('Answers that read nothing from the provider’s cache, with the likeliest cause read from the ledger. The provider does not say why.')}</p>
      </details>}
      {m.prefix.sessions > 0 && <PrefixDetails meter={m.prefix}/>}
      {m.classes.requests > 0 && <ClassDetails classes={m.classes}/>}
      {m.withTokens > 0 && <details className="token-mix-details">
        <summary>{t('By client and model')}</summary>
        <div className="table-scroll"><MixTable title={t('Client')} groups={m.byClient}/></div>
        <div className="table-scroll"><MixTable title={t('Model')} groups={m.byModel} mono/></div>
      </details>}
    </div>
  </section>;
}

const classLabel = (key: string) => ({
  main: t('Main conversation'), subagent: t('Subagents'), workflow: t('Workflows'), compaction: t('Compaction'),
  auxiliary: t('Side requests'), other: t('Other'),
}[key] ?? key);

/** Claude Code's requests by kind (gateway::hints::request_classes): what the main conversation, subagents, compaction
 *  and side requests cost, and how far subagents fanned out. Only launches started with `pygg run claude` send these. */
function ClassDetails({classes: c}: {classes: RequestClasses}) {
  return <details className="token-mix-details">
    <summary>{plural(c.requests, 'By request kind (Claude Code): {n} request', 'By request kind (Claude Code): {n} requests')}</summary>
    <div className="table-scroll"><ClassTable title={t('Kind')} groups={c.groups} label={classLabel}/></div>
    {c.agentTypes.length > 0 && <div className="table-scroll"><ClassTable title={t('Subagent type')} groups={c.agentTypes} label={k => k}/></div>}
    <p className="group-footnote">
      {c.agents > 0 && <>{plural(c.agents, '{n} subagent', '{n} subagents')}{', '}
        {t('at most {n} working at once in a session, at most {m} started by one prompt.',
          {n: c.peakParallelAgents, m: c.mostAgentsOnePrompt})}{' '}</>}
      {t('Each subagent starts a conversation of its own, so its first request writes a new cache. Claude Code says what each request is only when started with pygg run claude.')}
    </p>
  </details>;
}

function ClassTable({title, groups, label}: {title: string; groups: ClassGroup[]; label: (k: string) => string}) {
  return <table className="data-table" aria-label={title}>
    <thead><tr>
      <th scope="col">{title}</th><th scope="col" className="num">{t('Requests')}</th>
      <th scope="col" className="num">{t('Agents')}</th><th scope="col" className="num">{t('From cache')}</th>
      <th scope="col" className="num">{t('Cache writes')}</th><th scope="col" className="num">{t('Known cost')}</th>
    </tr></thead>
    <tbody>
      {groups.map(g => <tr key={g.key}>
        <td title={g.key}>{label(g.key)}</td>
        <td className="num">{g.requests}</td>
        <td className="num">{g.agents > 0 ? g.agents : '—'}</td>
        <td className="num">{g.promptTokens > 0 ? `${pct(g.cacheReadTokens, g.promptTokens)}%` : '—'}</td>
        <td className="num">{tokens(g.cacheWriteTokens)}</td>
        <td className="num">{microUsd(g.knownMicroUsd)}{g.unknownCost > 0 && <span className="secondary"> +{g.unknownCost} ?</span>}</td>
      </tr>)}
    </tbody>
  </table>;
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

/** What sessions send before their conversation (store::prefix_meter): the tools and system prompt at a session's first
 *  request, how often they changed, and how often they went again without a cache read. Numbers only. */
function PrefixDetails({meter}: {meter: PrefixMeter}) {
  const each = (n: number) => tokens(Math.round(n / meter.sessions));
  return <details className="token-mix-details">
    <summary>{plural(meter.sessions, 'Tools and system prompt: {n} session', 'Tools and system prompt: {n} sessions')}</summary>
    <dl className="savings-figures token-figures">
      <div title={t('Tool definitions at each session’s first request, on average')}>
        <dt>{t('Tools per session')}</dt><dd>{each(meter.firstToolsTokens)}</dd>
      </div>
      <div title={t('System prompt at each session’s first request, on average')}>
        <dt>{t('System prompt per session')}</dt><dd>{each(meter.firstSystemTokens)}</dd>
      </div>
      <div title={t('Requests whose tools or system prompt differed from the session’s request before')}>
        <dt>{t('Changed in a session')}</dt><dd>{meter.changes === 0 ? t('None') : plural(meter.changes, '{n} time', '{n} times')}</dd>
      </div>
      <div title={t('Later requests that read nothing from the cache, so the tools and system prompt went again in full or as a cache write')}>
        <dt>{t('Sent again uncached')}</dt>
        <dd>{meter.resent === 0 ? t('None') : <>{tokens(meter.resentTokens)}{' '}
          <small className="secondary">{plural(meter.resent, '{n} request', '{n} requests')}</small></>}</dd>
      </div>
    </dl>
    <div className="table-scroll"><PrefixTable sessions={meter.top}/></div>
    <p className="group-footnote">{t('What each session sends before its conversation, measured from the request; sizes are Pygg’s estimate. Sent again uncached counts later requests of a session that read nothing from the provider’s cache. Pygg keeps these numbers only, never the text. Requests without a session are not counted.')}</p>
  </details>;
}

function PrefixTable({sessions}: {sessions: PrefixSession[]}) {
  return <table className="data-table" aria-label={t('Sessions that sent their tools and system prompt again the most')}>
    <thead><tr>
      <th scope="col">{t('Client')}</th><th scope="col">{t('Session')}</th>
      <th scope="col" className="num">{t('Tools')}</th><th scope="col" className="num">{t('System prompt')}</th>
      <th scope="col" className="num">{t('Requests')}</th><th scope="col" className="num">{t('Changed')}</th>
      <th scope="col" className="num">{t('Sent again')}</th>
    </tr></thead>
    <tbody>
      {sessions.map(s => <tr key={`${s.client}|${s.session}`}>
        <td title={s.client}>{s.client || t('No client')}</td>
        <td className="mono" title={s.session}>{s.session}</td>
        <td className="num">{tokens(s.toolsTokens)}</td>
        <td className="num">{tokens(s.systemTokens)}</td>
        <td className="num">{s.requests}</td>
        <td className="num">{s.changes}</td>
        <td className="num">{s.resent > 0 ? `${tokens(s.resentTokens)} (${s.resent})` : '—'}</td>
      </tr>)}
    </tbody>
  </table>;
}

function MissTable({groups}: {groups: CacheMissGroup[]}) {
  return <table className="data-table" aria-label={t('Why the cache was missed')}>
    <thead><tr>
      <th scope="col">{t('Cause')}</th><th scope="col" className="num">{t('Answers')}</th>
      <th scope="col" className="num">{t('Prompt tokens')}</th><th scope="col">{t('What it means')}</th>
    </tr></thead>
    <tbody>
      {groups.map(g => <tr key={g.cause}>
        <td className="nowrap">{cacheMissLabel(g.cause)}</td>
        <td className="num">{g.requests}</td>
        <td className="num">{tokens(g.inputTokens)}</td>
        <td className="secondary">{cacheMissText(g.cause)}</td>
      </tr>)}
    </tbody>
  </table>;
}
