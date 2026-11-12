/**
 * The app's place in the system (src-tauri/src/shell.rs): the tray, notifications about tasks, and their
 * preferences. In a browser preview there is no tray, so these do nothing and preferences are not offered.
 */
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import { runningInDesktop } from './native';

export type Preferences = { keepInTray: boolean; notifyTasks: boolean; openAtSignIn: boolean };

export function preferences(): Promise<Preferences> {
  return invoke('app_preferences');
}

export function setPreference(key: keyof Preferences, value: boolean): Promise<Preferences> {
  return invoke('app_set_preference', {key, value});
}

/** Sends the tray's and the notifications' words in the current language. Never names a task (ADR-010). */
export function sendShellStrings() {
  if (!runningInDesktop) return;
  const strings: Record<string, string> = {
    'tray.open': t('Open Pygg'),
    'tray.quit': t('Quit Pygg'),
    'tray.hint.title': t('Pygg is still running'),
    'tray.hint.body': t('The gateway and your tasks keep running. Open Pygg or quit it from the tray icon.'),
    'notify.approval.title': t('A task is waiting for your approval'),
    'notify.approval.body': t('Open Pygg to approve or decline its paid step.'),
    'notify.passed.title': t('A task is ready'),
    'notify.passed.body': t('Its checks passed. Open Pygg to keep or discard the result.'),
    'notify.review.title': t('A task needs your review'),
    'notify.review.body': t('It finished, but no check confirmed it. Open Pygg to review the changes.'),
    'notify.failed.title': t('A task did not finish'),
    'notify.failed.body': t('Open Pygg to see why each step stopped.'),
  };
  invoke('ui_strings', {strings}).catch(() => undefined);
}

/** Calls `listener` whenever a task's runner stops (a task from the IDE included). */
export function onTasksChanged(listener: () => void): () => void {
  if (!runningInDesktop) return () => undefined;
  let stop: (() => void) | undefined;
  let gone = false;
  import('@tauri-apps/api/event').then(m => m.listen('tasks-changed', listener)).then(unlisten => {
    if (gone) unlisten(); else stop = unlisten;
  }).catch(() => undefined);
  return () => { gone = true; stop?.(); };
}
