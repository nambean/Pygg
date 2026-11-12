import { TriangleAlert } from 'lucide-react';
import type { UsageAlert } from '../../lib/api';
import { plural, t } from '../../lib/i18n';
import { microUsd } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';

function alertText(a: UsageAlert): string {
  switch (a.kind) {
    case 'over_limit': return t('{client} has used {used} of its {limit} monthly limit; its requests are refused until next month or a higher limit.',
      {client: a.client ?? '', used: microUsd(a.value), limit: microUsd(a.against)});
    case 'near_limit': return t('{client} has used {used} of its {limit} monthly limit.',
      {client: a.client ?? '', used: microUsd(a.value), limit: microUsd(a.against)});
    case 'spend_spike': return t('The last 24 hours cost {today}, against {average} a day over the week before.',
      {today: microUsd(a.value), average: microUsd(a.against)});
    case 'unknown_costs': return plural(a.value, '{n} request in the last 24 hours has no known cost: give its model a price in Connections.',
      '{n} requests in the last 24 hours have no known cost: give their models a price in Connections.');
    case 'failures': return t('{failed} of the last hour’s {total} requests failed.', {failed: a.value, total: a.against});
  }
}

/** What in the ledger is worth a look now (store::usage_alerts): read from recorded figures, never a forecast. */
export function UsageAlerts({onOpen}: {onOpen?: () => void}) {
  const alerts = useGateway(api => api.usageAlerts(), [], 30_000);
  const list = alerts.data ?? [];
  if (!list.length) return null;
  return <section className="group" aria-label={t('Worth a look')}>
    <div className="group-box usage-alerts">
      {list.map(a => <p key={`${a.kind}-${a.client ?? ''}`} className="usage-alert" role="status">
        <TriangleAlert size={15} aria-hidden className={a.kind === 'over_limit' || a.kind === 'failures' ? 'error-text' : 'warn-text'}/>
        <span>{alertText(a)}</span>
      </p>)}
      {onOpen && <button type="button" className="link-button" onClick={onOpen}>{t('Open Usage')}</button>}
    </div>
  </section>;
}
