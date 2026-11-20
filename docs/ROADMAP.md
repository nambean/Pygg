# Roadmap

> **Status:** Authoritative · delivery order and release gates · reviewed 2026-11-21

Built: [Implementation Status](IMPLEMENTATION-STATUS.md); why: [DECISIONS](DECISIONS.md).

## Next, in order

| # | Item | Status |
| --- | --- | --- |
| 1 | Close the gaps in [Not verified yet](IMPLEMENTATION-STATUS.md#not-verified-yet), within ADR-028's budget | in progress |
| 2 | Every screen clicked in the installed app; D12 checks (ADR-012) | next |
| 3 | Clean Windows machine: install, first run, a tool set up, uninstall | next |
| 4 | Token and cost work (ADR-036): read the cache-lifetime and clearing shadows and session receipts on real use; opt-in changes only if they pay | in progress |
| 5 | Measure Codex with and without Pygg's model catalog | next |
| 6 | macOS npm packages on a real Mac; the app on macOS and Linux (ADR-001) | next |
| 7 | Demo data replays recorded snapshots | next |
| 8 | Vietnamese translation | next (last) |
| 9 | A larger benchmark: more cases, every coding-capable model, three clients, three repeats, results per task category | later |

## Release gates

- **Parity:** same model direct and through Pygg, no unexplained cache or cost regression.
- **Honesty:** no saving claim without a reproducible benchmark; unknown cost stays Unknown.
- **Security:** no unconfirmed account switch or refused data-use terms; limits hold under concurrency; loopback only.
- **Errors:** real 403, 429, quota and timeout answers as fixtures.
- **Install:** clean-machine install and uninstall; signed installer.
- **Interface:** every screen clicked natively; D12 checks; `pnpm ui-check`; Vietnamese complete.
- **Web sessions:** the guard always on; account-risk warning per connection (ADR-035).

## Later (when data or a request justifies it)

- Subscription quota tracking, if a provider states it.
- The `image` role, once the gateway has an images path (ADR-041).
- A root `SECURITY.md` and a `CHANGELOG.md`.
- Billing-tier views (ADR-020); minimum-tier rules by task category (ADR-006).
- Idempotency keys, provider usage reconciliation, fallback after headers: held.
- The benchmark run from the app.
- Aider and Cursor verified as clients.
- A signed updater.
- `auto/fast`, `auto/local`, `auto/quality`: only if the benchmark needs them.
- Semantic caching, advanced compaction: only if a test beats a simpler baseline.
- Skills from a catalog, pinned to a reviewed commit (ADR-039).

Withdrawn: Auto Chat and app-owned memory (ADR-028); bridging subscription sign-ins (ADR-026).
