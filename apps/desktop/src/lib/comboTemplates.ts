// Combo templates: rules over the user's connected models, resolved into an ordered list when the combo is created
// (Combos, SCREENS §2.2). A combo stores a list, not rules: models connected later are not added by themselves, and the
// preview says so. Unknown facts never satisfy a requirement; nothing is added silently to fill a stage.

import type { Billing, Connection, Model, Target, Tier } from './api';

export type Protocol = 'openai' | 'anthropic';
/** allowUntiered: free and local models without a tier may take part, last in their stage (the user's explicit choice). */
export type TemplateOptions = {tools: boolean; minTier: Tier; protocol: Protocol; allowUntiered?: boolean};

export type TemplateId = 'free-first' | 'subscription-only' | 'plans-first' | 'strong-backup' | 'no-metered' | 'local-only' | 'long-context' | 'cheapest';

export type Template = {
  id: TemplateId; name: string; purpose: string; defaultName: string;
  /** The billing stages, in order; a model's stage decides its place before anything else. */
  stages: Billing[];
  /** Least context a member must be known to have. */
  minContext?: number;
  /** Only connections served from this computer. */
  loopback?: boolean;
  /** Its name promises free, local or plan models first: it is not created with paid models alone, and its paid stage
   * keeps only the cheapest backups. */
  unpaidFirst?: boolean;
  /** The combo's strategy: `cheapest` re-orders the members for each request by its estimated cost. */
  strategy?: 'cheapest';
  /** How members are ordered within a stage. */
  order: 'tier-then-cost' | 'cost-then-tier' | 'strongest-then-cost';
};

export const TEMPLATES: Template[] = [
  {id: 'free-first', name: 'Free first', defaultName: 'free-first', stages: ['local', 'free', 'paid'], order: 'tier-then-cost', unpaidFirst: true,
    purpose: 'Try models on this computer and free tiers before paid ones; paid models need a known price.'},
  {id: 'plans-first', name: 'Use my plans first', defaultName: 'plans-first', stages: ['subscription', 'local', 'free', 'paid'],
    order: 'tier-then-cost', unpaidFirst: true, purpose: 'Spend the quota you already pay for, then free, then paid as a backup.'},
  {id: 'subscription-only', name: 'Subscription only', defaultName: 'subscription', stages: ['subscription'], order: 'tier-then-cost',
    purpose: 'Only your plans. When their quota is out, the request fails instead of spilling into paid usage.'},
  {id: 'strong-backup', name: 'Strong coding, cheap backup', defaultName: 'coding', stages: ['subscription', 'local', 'free', 'paid'],
    minContext: 128_000, order: 'strongest-then-cost',
    purpose: 'The strongest model first, then affordable backups that can still do the work (128k context or more).'},
  {id: 'no-metered', name: 'No metered spend', defaultName: 'no-spend', stages: ['local', 'free'], order: 'tier-then-cost',
    purpose: 'Free and local models only: no per-token charge and no plan quota. Local runs still use your electricity.'},
  {id: 'local-only', name: 'Local only', defaultName: 'local', stages: ['local'], loopback: true, order: 'tier-then-cost',
    purpose: 'Keep code on this computer: models served from 127.0.0.1 or localhost only.'},
  {id: 'long-context', name: 'Long-context coding', defaultName: 'long-context', stages: ['local', 'free', 'subscription', 'paid'],
    minContext: 200_000, order: 'cost-then-tier', purpose: 'Every member takes large requests (200k context or more), cheapest first.'},
  {id: 'cheapest', name: 'Cheapest for each request', defaultName: 'cheapest', stages: ['local', 'free', 'subscription', 'paid'],
    order: 'cost-then-tier', strategy: 'cheapest',
    purpose: 'Qualifying models re-ordered for every request by its estimated cost: free, local and plan first, unknown prices last.'},
];

/** Reference workload for ordering by price: 2,000 tokens in, 1,000 out. A static order cannot be cheapest for every request. */
export const REFERENCE = {input: 2_000, output: 1_000};

const TIER_RANK: Record<Tier, number> = {T0: 0, T1: 1, T2: 2, T3: 3, T4: 4};

export type Resolved = {
  members: {target: Target; why: string; referenceMicroUsd: number | null}[];
  excluded: {target: Target; why: string}[];
  /** Stages of the template that matched nothing. */
  missingStages: Billing[];
  /** Members left out because a combo is kept short. */
  capped: number;
  /** The template leads with free, local or plan stages, and none of them matched: only paid models are left, so
   * creating it would contradict its name. */
  paidOnly: boolean;
  /** Why the models of each non-paid stage were left out, most common first, for the advice shown. */
  stageReasons: {stage: Billing; total: number; reasons: [string, number][]}[];
};

const MAX_MEMBERS = 6;
/** A paid stage after free ones is a backup: its cheapest two are enough. */
const MAX_PAID_BACKUPS = 2;

function referenceCost(m: Model): number | null {
  if (!m.price) return null;
  return Math.round((m.price.inputMicroPerMtok * REFERENCE.input + m.price.outputMicroPerMtok * REFERENCE.output) / 1_000_000);
}

function isLoopback(c: Connection): boolean {
  return /^https?:\/\/(127\.0\.0\.1|localhost|\[::1\])(:|\/|$)/i.test(c.endpoint ?? '');
}

export function resolve(template: Template, options: TemplateOptions, models: Model[], connections: Connection[]): Resolved {
  const excluded: Resolved['excluded'] = [];
  const candidates: {model: Model; stage: number; cost: number | null; tier: number}[] = [];
  const reasonsByStage = new Map<Billing, Map<string, number>>();
  for (const m of models) {
    const target = {providerId: m.providerId, modelId: m.modelId};
    const c = connections.find(x => x.id === m.providerId);
    const no = (why: string) => {
      excluded.push({target, why});
      if (template.stages.includes(m.billing) && m.billing !== 'paid') {
        const counts = reasonsByStage.get(m.billing) ?? new Map<string, number>();
        const kind = why.startsWith('tier ') && why.includes('below') ? `tier below ${options.minTier}` : why.replace(/ \d+k, below \d+k$/, ' too small');
        counts.set(kind, (counts.get(kind) ?? 0) + 1);
        reasonsByStage.set(m.billing, counts);
      }
    };
    if (!c || !c.enabled) { no('its connection is off'); continue; }
    if (!m.enabled) { no('the model is off'); continue; }
    const stage = template.stages.indexOf(m.billing);
    if (stage < 0) { no(`billed as ${m.billing}, which this template leaves out`); continue; }
    if (template.loopback && !isLoopback(c)) { no('not served from this computer'); continue; }
    if (options.protocol === 'openai' && c.wire === 'anthropic') { no('Anthropic-format only; OpenAI-format clients can’t use it'); continue; }
    const caps = m.capabilities;
    if (options.tools && caps.supportsTools !== true) { no(caps.supportsTools === false ? 'does not take tools' : 'tool support unknown'); continue; }
    const untieredOk = caps.tier === null && options.allowUntiered && (m.billing === 'free' || m.billing === 'local');
    if (caps.tier === null && !untieredOk) { no('tier not set'); continue; }
    if (caps.tier !== null && TIER_RANK[caps.tier] < TIER_RANK[options.minTier]) { no(`tier ${caps.tier}, below ${options.minTier}`); continue; }
    if (template.minContext && (caps.contextTokens ?? 0) < template.minContext) {
      no(caps.contextTokens === null ? 'context size unknown' : `context ${Math.round(caps.contextTokens / 1000)}k, below ${template.minContext / 1000}k`); continue;
    }
    const cost = referenceCost(m);
    if (m.billing === 'paid' && cost === null) { no('paid without a known price'); continue; }
    candidates.push({model: m, stage, cost, tier: caps.tier === null ? -1 : TIER_RANK[caps.tier]});
  }
  const byCost = (a: number | null, b: number | null) => (a ?? 0) - (b ?? 0);
  const stable = (a: Model, b: Model) => `${a.providerId}/${a.modelId}`.localeCompare(`${b.providerId}/${b.modelId}`);
  let ordered: typeof candidates;
  if (template.order === 'strongest-then-cost') {
    // The strongest model first wherever it is billed; the rest by stage, then cost.
    const best = [...candidates].sort((a, b) => b.tier - a.tier || byCost(a.cost, b.cost) || stable(a.model, b.model))[0];
    const rest = candidates.filter(c => c !== best).sort((a, b) => a.stage - b.stage || byCost(a.cost, b.cost) || stable(a.model, b.model));
    ordered = best ? [best, ...rest] : [];
  } else {
    // Free, local and plan stages: the strongest first. A paid stage is a backup: the cheapest first, whatever the order.
    ordered = [...candidates].sort((a, b) => a.stage - b.stage || (template.order === 'tier-then-cost' && a.model.billing !== 'paid'
      ? b.tier - a.tier || byCost(a.cost, b.cost) : byCost(a.cost, b.cost) || b.tier - a.tier) || stable(a.model, b.model));
  }
  // After free or plan stages, paid models are only backups: keep the cheapest two.
  const leadsFree = !!template.unpaidFirst;
  let paidKept = 0;
  const trimmed = leadsFree ? ordered.filter(c => c.model.billing !== 'paid' || paidKept++ < MAX_PAID_BACKUPS) : ordered;
  const kept = trimmed.slice(0, MAX_MEMBERS);
  const members = kept.map((c, i) => ({
    target: {providerId: c.model.providerId, modelId: c.model.modelId},
    referenceMicroUsd: c.cost,
    why: [template.order === 'strongest-then-cost' && i === 0 ? 'strongest that qualifies' : null, c.model.billing,
      c.model.capabilities.tier ?? 'tier not set', c.cost !== null && c.model.billing === 'paid' ? 'priced' : null].filter(Boolean).join(' · '),
  }));
  const present = new Set(kept.map(c => c.model.billing));
  const paidOnly = leadsFree && kept.length > 0 && kept.every(c => c.model.billing === 'paid');
  const stageReasons = template.stages.filter(s => s !== 'paid' && !present.has(s)).map(stage => {
    const counts = [...(reasonsByStage.get(stage) ?? new Map<string, number>()).entries()].sort((a, b) => b[1] - a[1]);
    return {stage, total: counts.reduce((n, [, k]) => n + k, 0), reasons: counts.slice(0, 4)};
  });
  return {members, excluded, missingStages: template.stages.filter(s => !present.has(s)),
    capped: Math.max(0, trimmed.length - MAX_MEMBERS), paidOnly, stageReasons};
}
