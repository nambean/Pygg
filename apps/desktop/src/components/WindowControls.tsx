// Traffic-light window buttons (close, minimize, zoom) for Windows and Linux, where the native title bar is
// turned off (tauri.windows.conf.json). On macOS the system draws its own. Same component as DownFetch, kept
// on purpose (docs/design/SCREENS.md, D12).
import { useEffect, useState } from 'react';
import { t } from '../lib/i18n';

type Win = { close(): Promise<void>; minimize(): Promise<void>; toggleMaximize(): Promise<void>;
  isMaximized(): Promise<boolean>; onResized(handler: () => void): Promise<() => void> };

let current: Promise<Win | null> | null = null;
function win(): Promise<Win | null> {
  current ??= import('@tauri-apps/api/window').then((m) => m.getCurrentWindow() as unknown as Win).catch(() => null);
  return current;
}

export function WindowControls() {
  const [active, setActive] = useState(() => document.hasFocus());
  useEffect(() => {
    const on = () => setActive(true);
    const off = () => setActive(false);
    window.addEventListener('focus', on);
    window.addEventListener('blur', off);
    return () => { window.removeEventListener('focus', on); window.removeEventListener('blur', off); };
  }, []);
  // The green button is named for what it does on Windows and Linux, Maximize or Restore, as screen readers announce
  // the native caption buttons (D12).
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    void win().then(async (w) => {
      if (!w || gone) return;
      const read = () => { void w.isMaximized().then(setMaximized, () => {}); };
      read();
      const unlisten = await w.onResized(read).catch(() => undefined);
      if (gone) unlisten?.(); else stop = unlisten;
    });
    return () => { gone = true; stop?.(); };
  }, []);
  const zoomLabel = maximized ? t('Restore') : t('Maximize');

  const run = (action: (w: Win) => Promise<void>) => () => { void win().then((w) => w && action(w)); };

  return <div className={`window-controls${active ? '' : ' inactive'}`} role="group" aria-label={t('Window')}>
    <button type="button" className="light close" onClick={run((w) => w.close())} aria-label={t('Close')} title={t('Close')}>
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.5 1.5l5 5M6.5 1.5l-5 5"/></svg>
    </button>
    <button type="button" className="light minimize" onClick={run((w) => w.minimize())} aria-label={t('Minimize')} title={t('Minimize')}>
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.2 4h5.6"/></svg>
    </button>
    <button type="button" className="light zoom" onClick={run((w) => w.toggleMaximize())} aria-label={zoomLabel} title={zoomLabel}>
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M2 2.2h3.8V6M6 6H2.2V2.2" className="zoom-glyph"/></svg>
    </button>
  </div>;
}
