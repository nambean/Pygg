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
