import type { ClearingShadow as Shadow } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd, tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';

/**
 * Clearing old tool results, measured in shadow (ROADMAP item 4, store::clearing_shadow): what Anthropic's context editing
 * at its defaults would have taken out of OpenAI-format clients' requests, and the cache writes it would have caused.
 * Nothing is cleared; Pygg offers it only if the net stays above zero.
 */
export function ClearingShadow({days}: {days: number}) {
  const shadow = useGateway(api => api.usageShadow(days), [days], 30_000);
  const c: Shadow | null = shadow.data?.clearing ?? null;
  if (!c || c.requests === 0) return null;
  const net = c.netMicroUsd;
  const verdict = c.overTrigger === 0 ? t('nothing to clear')
    : net > 0 ? t('would save {amount}', {amount: microUsd(net)}) : t('would cost {amount} more', {amount: microUsd(-net)});
  return <details className="group observation">
    <summary><h2>{t('Clearing Old Tool Results')}</h2> <span className="secondary">{t('measured only · {verdict}', {verdict})}</span></summary>
    <div className="group-box totals">
      <div className="totals-main">
        <span className="totals-figure">{c.overTrigger === 0 ? t('Nothing to clear') : net >= 0 ? microUsd(net) : `−${microUsd(-net)}`}</span>
        {c.overTrigger > 0 && <span className="secondary">{t('net, after the cache writes clearing would cause')}</span>}
      </div>
      <div className="totals-parts">
        <span>{plural(c.overTrigger, '{n} request past 100,000 prompt tokens', '{n} requests past 100,000 prompt tokens')}{' '}
          {plural(c.requests, 'of {n} measured', 'of {n} measured')}</span>
        {c.overTrigger > 0 && <>
          <span>{t('≈{n} tokens of old tool results not sent: {amount} less', {n: tokens(c.clearedTokens), amount: microUsd(c.savedMicroUsd)})}</span>
          <span>{t('≈{n} tokens written to the cache again: {amount} more', {n: tokens(c.rewrittenTokens), amount: microUsd(c.extraMicroUsd)})}</span>
        </>}
        {c.unpriced > 0 && <span>{plural(c.unpriced, '{n} request on a model without a price is not in the money',
          '{n} requests on models without a price are not in the money')}</span>}
      </div>
      <p className="group-footnote">{t('Measured only, for OpenAI-format clients such as Codex; requests in Anthropic’s format, as Claude Code and OpenCode send them, are never measured or changed. At Anthropic’s defaults: past 100,000 prompt tokens, every tool result but the last three would be cleared. Each time one more is cleared, the rest of the conversation is written to the cache again. Clearing would also take those results away from the model, which this does not measure. Pygg will offer it only if this stays above zero.')}</p>
    </div>
  </details>;
}
