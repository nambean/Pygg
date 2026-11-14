# Archive

> **Status:** Historical · plans, specifications and studies that shaped Pygg and are now settled elsewhere · moved here 2026-11-14

These documents are kept because decisions, reviews and code comments cite them. Nothing here describes Pygg as it is: what is built is in [Implementation Status](../IMPLEMENTATION-STATUS.md), what comes next in the [ROADMAP](../ROADMAP.md), and why in [DECISIONS](../DECISIONS.md). Each file is left as written, so the line numbers that reviews cite still hold.

| Document | What it was | Where it is settled now |
| --- | --- | --- |
| [SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md) | The v1.2 product and functional plan | ROADMAP; ADR-026 to ADR-028 (gateway first, tasks from the IDE, no Auto Chat); the built screens in [design/SCREENS.md](../design/SCREENS.md) §2.2 |
| [ACCOUNT_API_ROTATION.md](ACCOUNT_API_ROTATION.md) | Account pools and key rotation, as specified | ADR-034 (pools), ADR-038 (key rings), `gateway/policy.rs` and `gateway/keys.rs` |
| [CLI_IDE_NPM_PROVIDER_TIERS.md](CLI_IDE_NPM_PROVIDER_TIERS.md) | CLI, IDE integrations, the npm launcher and billing tiers | The `pygg` CLI ([design/CLI.md](../design/CLI.md)), ADR-037 (one npm command), billing per connection and model |
| [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md) | What to take from OmniRoute's features, and what to reject | ADR-024 (the shortlist and its gates); [PROVIDERS](../PROVIDERS.md) for what is not added and why |
| [RFC-2026-11-12-ORIGINAL-VISION-GAPS.md](RFC-2026-11-12-ORIGINAL-VISION-GAPS.md) | Eleven gaps against the original vision | ADR-030, after its [counter-review](../reviews/2026-11-12-original-vision-gaps-response.md) |
| [UNIVERSAL_SHARED_MEMORY.md](UNIVERSAL_SHARED_MEMORY.md) | App-owned conversation memory | Withdrawn by ADR-028 (no Auto Chat); explicit sessions (P0-4) remain |
| [research/2026-11-12-web-session-routers.md](research/2026-11-12-web-session-routers.md) | How other routers drive AI websites | ADR-035 (web sessions) |
| [research/2026-11-12-open-source-adoption-study.md](research/2026-11-12-open-source-adoption-study.md) | Mem0, OpenCode, Letta, LangGraph, Aider, LiteLLM and others, compared | Its [response](../reviews/2026-11-12-oss-patterns-response.md) |
| [research/2026-11-13-omniroute-docs-study.md](research/2026-11-13-omniroute-docs-study.md) | How OmniRoute and 9router document themselves | The documentation layout in [docs/README.md](../README.md) |
| [research/2026-11-14-token-optimization-study.md](research/2026-11-14-token-optimization-study.md) | Open-source token-saving tools, studied | ADR-036 (keep the provider's cache working) and its [discussion](../reviews/2026-11-14-codex-token-optimization-discussion.md) |
