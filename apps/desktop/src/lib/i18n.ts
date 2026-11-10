/**
 * Interface language, as in DownFetch. Texts are written in English in the code and wrapped in `t()`;
 * the English text is the key, so a missing translation just shows English. Translations live in
 * src/locales/<language>/ and load only when chosen.
 *
 *   t('New Task')
 *   t('{n} running', {n: 2})                 // placeholders in braces
 *   plural(n, '{n} task', '{n} tasks')       // one/other forms
 */
import { createContext, useContext } from 'react';

export type Language = 'en' | 'vi';
export type LanguageChoice = 'system' | Language;

/** Every interface language: its own name (shown untranslated in the picker) and the locale for dates and
 *  numbers. */
export const LANGUAGES: {code: Language; name: string; locale: string}[] = [
  {code: 'en', name: 'English', locale: 'en-US'},
  {code: 'vi', name: 'Tiếng Việt', locale: 'vi-VN'},
];
const CODES = new Set<string>(LANGUAGES.map(l => l.code));
export const isLanguage = (value: unknown): value is Language => typeof value === 'string' && CODES.has(value);

const LOADERS: Record<Exclude<Language, 'en'>, () => Promise<Record<string, string>>> = {
  vi: () => import('../locales/vi').then(m => m.vi),
};

let current: Language = 'en';
let table: Record<string, string> = {};
const tables = new Map<Language, Record<string, string>>();

/** Loads a language's texts; call (and wait) before applyLanguage, so nothing flashes in English. */
export async function loadLanguage(next: Language): Promise<void> {
  if (next === 'en' || tables.has(next)) return;
  tables.set(next, await LOADERS[next]());
}

/** The language, for memoised components: a context change reaches them even when their props are unchanged. */
export const LanguageContext = createContext<Language>('en');
export const useLanguage = () => useContext(LanguageContext);

/** The first of the system's (WebView's) preferred languages that the app speaks, else English. */
export function systemLanguage(): Language {
  for (const tag of navigator.languages?.length ? navigator.languages : [navigator.language || '']) {
    const code = String(tag).toLowerCase().split('-')[0];
    if (isLanguage(code)) return code;
  }
  return 'en';
}

export function resolve(choice: LanguageChoice): Language {
  return choice === 'system' ? systemLanguage() : choice;
}

export function language(): Language { return current; }

/** Locale for dates, times and numbers: the interface language, not the system's. */
export function locale(): string {
  if (current === 'en') return /^en\b/i.test(navigator.language || '') ? navigator.language : 'en-US';
  return LANGUAGES.find(l => l.code === current)!.locale;
}

/** Sets the language used by `t()`; the caller re-renders the interface. */
export function applyLanguage(next: Language) {
  current = next;
  table = tables.get(next) ?? {};
  document.documentElement.lang = next;
}

export function t(text: string, vars?: Record<string, string | number>): string {
  const out = table[text] ?? text;
  return vars ? out.replace(/\{(\w+)\}/g, (all, key: string) => key in vars ? String(vars[key]) : all) : out;
}

/** `one` when n is 1, else `other`; both may use {n}. */
export function plural(n: number, one: string, other: string): string {
  return t(n === 1 ? one : other, {n});
}
