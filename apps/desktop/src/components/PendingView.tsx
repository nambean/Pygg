import { Construction } from 'lucide-react';
import { t } from '../lib/i18n';

/**
 * A destination whose screen is designed (docs/design/SCREENS.md) but not built. It says so plainly
 * instead of showing empty data that could be mistaken for a real, empty configuration.
 */
export function PendingView({title, purpose, section}: {title: string; purpose: string; section: string}) {
  return <div className="placeholder">
    <Construction size={28} strokeWidth={1.6} aria-hidden className="placeholder-icon"/>
    <h2>{t('{name} isn’t built yet', {name: title})}</h2>
    <p>{purpose}</p>
    <p className="mono">{`docs/design/SCREENS.md ${section}`}</p>
  </div>;
}
