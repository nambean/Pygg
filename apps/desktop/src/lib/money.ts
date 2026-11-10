/**
 * Money and token figures for the gateway screens (SCREENS.md §3.3). Amounts are integer micro-USD from
 * the core. An unknown cost is never formatted as a number: callers show the word "Unknown" instead.
 */
import { locale, t } from './i18n';
import type { Billing, CostStatus, Price } from './api';

/** "$12.48", "$0.0031", "<$0.0001", "$0.00" (a known zero). */
export function microUsd(micro: number): string {
  const usd = micro / 1_000_000;
  if (micro > 0 && usd < 0.0001) return `<${format(0.0001, 4)}`;
  const digits = usd === 0 || usd >= 0.01 ? 2 : 4;
  return format(usd, digits);
}

function format(usd: number, digits: number): string {
  return new Intl.NumberFormat(locale(), {style: 'currency', currency: 'USD', minimumFractionDigits: digits,
    maximumFractionDigits: digits}).format(usd);
}

/** A ledger row's cost: "$0.0031", "≈$0.0031" or "Unknown". */
export function costText(micro: number | null, status: CostStatus): string {
  if (status === 'unknown' || micro === null) return t('Unknown');
  return status === 'estimated' ? `≈${microUsd(micro)}` : microUsd(micro);
}

/** "$0.10 / $0.40 per 1M tokens". */
export function priceText(price: Price): string {
  const per = (micro: number) => format(micro / 1_000_000, micro % 10_000 === 0 ? 2 : 4);
  return t('{input} in · {output} out per 1M', {input: per(price.inputMicroPerMtok), output: per(price.outputMicroPerMtok)});
}

/** "1.2k", "34k", "1.4M". */
export function tokens(n: number): string {
  return new Intl.NumberFormat(locale(), {notation: 'compact', maximumFractionDigits: 1}).format(n);
}

export function billingLabel(billing: Billing): string {
  return {free: t('Free'), local: t('Local'), subscription: t('Subscription'), paid: t('Paid'), unknown: t('Unknown billing')}[billing];
}

/** Translates the core's cost provenance (an English phrase) for display. */
export function sourceText(source: string): string {
  const status = /^rejected by the provider \(HTTP (\d+)\)$/.exec(source);
  if (status) return t('Rejected by the provider (HTTP {status}), not billed', {status: status[1]});
  const known: Record<string, string> = {
    'provider response': t('Reported by the provider'),
    'price: user': t('Your price × reported tokens'),
    'connection marked free': t('Connection marked free'),
    'local model': t('Local model'),
    'no price recorded for this model': t('No price recorded for this model'),
    'the provider reported no usage': t('The provider reported no usage'),
    'subscription: counts against a quota, not money': t('Subscription: counts against a quota, not money'),
    'the request failed after reaching the provider': t('Failed after reaching the provider; billing unknown'),
    'the response was cut short before usage was reported': t('Cut short before usage was reported'),
    'not sent': t('Not sent'),
    'could not connect': t('Could not connect; not sent'),
  };
  return known[source] ?? source;
}

/** Parses "1.25" or "1,25" as dollars; null for empty, NaN for anything else. */
export function parseUsd(text: string): number | null {
  const trimmed = text.trim().replace(/^\$/, '').replace(',', '.');
  if (!trimmed) return null;
  return /^\d+(\.\d+)?$/.test(trimmed) ? Number(trimmed) : NaN;
}
