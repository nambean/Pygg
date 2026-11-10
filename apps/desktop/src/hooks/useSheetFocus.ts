import { useEffect, type RefObject } from 'react';

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], summary, [tabindex]:not([tabindex="-1"])';

/**
 * Focus for a modal sheet, as in DownFetch: moves into the sheet when it opens (unless a control there
 * already took it with autoFocus), keeps Tab and Shift+Tab inside it, and gives focus back to where it
 * was when the sheet closes. The window behind is inert meanwhile (main.tsx).
 */
export function useSheetFocus(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const sheet = ref.current;
    if (!sheet) return;
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const controls = () => Array.from(sheet.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(el => el.offsetParent !== null);
    if (!sheet.contains(document.activeElement)) (controls()[0] ?? sheet).focus();
    function trap(event: KeyboardEvent) {
      if (event.key !== 'Tab') return;
      const all = controls();
      if (!all.length) { event.preventDefault(); return; }
      const first = all[0], last = all[all.length - 1];
      if (event.shiftKey && (document.activeElement === first || !sheet!.contains(document.activeElement))) {
        event.preventDefault(); last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault(); first.focus();
      }
    }
    sheet.addEventListener('keydown', trap);
    return () => { sheet.removeEventListener('keydown', trap); if (before?.isConnected) before.focus(); };
  }, [ref]);
}
