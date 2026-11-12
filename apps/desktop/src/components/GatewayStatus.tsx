import type { GatewayStatus as Status } from '../lib/api';
import { t } from '../lib/i18n';

/**
 * Sidebar footer status (SCREENS.md §2.2). It names the local gateway's own state; an upstream provider
 * outage is reported per connection and never turns a healthy gateway into "offline".
 */
export function GatewayStatus({status, error, onOpen}: {status: Status | null; error: string | null; onOpen: () => void}) {
  const [text, dot] = error && !status ? [t('Core isn’t responding'), 'error']
    : !status ? [t('Connecting…'), '']
    : status.state === 'running' ? [t('Gateway Running · {port}', {port: status.port}), 'ok']
    : status.state === 'failed' ? [t('Gateway Couldn’t Start'), 'error']
    : [t('Gateway Stopped'), 'off'];
  return <button type="button" className="runtime-status" onClick={onOpen} title={status?.error ?? t('Open Home')}>
    <span className={`dot ${dot}`} aria-hidden/>
    <span role="status">{text}</span>
  </button>;
}
