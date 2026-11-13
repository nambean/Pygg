import { useState } from 'react';
import type { GatewayStatus } from '../../lib/api';
import { t } from '../../lib/i18n';
import { CopyButton } from '../ui';
import { Clients } from './Clients';
import { ConfigureSheet, GatewayCard } from './Overview';

/**
 * Endpoint (SCREENS.md §5.7): where tools send their requests, and the Pygg API keys they use. The same screen as
 * 9router's and OmniRoute's Endpoint, with Pygg's per-key limits.
 */
export function Endpoint({status, onStatus, intent, onIntentDone}: {status: GatewayStatus | null; onStatus: (s: GatewayStatus) => void;
  intent?: 'create'; onIntentDone: () => void}) {
  const [configuring, setConfiguring] = useState(false);
  const base = status?.baseUrl ?? `http://127.0.0.1:${status?.port ?? 8741}`;
  const rows: {label: string; hint: string; value: string}[] = [
    {label: t('OpenAI-compatible'), hint: t('Chat Completions and Responses: Codex, OpenCode, editors, SDKs'), value: `${base}/v1`},
    {label: t('Anthropic'), hint: t('Messages API: Claude Code, Anthropic SDKs'), value: base},
    {label: t('Model'), hint: t('auto picks a model for each request; auto/cheap the cheapest that qualifies. A combo’s name, or connection/model, also works.'), value: 'auto'},
  ];
  return <div className="page">
    <GatewayCard status={status} onStatus={onStatus} onConfigure={() => setConfiguring(true)}
      copies={[{label: t('Copy OpenAI URL'), text: `${base}/v1`}, {label: t('Copy Anthropic URL'), text: base}]}/>
    <section className="group">
      <h2>{t('Base URLs')}</h2>
      <div className="group-box">
        {rows.map(r => <div key={r.label} className="group-row">
          <span className="group-label">{r.label}<small>{r.hint}</small></span>
          <span className="group-control endpoint-value"><span className="mono selectable">{r.value}</span>
            <CopyButton text={r.value} label={t('Copy')}/></span>
        </div>)}
      </div>
      <p className="group-footnote">{t('Send a Pygg API key as the bearer token (or x-api-key). The gateway listens on this computer only.')}</p>
    </section>
    <Clients intent={intent} onIntentDone={onIntentDone}/>
    {configuring && status && <ConfigureSheet status={status} onStatus={onStatus} onClose={() => setConfiguring(false)}/>}
  </div>;
}
