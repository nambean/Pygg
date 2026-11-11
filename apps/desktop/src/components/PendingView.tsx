import type { ReactNode } from 'react';
import { Construction } from 'lucide-react';
import { t } from '../lib/i18n';

/**
 * A destination whose screen is designed (docs/design/SCREENS.md) but not built. It says so plainly
 * instead of showing empty data that could be mistaken for a real, empty configuration, and points to
 * where the controls that already exist live.
 */
export function PendingView({title, purpose, children}: {title: string; purpose: string; children?: ReactNode}) {
  return <div className="placeholder">
    <Construction size={28} strokeWidth={1.6} aria-hidden className="placeholder-icon"/>
    <h2>{t('{name} isn’t built yet', {name: title})}</h2>
    <p>{purpose}</p>
    {children && <div className="placeholder-actions">{children}</div>}
  </div>;
}
