# Roadmap

> **Status:** Authoritative · delivery order and release gates · reviewed 2026-11-16

What comes next, in order, and what must be true before a public release. What is already built, and how it was verified, is in [Implementation Status](IMPLEMENTATION-STATUS.md). Why, in [DECISIONS](DECISIONS.md). Nothing here is a claim that code exists.

**North star:** a local gateway that lets developers and everyday users reach the providers, plans and free tiers they already have through the tools they already use, and lowers the total cost per successful answer, not the nominal token price. The first measured audience is API-heavy developers using coding agents, because cost per solved task can be benchmarked. The app hosts no chat of its own (ADR-028).

## Next, in order

1. **Close the live-evidence gaps.** Each needs a key, an account or the owner's time; paid traffic stays inside the benchmark budget (ADR-028).
   - Successful API requests to OpenAI, Groq, DeepSeek and Mistral; DeepSeek's thinking mode with tools across the Anthropic translation.
   - OpenRouter's `cache_control` on translated requests, measured.
   - An Anthropic-compatible endpoint other than Anthropic's own.
   - Gemini's quota answer (429 `RESOURCE_EXHAUSTED`) as a fixture; the 429 hold and the prefill fallback in a live `auto/cheap` run.
   - Claude Code in an interactive terminal, through the launcher, answered by a real provider.
   - `pygg run codex` and `pygg run opencode` against the installed tools; `pygg setup` against a real tool's configuration.
   - Key rings, cheapest-first combos and combo tests with real providers.
   - Estimated savings and the token mix read against a real ledger.
2. **The native window, clicked through.** Every screen in the installed app, not only the browser preview. The D12 checks (ADR-012): scaling, snap layouts above 100%, Narrator and NVDA, contrast themes.
3. **Install on a clean Windows machine.** NSIS install, first run, gateway, a coding tool set up, uninstall leaving nothing behind.
4. **Token and cost work, in the order ADR-036 sets.** The token mix, launch sessions, the way home after a fallback, why each answer missed the cache, routing diagnostics, the always-on context of each agent's global files (Skills & Prompts, ADR-039), the cache marker for Anthropic's models through OpenRouter, the session passed on to OpenRouter and OpenAI, and the meter for each session's tools and system prompt exist, proved on the fake provider. Next:
   - **One short live check** against a provider that reports its cache, read against the ledger: the way home, the cache-miss causes, the tools-and-system-prompt meter, and OpenRouter's marker and `x-session-id` on an `anthropic/…` model. Needs the owner's go-ahead for the spend.
   - **Clearing old tool results** (Anthropic's context editing) in shadow first, for OpenAI-format clients only and never for Claude Code's native requests; opt-in only if the shadow shows a net saving after the cache writes it causes.
   Anything that changes a request is opt-in, reversible and checked for cache parity first. Never rewrite an external agent's history, inject instructions or split tasks into sub-agents to save tokens.
5. **Codex's model catalog** for the models Pygg lists, so Codex gets `apply_patch` and real context sizes without a hand-written catalog.
6. **Other platforms.** The macOS npm packages (published, ADR-037) run on a real Mac; the desktop app on macOS and Linux (ADR-001).
7. **Housekeeping.** Demo data that replays recorded snapshots instead of copying the core's rules.
8. **The Vietnamese translation**, last, once the interface's English text is final.

Smart tasks are **on hold** until they are re-evaluated. No new work on tasks, workspaces or agents runs until then.

## Release gates

A public release needs all of these:

- **Parity.** For each provider path Pygg claims, the same model direct and through Pygg, over at least two turns with prompt caching, tools and streaming: cache savings and the billed amount do not regress without a stated reason. Paths not measured are named as not claimed.
- **Honesty.** No percentage saving and no promise of kept quality without a reproducible benchmark with confidence bounds. `auto` makes no savings claim of its own until a benchmark supports one. Unknown cost stays Unknown.
- **Security.** No key or account switch the user did not confirm, no provider whose data-use terms the client refused, no limit overrun through concurrency, loopback only. [SECURITY](SECURITY.md) matches the build.
- **Errors.** Real 403, 429, quota and timeout answers from the main providers run through the error policy as fixtures.
- **Install.** The clean-machine install and uninstall pass; the installer is signed (not during development).
- **Interface.** Every screen clicked through in the native window; the D12 checks pass; `pnpm ui-check` passes; the Vietnamese strings are complete.
- **Web sessions.** The guard stays on and cannot be turned off; the account-risk warning is ticked per connection (ADR-035).

## Later, after data or a request

Unordered. Each moves up only when data or a user request justifies it.

- The Setup Assistant ([SCREENS](design/SCREENS.md)).
- Quota tracking for subscriptions, if a provider or agent ever states it.
- Billing-tier views with per account-model entitlements (ADR-020).
- Minimum-tier rules by task category, and a measured ranking above a minimum sample (ADR-006).
- Idempotency keys; provider-verified organization and quota scope; reconciliation against a provider's own usage. Held until a provider adapter can fill them.
- Fallback after headers but before the first token. Held.
- Running the benchmark from the app.
- Client verification beyond Claude Code, Codex and OpenCode (Aider, Cursor).
- A signed updater.
- More `auto` aliases (`auto/fast`, `auto/coding`, `auto/local`, `auto/quality`): not planned until the benchmark shows `auto` and `auto/cheap` are not enough.
- Semantic caching and advanced compaction: only if a test beats a simpler baseline.
- Skills from a catalog or GitHub, each pinned to a reviewed commit with its files checked and scripts held back (ADR-039).

Withdrawn: Auto Chat and app-owned memory (ADR-028); bridging subscription sign-ins or another product's OAuth (ADR-026).

## Product rules

- **Billing tier, sign-in method and capability tier are different things.** Free, Subscription, API Paid, Local and Unknown are per account-model, with a source.
- **No model switch inside an agent's attempt, tool call or partial stream.** Pinned clients keep their prompt and cache.
- **Native passthrough over translation.** A translated route must pass the streaming, tools, images, cache and citation contract tests and name what it drops.
- **No account cycling to evade bans, rate limits or provider terms.** Quota groups are hard limits; key rings and pools need accounts confirmed as separate.
- **Free plan or subscription tokens are not counted as money saved.** Money, tokens, local running cost and human rework are kept apart.
- **Metrics never invent precision.** An unknown price or usage is Unknown; a saving needs a valid baseline.

## Owner decisions in force

- MIT licence (ADR-028).
- Benchmark paid traffic: $20 in total (ADR-028). More needs the owner's approval.
- No Auto Chat in the app (ADR-028).
- No code signing during development; a certificate is bought for release.
