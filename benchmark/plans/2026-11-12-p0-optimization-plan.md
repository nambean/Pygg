# P0 optimization benchmark: plan, 2026-11-12

> **Status:** Proposed · waiting for the owner's approval of scope and budget · ROADMAP "P0 release proof" · builds on [N2](../reports/2026-11-12-n2-savings.md)

## What is still missing from the P0 release proof

The roadmap asks for four baselines before any savings claim: pinned premium, pinned inexpensive capable, a static combo, and rule-based `auto`. Each must have success checks, and the accounting must include retries, fallbacks and unknown costs.

N2 already ran on 12 `hard-*` tasks × 3 runs with OpenCode:
- pinned Sonnet 5.5 and pinned Haiku 5.5;
- `auto`, which chose Qwen's subscription;
- two task-engine cascades.

These baselines are still missing:

1. **A static combo.** It falls back on errors only, not on failed checks, which is how the gateway's combos behave for a client.
2. **`auto/cheap`, and `auto` on today's routing.** Since N2, the reference catalog (ADR-031), the free-connection rule (ADR-032), the 402 fix and the quota filter have changed what `auto` sees. N2's `auto` arm also measured a subscription, so it compared no money against money.
3. **A second agent.** N2 used only OpenCode, and lists "Other agents" under what it cannot claim. Claude Code sends a different, larger prompt.
4. **Drift.** N2 ran on 2026-11-11/12. A small control arm shows whether the same arm still gives the same figures.

## Arms

All arms use the same 12 `hard-*` cases, the same checks and protected test files, and the task engine in single mode (`bench --engine`). Each attempt goes through a scratch Pygg gateway and carries its own session key, so its cost comes from the ledger.

| # | Arm | Agent | Routing | Runs | Expected metered money |
| --- | --- | --- | --- | --- | --- |
| A | Drift control | OpenCode | pinned Haiku 5.5 | 12 (×1) | ≈ $0.16 |
| B | `auto/cheap` | OpenCode | lowest stated cost: free and local models first | 36 (×3) | $0 to $0.50 (Haiku if it is ranked) |
| C | `auto`, metered and free only | OpenCode | `auto` with Qwen's subscription off for the client, so money is compared with money | 36 | $0.20 to $1.00 |
| D | Static combo "free, then Haiku" | OpenCode | combo: an OpenRouter `:free` coding model, then Haiku 5.5 on errors | 36 | $0.10 to $0.50 |
| E | Claude Code, pinned Haiku 5.5 | Claude Code | through the gateway, model pinned | 36 | ≈ $1.00 |
| F | Claude Code, pinned Sonnet 5.5 | Claude Code | through the gateway, model pinned | 12 (×1) | ≈ $2.00 |

- **Total expected:** about $3.50 to $5.00.
- **Proposed cap:** $5.00. Each arm has its own `--budget-usd`, and the client has a strict monthly limit.
- **Spend so far:** about $9.62 of the $20 (ADR-028). After this run it would be at most $14.62.
- Sonnet with OpenCode is not run again. N2's figure ($0.114 per solved task) is the premium baseline, checked for drift by arm A.

## What it could support, and what not

- **Can:** "on these 12 tasks, with OpenCode, `auto/cheap` / the free-then-Haiku combo / `auto` cost X% less per solved task than pinned Sonnet, at a pass rate of Y (95% CI)". The same statement for Claude Code, Haiku against Sonnet, on 12 tasks with one run each.
- **Cannot:**
  - savings on the owner's or anyone's real work;
  - other languages;
  - provider invoices: these are list-price estimates;
  - the value of a subscription's quota.

## Prerequisites from the owner

1. **Approve the cap of $5.00.**
2. **The keys needed for a scratch gateway in the User environment.** The bench never touches the owner's store; it builds a scratch store, adds connections from environment variables, and removes them from the Credential Manager afterwards.
   - `ANTHROPIC_API_KEY` and `QWEN_API_KEY` are already there.
   - Needed: `OPENROUTER_API_KEY`.
   - Optional: `ZAI_API_KEY` (free Flash models) and `GEMINI_API_KEY` (already there; free tier).
3. **OpenRouter's free daily limit.** A key that has bought less than $10 of credit gets 50 free requests a day (OpenRouter's docs), and arms B and D need about 300. Options:
   - **(a) Top up $10 of OpenRouter credit.** The limit becomes 1,000 a day. This is outside the benchmark budget; the credit stays usable.
   - **(b) Leave OpenRouter out of B and D.** Free models then come from Z.AI's Flash models, Gemini's free tier and the local Ollama model.
   - **(c) Spread B and D over several days**, at 50 requests a day.
4. **Claude Code through the gateway** bills the Anthropic API key, not the owner's subscription, during the run only. Its own settings are not changed: the gateway address and session header are set for the bench process only.

## Steps

1. Smoke test, $0.05 at most: one case per arm, to check routing, session keys, the ledger link and the cap. Stop if anything reads as an unknown cost where a price exists.
2. Run A, then B, C and D (OpenCode), then E and F (Claude Code). Stop any arm that reaches its cap.
3. Report in `benchmark/reports/`, in N2's form:
   - results for readers;
   - the full table from `scripts/bench-report.py --baseline sonnet`;
   - tokens and requests from the ledger;
   - "What this run cannot claim".

   Raw results go in `benchmark/results/` with `SHA256SUMS`, scanned for keys before they are committed.
4. Update the ROADMAP's P0 status and the progress report. Publish only after the owner approves, as with N2.

## Timing

About 3 to 5 hours of unattended runs, mostly OpenCode arms and the free models' rate limits. It can run while other work continues.
