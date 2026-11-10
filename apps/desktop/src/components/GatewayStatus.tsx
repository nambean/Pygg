import { useEffect, useState } from 'react';
import { nativeHealth, runningInDesktop, type HealthStatus } from '../lib/native';
import { t } from '../lib/i18n';

/**
 * Sidebar footer status (SCREENS.md §2.2). It names the local gateway's own state; an upstream provider
 * outage will be reported separately and never turns a healthy gateway into "offline".
 */
export function GatewayStatus() {
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => {
    let mounted = true;
    nativeHealth().then(h => { if (mounted) setHealth(h); }).catch(() => { if (mounted) setError(true); });
    return () => { mounted = false; };
  }, []);

  const [text, dot] = error ? [t('Core isn’t responding'), 'error']
    : !runningInDesktop ? [t('Browser preview'), '']
    : !health ? [t('Connecting…'), '']
    : health.gateway === 'running' ? [t('Gateway Running'), 'ok']
    : health.gateway === 'stopped' ? [t('Gateway Stopped'), 'error']
    : [t('Gateway not built yet'), ''];
  return <span className="runtime-status" role="status">
    <span className={`dot ${dot}`} aria-hidden/>
    {text}
  </span>;
}
