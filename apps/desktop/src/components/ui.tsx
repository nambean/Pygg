/**
 * Small shared controls for the gateway screens, in DownFetch's style: sheets, switches, segmented
 * controls, copy buttons and status badges. Status always pairs an icon or dot with a word (§3.4).
 */
import React, { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, CircleAlert, Copy, LoaderCircle } from 'lucide-react';
import { t } from '../lib/i18n';
import { useSheetFocus } from '../hooks/useSheetFocus';

let openSheets = 0;

/**
 * A modal sheet: rendered outside the window, which is made inert while it is open. Escape closes it unless
 * it is busy; clicking the backdrop does not (SCREENS.md §6).
 */
export function Sheet({title, subtitle, onClose, busy, children, footer, className}: {
  title: string; subtitle?: string; onClose: () => void; busy?: boolean; children: React.ReactNode;
  footer: React.ReactNode; className?: string;
}) {
  const dialog = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const win = document.querySelector('.window');
    openSheets++;
    win?.setAttribute('inert', '');
    return () => { if (--openSheets === 0) win?.removeAttribute('inert'); };
  }, []);
  // After the effect above: cleanups run in order, so the window is no longer inert when focus goes back to it.
  useSheetFocus(dialog);
  const onKey = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape' && !busy) { event.stopPropagation(); onClose(); }
  };
  return createPortal(<div className="sheet-backdrop" onKeyDown={onKey}>
    <div className={`sheet${className ? ` ${className}` : ''}`} role="dialog" aria-modal="true" aria-label={title} ref={dialog} tabIndex={-1}>
      <header className="sheet-header">
        <strong>{title}</strong>
        {subtitle && <span>{subtitle}</span>}
      </header>
      <div className="sheet-body">{children}</div>
      <footer className="sheet-footer">{footer}</footer>
    </div>
  </div>, document.body);
}

export function Switch({checked, onChange, label, disabled}: {checked: boolean; onChange: (checked: boolean) => void; label: string; disabled?: boolean}) {
  return <label className="switch" title={label}>
    <input type="checkbox" role="switch" checked={checked} disabled={disabled} aria-label={label}
      onChange={e => onChange(e.target.checked)}/>
    <span aria-hidden/>
  </label>;
}

export function Segmented<T extends string>({value, options, onChange, label}: {
  value: T; options: {value: T; label: string}[]; onChange: (value: T) => void; label: string;
}) {
  // A radio group: one Tab stop (the selected option), arrow keys and Home/End move the choice.
  const index = Math.max(0, options.findIndex(o => o.value === value));
  const keyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const next = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? (index + 1) % options.length
      : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? (index - 1 + options.length) % options.length
      : e.key === 'Home' ? 0 : e.key === 'End' ? options.length - 1 : -1;
    if (next < 0) return;
    e.preventDefault();
    onChange(options[next].value);
    (e.currentTarget.children[next] as HTMLElement | undefined)?.focus();
  };
  return <div className="segmented" role="radiogroup" aria-label={label} onKeyDown={keyDown}>
    {options.map((o, i) => <button key={o.value} type="button" role="radio" aria-checked={o.value === value}
      tabIndex={i === index ? 0 : -1}
      className={o.value === value ? 'selected' : ''} onClick={() => onChange(o.value)}>{o.label}</button>)}
  </div>;
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Older web views: a hidden text area and the copy command.
    const area = document.createElement('textarea');
    area.value = text;
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand('copy');
    area.remove();
    return ok;
  }
}

export function CopyButton({text, label = t('Copy'), prominent}: {text: string; label?: string; prominent?: boolean}) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1600);
    return () => clearTimeout(timer);
  }, [copied]);
  return <button type="button" className={prominent ? 'button prominent' : 'button'}
    onClick={() => copyText(text).then(setCopied)} aria-live="polite">
    {copied ? <Check size={13} strokeWidth={2.4} aria-hidden/> : <Copy size={13} strokeWidth={2} aria-hidden/>}
    {copied ? t('Copied') : label}
  </button>;
}

export function CodeBlock({code, label}: {code: string; label: string}) {
  return <div className="code-block">
    <pre className="selectable" aria-label={label}><code>{code}</code></pre>
    <CopyButton text={code}/>
  </div>;
}

export type Tone = 'ok' | 'warn' | 'error' | 'off' | 'info';

export function Badge({tone, children, title}: {tone: Tone; children: React.ReactNode; title?: string}) {
  return <span className={`badge ${tone}`} title={title}><span className="dot" aria-hidden/>{children}</span>;
}

export function ErrorNote({children}: {children: React.ReactNode}) {
  return <p className="error-note" role="alert"><CircleAlert size={14} aria-hidden/>{children}</p>;
}

export function Spinner({label}: {label?: string}) {
  // A status, so a screen reader says something is loading; without a label it still says so.
  return <span className="spinner-label" role="status"><LoaderCircle size={14} className="spin" aria-hidden/>
    {label ?? <span className="sr-only">{t('Loading…')}</span>}</span>;
}

/** Placeholder rows while a slow list loads: the shape of what is coming, with a status for screen readers. */
export function SkeletonRows({count, label}: {count: number; label: string}) {
  return <div className="skeleton" role="status" aria-label={label}>
    <ul className="group-box skeleton-list" aria-hidden>
      {Array.from({length: count}, (_, i) => <li key={i} className="skeleton-row">
        <span className="skeleton-icon"/>
        <span className="skeleton-text"><span className="skeleton-bar" style={{width: `${38 + (i * 17) % 30}%`}}/>
          <span className="skeleton-bar short" style={{width: `${55 + (i * 23) % 35}%`}}/></span>
      </li>)}
    </ul>
    <p className="skeleton-label"><LoaderCircle size={13} className="spin" aria-hidden/>{label}</p>
  </div>;
}

/** A centered empty state with an icon, a title, a sentence and an optional action. */
export function Empty({icon, title, children, action}: {icon: React.ReactNode; title: string; children?: React.ReactNode; action?: React.ReactNode}) {
  return <div className="placeholder">
    <span className="placeholder-icon" aria-hidden>{icon}</span>
    <h2>{title}</h2>
    {children && <p>{children}</p>}
    {action}
  </div>;
}

/** Asks before something that cannot be undone. */
export function ConfirmSheet({title, message, confirmLabel, onConfirm, onClose, destructive = true}: {
  title: string; message: React.ReactNode; confirmLabel: string; onConfirm: () => Promise<void> | void; onClose: () => void;
  destructive?: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async () => {
    setBusy(true);
    try { await onConfirm(); onClose(); } catch (e) { setError(String(e)); setBusy(false); }
  };
  return <Sheet title={title} onClose={onClose} busy={busy} className="narrow" footer={<>
    <span className="spacer"/>
    <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
    <button type="button" className={destructive ? 'button destructive' : 'button prominent'} onClick={run} disabled={busy}>
      {busy ? <Spinner/> : null}{confirmLabel}
    </button>
  </>}>
    <div className="sheet-text">{message}</div>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

/** A labeled row in a sheet form. */
export function FormRow({label, htmlFor, hint, children}: {label: string; htmlFor?: string; hint?: React.ReactNode; children: React.ReactNode}) {
  const hintId = htmlFor && hint ? `${htmlFor}-hint` : undefined;
  // The hint describes the field, so a screen reader reads it with the field's label.
  useEffect(() => {
    const field = htmlFor ? document.getElementById(htmlFor) : null;
    if (!field) return;
    if (hintId) field.setAttribute('aria-describedby', hintId); else field.removeAttribute('aria-describedby');
  }, [htmlFor, hintId]);
  return <div className="form-row">
    <label className="form-label" htmlFor={htmlFor}>{label}</label>
    <div className="form-field">
      {children}
      {hint && <small className="form-hint" id={hintId}>{hint}</small>}
    </div>
  </div>;
}
