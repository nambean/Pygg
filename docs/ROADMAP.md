# Roadmap

> **Status:** Authoritative · delivery order and release gates · reviewed 2026-11-21

The order of work and what a public release needs. Built and verified: [Implementation Status](IMPLEMENTATION-STATUS.md); why: [DECISIONS](DECISIONS.md). Nothing here claims code exists.

## Next, in order

| # | Item | Status |
| --- | --- | --- |
| 1 | Close the live-evidence gaps in [Not verified yet](IMPLEMENTATION-STATUS.md#not-verified-yet); each needs a key, an account or the owner's time; paid traffic within ADR-028's budget; each prerelease is the owner's call, npm follows ([PUBLISHING](../packages/npm/PUBLISHING.md)) | in progress |
| 2 | The native window clicked through: every screen in the installed app; the D12 checks (ADR-012): scaling, snap layouts above 100%, Narrator and NVDA, contrast themes | next |
| 3 | Clean Windows machine: NSIS install, first run, gateway, a coding tool set up, uninstall leaves nothing | next |
| 4 | Token and cost work, in ADR-036's order ([below](#token-and-cost-work)) | in progress |
| 5 | Codex's catalog measured: Codex with and without it on the same benchmark cases (passes, cost); keep `third_party/codex/prompt.md` in step with the tested Codex version | next |
| 6 | Other platforms: the macOS npm packages (published, ADR-037) run on a real Mac; the desktop app on macOS and Linux (ADR-001) | next |
| 7 | Housekeeping: demo data replays recorded snapshots instead of copying the core's rules | next |
| 8 | The Vietnamese translation, once the English text is final | next (last) |
| 9 | [A larger benchmark](#a-larger-benchmark), after 1–8 | later |

### Token and cost work

| Item | Status |
| --- | --- |
| Cache marker on OpenRouter, cache-miss causes and the meter, checked live ([report](../benchmark/reports/2026-11-16-openrouter-cache-live.md)) | done |
| The way home after a fallback, live: needs a real failure; OpenAI's `prompt_cache_key` needs an OpenAI key | next |
| Read the clearing shadow (Usage → Clearing Old Tool Results) over long Codex sessions; only if its net stays above zero, opt-in clearing with an answer-quality check, as Anthropic's `context_management` edit, never Pygg rewriting history | next |
| Read session receipts on real work: launches with Git changes (schema v38) and Claude Code request classes (v39) are recorded; after a few weeks of the owner's use, read `pygg launches --by-repo` and by tool and model, as observations; no productivity claim | in progress |
| Read the cache-lifetime shadow (Usage → Cache Lifetime); if one way stays above zero for a client, offer it per client key, opt-in: a one-hour `ttl` on Pygg's marker, or a keep-alive | next |

Anything that changes a request is opt-in, reversible and checked for cache parity first. Never rewrite an external agent's history, inject instructions or split tasks into sub-agents to save tokens.

### A larger benchmark

Published runs are the maintainer's own (fixture cases in `benchmark/cases`, six tasks from one private repository): too few for a headline. The rerun:

- **Cases:** large cross-module refactors, more database migrations and languages, tasks cut from real repositories, beside the 41 cases in `benchmark/cases` (five harder ones, a schema migration and two Python cases among them).
- **Models:** every provider and model in the owner's store that can serve a coding agent (API keys, coding plans, local). No web sessions: the guard refuses benchmark-sized prompts, and a long-prompt run got an account suspended (ADR-035).
- **Clients:** Claude Code, Codex and OpenCode, at least three repeats per case and arm; `auto`, `auto/cheap` and the role aliases beside pinned models.
- **Report per task category:** first-attempt and all-repeat pass rate, cost per solved task, time, confidence intervals, and every arm where the cheaper route lost.

Its results replace the README's figures. Paid traffic stays within [ADR-028](DECISIONS.md#adr-028--mit-licence-a-20-benchmark-budget-and-no-auto-chat)'s budget (which also says when Claude models join); more needs the owner's approval. Contributed runs ([CONTRIBUTING](../CONTRIBUTING.md#benchmark-runs)) are added beside it.

## Release gates

A public release needs all of these:

- **Parity.** Per claimed provider path, the same model direct and through Pygg, two or more turns with prompt caching, tools and streaming: no unexplained regression in cache savings or billed amount. Unmeasured paths are named as not claimed.
- **Honesty.** No percentage saving or kept-quality promise without a reproducible benchmark with confidence bounds; `auto` claims no saving until one supports it; unknown cost stays Unknown.
- **Security.** No unconfirmed key or account switch, no provider whose data-use terms the client refused, no limit overrun through concurrency, loopback only; [SECURITY](SECURITY.md) matches the build.
- **Errors.** Real 403, 429, quota and timeout answers from the main providers run through the error policy as fixtures.
- **Install.** Clean-machine install and uninstall pass; installer signed (no signing during development; a certificate is bought for release).
- **Interface.** Every screen clicked through in the native window; D12 checks pass; `pnpm ui-check` passes; Vietnamese strings complete.
- **Web sessions.** The guard stays on and cannot be turned off; the account-risk warning ticked per connection (ADR-035).

## Later, after data or a request

Unordered; each moves up only when data or a user request justifies it.

- Quota tracking for subscriptions, if a provider or agent ever states it.
- The `image` role, once the gateway has an images path (ADR-041).
- A root `SECURITY.md` for GitHub's Security tab; `CHANGELOG.md` from the first tagged release.
- Billing-tier views with per account-model entitlements (ADR-020).
- Minimum-tier rules by task category; a measured ranking above a minimum sample (ADR-006).
- Idempotency keys, provider-verified organization and quota scope, reconciliation against a provider's own usage: held until a provider adapter can fill them.
- Fallback after headers but before the first token: held.
- Running the benchmark from the app.
- Client verification beyond Claude Code, Codex and OpenCode (Aider, Cursor).
- A signed updater.
- `auto/fast`, `auto/local`, `auto/quality`: only if the benchmark shows `auto` and `auto/cheap` are not enough. (The role aliases are built, ADR-041, and measured before any claim.)
- Semantic caching and advanced compaction: only if a test beats a simpler baseline.
- Skills from a catalog or GitHub, each pinned to a reviewed commit, files checked, scripts held back (ADR-039).

Withdrawn: Auto Chat and app-owned memory (ADR-028); bridging subscription sign-ins or another product's OAuth (ADR-026).

## Product rules

- Billing tier, sign-in method and capability tier are different things; Free, Subscription, API Paid, Local and Unknown are per account-model, with a source.
- No model switch inside an agent's attempt, tool call or partial stream; pinned clients keep their prompt and cache.
- Native passthrough over translation; a translated route must pass the streaming, tools, images, cache and citation contract tests and name what it drops.
- No account cycling to evade bans, rate limits or provider terms; quota groups are hard limits; key rings and pools need accounts confirmed as separate.
- Free-plan or subscription tokens are not money saved; money, tokens, local running cost and human rework are kept apart.
- Metrics never invent precision: unknown price or usage is Unknown; a saving needs a valid baseline.
