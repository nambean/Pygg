import type { CacheLifetime as Shadow } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd, tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';

const signed = (n: number) => (n >= 0 ? microUsd(n) : `−${microUsd(-n)}`);

/**
 * A longer cache lifetime, measured in shadow (ROADMAP item 4, store::cache_lifetime): on models that bill cache writes,
 * the answers that missed only because the five-minute cache had expired, and what a one-hour lifetime or a keep-alive
 * would have cost and saved. Nothing is sent or changed.
 */
export function CacheLifetime({days}: {days: number}) {
  const shadow = useGateway(api => api.usageShadow(days), [days], 30_000);
  const c: Shadow | null = shadow.data?.lifetime ?? null;
  if (!c || c.sessions === 0) return null;
  const best = Math.max(c.hourNetMicroUsd, c.keepAliveNetMicroUsd);
  const verdict = c.expiredMisses === 0 ? t('no pauses cost a cache')
    : best > 0 ? t('could save {amount}', {amount: microUsd(best)}) : t('would cost more than it saves');
  return <details className="group observation">
    <summary><h2>{t('Cache Lifetime')}</h2> <span className="secondary">{t('measured only · {verdict}', {verdict})}</span></summary>
    <div className="group-box totals">
      <div className="totals-main">
        <span className="totals-figure">{c.expiredMisses === 0 ? t('Nothing lost') : signed(best)}</span>
        {c.expiredMisses > 0 && <span className="secondary">{c.hourNetMicroUsd >= c.keepAliveNetMicroUsd
          ? t('net, with a one-hour cache lifetime') : t('net, with a keep-alive')}</span>}
      </div>
      <div className="totals-parts">
        <span>{plural(c.expiredMisses, '{n} answer missed the cache after a pause of 5 to 60 minutes',
          '{n} answers missed the cache after a pause of 5 to 60 minutes')}{' '}
          {plural(c.requests, 'of {n} measured', 'of {n} measured')}</span>
        {c.expiredMisses > 0 && <>
          <span>{t('Reading their ≈{n} tokens instead of writing them again: {amount} less',
            {n: tokens(c.recoveredTokens), amount: microUsd(c.savedMicroUsd)})}</span>
          <span>{t('One-hour lifetime: {extra} more on the other cache writes, {net} net',
            {extra: microUsd(c.hourExtraMicroUsd), net: signed(c.hourNetMicroUsd)})}</span>
          <span>{t(c.keepAliveRequests === 1
            ? 'Keep-alive: {n} re-send to hold the cache, {extra} in cache reads, {net} net'
            : 'Keep-alive: {n} re-sends to hold the cache, {extra} in cache reads, {net} net',
          {n: c.keepAliveRequests, extra: microUsd(c.keepAliveExtraMicroUsd), net: signed(c.keepAliveNetMicroUsd)})}</span>
        </>}
        {c.unpriced > 0 && <span>{plural(c.unpriced, '{n} answer on a model without a cached price is not in the money',
          '{n} answers on models without a cached price are not in the money')}</span>}
      </div>
      <p className="group-footnote">{t('Measured only, on models that bill cache writes, such as Claude: their prompt cache lasts five minutes, so an answer after a longer pause writes the whole conversation again. A one-hour lifetime would make every cache write cost twice the input price instead of 1.25 times; a keep-alive would re-send each session’s last request every 4.5 minutes for up to an hour, also after the last answer of a session. Pygg will offer one only if it stays above zero.')}</p>
    </div>
  </details>;
}
