// Traffic-light window buttons (close, minimize, zoom) for Windows and Linux, where the native title bar is
// turned off (tauri.windows.conf.json). On macOS the system draws its own. Same component as DownFetch, kept
// on purpose (docs/design/SCREENS.md, D12).
import { useEffect, useRef, useState } from 'react';
import { t } from '../lib/i18n';
import { setSnapArea, SNAP_HOVER_EVENT } from '../lib/native';
import { PLATFORM } from '../lib/platform';

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

  // Windows 11 offers snap layouts only over what its hit test calls a maximize button, and the page's buttons are
  // inside WebView2. So the core lays an invisible native window over the green button (src-tauri/src/snap.rs):
  // it gets the button's rectangle in physical pixels whenever layout, size or scaling changes. That window takes
  // the pointer there, maximizes or restores on click itself, and says when the pointer enters or leaves, so the
  // buttons still show their glyphs on hover. The keyboard and screen readers still reach the button here.
  const zoomRef = useRef<HTMLButtonElement>(null);
  const [pointerOnZoom, setPointerOnZoom] = useState(false);
  useEffect(() => {
    const button = zoomRef.current;
    if (PLATFORM !== 'windows' || !button) return;
    let frame = 0;
    const send = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const r = button.getBoundingClientRect();
        const scale = window.devicePixelRatio || 1;
        const area = r.width > 0 && r.height > 0
          ? { x: r.left * scale, y: r.top * scale, width: r.width * scale, height: r.height * scale } : null;
        void setSnapArea(area).catch(() => {});
      });
    };
    send();
    const resized = new ResizeObserver(send);
    resized.observe(button);
    window.addEventListener('resize', send);
    // The text direction moves the buttons to the other side.
    const changed = new MutationObserver(send);
    changed.observe(document.documentElement, { attributes: true, attributeFilter: ['dir', 'lang', 'style', 'class'] });
    // devicePixelRatio changes when the window moves to a screen with other scaling, or the page is zoomed.
    let scaleQuery: MediaQueryList | null = null;
    const watchScale = () => {
      scaleQuery?.removeEventListener('change', onScale);
      scaleQuery = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      scaleQuery.addEventListener('change', onScale);
    };
    function onScale() { watchScale(); send(); }
    watchScale();
    let stop: (() => void) | undefined;
    let gone = false;
    void import('@tauri-apps/api/event')
      .then((m) => m.listen<boolean>(SNAP_HOVER_EVENT, (e) => setPointerOnZoom(e.payload === true)))
      .then((unlisten) => { if (gone) unlisten(); else stop = unlisten; }, () => {});
    return () => {
      gone = true;
      stop?.();
      cancelAnimationFrame(frame);
      resized.disconnect();
      changed.disconnect();
      window.removeEventListener('resize', send);
      scaleQuery?.removeEventListener('change', onScale);
      void setSnapArea(null).catch(() => {});
    };
  }, []);

  const run = (action: (w: Win) => Promise<void>) => () => { void win().then((w) => w && action(w)); };

  return <div className={`window-controls${active ? '' : ' inactive'}${pointerOnZoom ? ' pointer-on-zoom' : ''}`} role="group" aria-label={t('Window')}>
    <button type="button" className="light close" onClick={run((w) => w.close())} aria-label={t('Close')} title={t('Close')}>
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.5 1.5l5 5M6.5 1.5l-5 5"/></svg>
    </button>
    <button type="button" className="light minimize" onClick={run((w) => w.minimize())} aria-label={t('Minimize')} title={t('Minimize')}>
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.2 4h5.6"/></svg>
    </button>
    <button ref={zoomRef} type="button" className="light zoom" onClick={run((w) => w.toggleMaximize())} aria-label={zoomLabel} title={zoomLabel}>
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M2 2.2h3.8V6M6 6H2.2V2.2" className="zoom-glyph"/></svg>
    </button>
  </div>;
}
