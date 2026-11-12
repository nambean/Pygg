import { useMemo } from 'react';
import type { GatewayStatus } from '../../lib/api';
import { t } from '../../lib/i18n';
import { useGateway } from '../../hooks/useGateway';
import { Integrations } from './Agents';
import { ClientCheck, ClientSetup, type Navigate } from './Overview';

/**
 * CLI Tools (SCREENS.md §5.8): point a coding tool at Pygg, as 9router's and OmniRoute's CLI Tools pages do. Codex and
 * OpenCode in one click (integrations.rs: backup, a key of their own, the change, a check; Remove puts it back), the
 * others by hand. The newest request says whether it works; nothing is sent to find out.
 */
export function CliTools({status, navigate}: {status: GatewayStatus | null; navigate: Navigate}) {
  const combos = useGateway(api => api.combos());
  const models = useGateway(api => api.models());
  const clients = useGateway(api => api.clients());
  const usage = useGateway(api => api.usage(1), [], 10_000);
  const routes = useMemo(() => [
    {value: 'auto', label: t('auto (automatic)')}, {value: 'auto/cheap', label: t('auto/cheap (automatic, lowest cost)')},
    ...(combos.data ?? []).map(c => ({value: c.name, label: t('{name} (combo)', {name: c.name})})),
    ...(models.data ?? []).filter(m => m.enabled).map(m => ({value: `${m.providerId}/${m.modelId}`, label: `${m.providerId}/${m.modelId}`})),
  ], [combos.data, models.data]);
  const hasKey = (clients.data ?? []).some(c => !c.revokedAt);
  return <div className="page">
    <p className="page-intro">{t('Point your coding tool at Pygg and choose the model auto, then work as usual. Pygg picks an eligible model for each request from your providers, free and cheap ones first where they qualify, within your limits, and records what each request cost.')}</p>
    <section className="group">
      <h2>{t('One-Click Setup')}</h2>
      <p className="group-intro">{t('Pygg backs up the tool’s configuration, gives it a Pygg API key of its own, writes the change and checks it. Remove puts the configuration back and revokes the key.')}</p>
      <Integrations kind="gateway"/>
    </section>
    <ClientSetup baseUrl={status?.baseUrl ?? `http://127.0.0.1:${status?.port ?? 8741}`} routes={routes}
      hasToken={hasKey} onCreateToken={() => navigate('endpoint', 'create')}/>
    {usage.data && <ClientCheck last={usage.data.recent[0]} onOpen={() => navigate('usage')}/>}
  </div>;
}
