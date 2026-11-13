import type { ShadowSummary } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';

/**
 * Token reduction in shadow (ROADMAP P1-4): how much tool output requests repeated, against their input and what the
 * prompt cache already served. Nothing is removed: deduplicating would rewrite the agent's history and break the cache.
 */
export function RepeatedOutput({days}: {days: number}) {
  const shadow = useGateway(api => api.usageShadow(days), [days], 30_000);
  const s: ShadowSummary | null = shadow.data ?? null;
  if (!s || s.requests === 0) return null;
  const share = s.inputTokens ? Math.round((s.repeatedTokens / s.inputTokens) * 1000) / 10 : 0;
  const cached = s.inputTokens ? Math.round((s.cacheReadTokens / s.inputTokens) * 100) : 0;
  // An observation, measured in shadow: closed until asked for, so Summary stays short (Codex review 7).
  return <details className="group observation">
    <summary><h2>{t('Repeated Tool Output')}</h2> <span className="secondary">{t('measured only · {share}% of input', {share})}</span></summary>
    <div className="group-box totals">
      <div className="totals-main">
        <span className="totals-figure">{t('≈{n} tokens', {n: tokens(s.repeatedTokens)})}</span>
        <span className="secondary">{t('{share}% of input', {share})}</span>
      </div>
      <div className="totals-parts">
        <span>{plural(s.requestsWithRepeats, '{n} request repeated a tool’s output', '{n} requests repeated a tool’s output')}</span>
        <span>{plural(s.requests, 'of {n} with tool output', 'of {n} with tool output')}</span>
        <span>{t('{cached}% of their input came from the prompt cache', {cached})}</span>
      </div>
      <p className="group-footnote">{t('Measured only: Pygg changes nothing in what agents send. Removing repeated output would also rewrite the agent’s history and break its prompt cache, which already serves most of this at a discount.')}</p>
    </div>
  </details>;
}
