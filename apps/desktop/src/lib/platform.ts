/** The system the app runs on, from the web view's user agent. Used for layout and wording only. */
export const PLATFORM: 'windows' | 'macos' | 'linux' =
  /Mac/i.test(navigator.userAgent) ? 'macos' : /Windows/i.test(navigator.userAgent) ? 'windows' : 'linux';

export const isMac = PLATFORM === 'macos';

/** The label of the primary modifier key in shortcuts. */
export const MOD = isMac ? '⌘' : 'Ctrl+';
