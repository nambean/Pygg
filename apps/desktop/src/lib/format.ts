import { locale, t } from './i18n';
import type { QuotaWindow } from './api';
import type { TaskCost } from './tasks';

/** "$0.08", "$3.42", "$0.004": two decimals, three for sub-cent amounts. Never used for unknown values. */
export function usd(value: number): string {
  const digits = value > 0 && value < 0.01 ? 3 : 2;
  return new Intl.NumberFormat(locale(), {style: 'currency', currency: 'USD', minimumFractionDigits: digits,
    maximumFractionDigits: digits}).format(value);
}

/** "8 min ago", "2 hr ago", in the interface language. */
export function ago(timestamp: number, now = Date.now()): string {
  const seconds = Math.round((timestamp - now) / 1000);
  const format = new Intl.RelativeTimeFormat(locale(), {numeric: 'auto', style: 'short'});
  if (Math.abs(seconds) < 60) return format.format(0, 'minute');
  const minutes = Math.round(seconds / 60);
  if (Math.abs(minutes) < 60) return format.format(minutes, 'minute');
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return format.format(hours, 'hour');
  return format.format(Math.round(hours / 24), 'day');
}

/** "1m 12s", "4m 08s", "12s". */
export function elapsed(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return m ? `${m}m ${String(s).padStart(2, '0')}s` : `${s}s`;
}

/**
 * A task's cost so far, honest about what is known (SCREENS.md §3.3): a local-only task says so instead of
 * "$0.00"; unknown calls are named next to the known amount, never added to it.
 */
export function taskCost(cost: TaskCost): string {
  const parts: string[] = [];
  if (cost.localOnly && cost.actualUsd === 0 && cost.estimatedUsd === 0) parts.push(t('API {zero} · local', {zero: usd(0)}));
  else if (cost.actualUsd > 0 || cost.estimatedUsd > 0) {
    parts.push(cost.estimatedUsd > 0
      ? t('{actual} + ≈{estimated} est.', {actual: usd(cost.actualUsd), estimated: usd(cost.estimatedUsd)})
      : usd(cost.actualUsd));
  }
  if (cost.unknownCalls > 0) {
    parts.push(cost.unknownCalls === 1 ? t('1 call unknown') : t('{n} calls unknown', {n: cost.unknownCalls}));
  }
  return parts.length ? parts.join(' · ') : usd(0);
}

/** One stated limit in words: "requests: 12 of 50 left, resets 14:00". */
export function windowText(w: QuotaWindow): string {
  const names: Record<string, string> = {requests: t('requests'), tokens: t('tokens'), input_tokens: t('input tokens'),
    output_tokens: t('output tokens'), free_requests_per_day: t('free requests today'), retry: t('asked to wait')};
  const name = names[w.kind] ?? w.kind;
  const left = w.remaining === null ? '' : w.limit !== null
    ? t('{left} of {limit} left', {left: w.remaining.toLocaleString(), limit: w.limit.toLocaleString()})
    : t('{left} left', {left: w.remaining.toLocaleString()});
  const reset = w.resetsAt && w.resetsAt > Date.now()
    ? t('resets {time}', {time: new Date(w.resetsAt).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit'})}) : '';
  return [name + (left ? `: ${left}` : ''), reset].filter(Boolean).join(', ');
}
