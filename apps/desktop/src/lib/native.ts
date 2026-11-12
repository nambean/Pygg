/**
 * Typed calls into the Rust core. Every Tauri command the interface uses goes through this file, so the
 * contract between the two sides is in one place. In a browser preview (pnpm dev) there is no core:
 * calls resolve to "not available" instead of failing.
 */
import { invoke, isTauri } from '@tauri-apps/api/core';
import { isLanguage, type LanguageChoice } from './i18n';

export const runningInDesktop = isTauri();

export type GatewayState = 'running' | 'stopped' | 'not_built';
export type HealthStatus = { app: string; version: string; gateway: GatewayState };

export async function nativeHealth(): Promise<HealthStatus | null> {
  if (!runningInDesktop) return null;
  return invoke<HealthStatus>('health_check');
}

/** The green window button's rectangle in physical pixels from the window's top left (src-tauri/src/snap.rs). */
export type SnapArea = { x: number; y: number; width: number; height: number };

/**
 * Tells the core where the green window button is, so Windows 11 offers snap layouts over it; `null` removes the
 * overlay. Windows only; elsewhere, and in a browser preview, it does nothing.
 */
export function setSnapArea(area: SnapArea | null): Promise<void> {
  if (!runningInDesktop) return Promise.resolve();
  return invoke('window_snap_area', { area });
}

/** The core's event when the pointer enters (`true`) or leaves (`false`) the snap overlay. */
export const SNAP_HOVER_EVENT = 'window-snap-hover';

// Language is kept per device. Until the core stores settings (a later step), the web view's storage
// holds it; storage can be unavailable, so every access is guarded.
const LANGUAGE_KEY = 'smarttaskrouter.language';

export function savedLanguage(): LanguageChoice {
  try {
    const value = localStorage.getItem(LANGUAGE_KEY);
    return value === 'system' || isLanguage(value) ? value : 'system';
  } catch {
    return 'system';
  }
}

export function saveLanguage(choice: LanguageChoice) {
  try { localStorage.setItem(LANGUAGE_KEY, choice); } catch { /* not kept; the choice still applies now */ }
}
