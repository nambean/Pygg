# Pygg documentation

> **Status:** Current · the index of every project document, grouped by reader, and the rules they follow · reviewed 2026-11-13

**Pygg** — *One Flow. Many Agents.* A local AI gateway that cuts what coding tools cost, with agent task routing on top ([branding](../branding/README.md)). The repository and its technical identifiers keep the engineering name **SmartTaskRouter**.

## Where to start

**To use Pygg:** [GETTING-STARTED.md](GETTING-STARTED.md), top to bottom, stopping at your coding tool. Then [PROVIDERS.md](PROVIDERS.md) when choosing a provider and [SECURITY.md](SECURITY.md) before pasting a key. The in-app Guide repeats the steps with the commands ready to copy.

**To change Pygg:** [CONTRIBUTING.md](../CONTRIBUTING.md), then [ARCHITECTURE.md](ARCHITECTURE.md), then [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) for what exists and what is not verified, then [ROADMAP.md](ROADMAP.md) for whether the change is in order, then the ADR in [DECISIONS.md](DECISIONS.md) that bounds the area.

**To review Pygg:** [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md), in particular "Not verified yet" and "Designed, not built"; the ROADMAP's P0 table and release gates; SECURITY's "Not covered yet"; the [benchmark reports](../benchmark/reports); then [reviews/](reviews) so an objection is not raised twice.

## Which document answers what

Read them in this order when they disagree:

1. **What is built, and how was it verified?** [Implementation Status](IMPLEMENTATION-STATUS.md).
2. **What comes next, and what is the gate?** [ROADMAP](ROADMAP.md). It is the only authority on order.
3. **Why was it decided?** [DECISIONS](DECISIONS.md) (ADR-001 to ADR-035).
4. **How does one part work?** The current and reference documents below.
5. **What did reviewers object to, and what was answered?** [reviews](reviews).

## Using Pygg

| Document | Status | What it holds |
| --- | --- | --- |
| [BENCHMARKS.md](BENCHMARKS.md) | Current | What Pygg changes, measured: every published benchmark in one table, with what it does not claim |
| [GETTING-STARTED.md](GETTING-STARTED.md) | Current | Install, first run, connect a provider, set up Claude Code, Codex, OpenCode or an editor, cost and savings, the command line, troubleshooting, FAQ |
| [PROVIDERS.md](PROVIDERS.md) | Current | Connection presets, their endpoints, free tiers, which have answered live, and the documentation each came from; web-session sites and their sign-in hosts |
| [SECURITY.md](SECURITY.md) | Current | What Pygg protects and how, what the user is trusted with, how to report a problem |
| [../README.md](../README.md) | Current | The overview: status, how it works, what works today, the navigation, how to build from source |

## Building and changing Pygg

| Document | Status | What it holds |
| --- | --- | --- |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) | Authoritative | Every built part, its verification level, and every live run with its date and cost |
| [ROADMAP.md](ROADMAP.md) | Authoritative | Delivery order (P0 status, the revised plan A–E, housekeeping) and release gates |
| [DECISIONS.md](DECISIONS.md) | Current | Architecture decision records |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current | How the parts fit together: a gateway request, a task, the boundaries, where to change what |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Current | How to change Pygg: order, status, tests, schema, money, writing, commits |
| [SCHEMA.md](SCHEMA.md) | Current | The store's tables and columns, generated from the migrations (`scripts/schema-reference.py`); edit the migrations, not this file |
| [AGENTS.md](AGENTS.md) | Current | Agent CLIs with their own sign-in, the task engine, the MCP server and task API (Smart Tasks, on hold) |
| [../benchmark/README.md](../benchmark/README.md) | Current | The benchmark harness, its cases and how to run it through the gateway |
| [../packages/npm/PUBLISHING.md](../packages/npm/PUBLISHING.md) | Current | How the npm developer preview is published |
| [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) | Current | Code and data taken from other projects, with their licences |

## Specifications and studies

| Document | Status | What it holds |
| --- | --- | --- |
| [ACCOUNT_API_ROTATION.md](ACCOUNT_API_ROTATION.md) | Reference | Account pool specification; pools of API-key accounts are built (ADR-034), and pools of web sessions (ADR-035) |
| [CLI_IDE_NPM_PROVIDER_TIERS.md](CLI_IDE_NPM_PROVIDER_TIERS.md) | Reference | CLI, IDE integrations, npm launcher and billing tiers; the CLI (`pygg setup`, `pygg run claude`) and the npm developer preview are built |
| [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md) | Reference | What was taken from OmniRoute's features, what was rejected, and why |
| [RFC-2026-11-12-ORIGINAL-VISION-GAPS.md](RFC-2026-11-12-ORIGINAL-VISION-GAPS.md) | Reference | Proposed gaps in shared task context, npm/CLI, integrations, routing and evidence; pending owner/Claude reconciliation |
| [research/2026-11-13-omniroute-docs-study.md](research/2026-11-13-omniroute-docs-study.md) | Reference | How OmniRoute and 9router document themselves, what Pygg borrows and what not, and the proposed `docs/` layout for the open-source release |
| [research/2026-11-12-web-session-routers.md](research/2026-11-12-web-session-routers.md) | Reference | How 9router, OmniRoute and others drive AI websites, which sites Pygg supports and why |
| [research/2026-11-12-open-source-adoption-study.md](research/2026-11-12-open-source-adoption-study.md) | Reference | Source-linked comparison of Mem0, OpenCode, Letta, LangGraph, Aider, Graphiti, LiteLLM, Semantic Router, OmniRoute and Skills, with Pygg-specific adoption gates |
| [design/SCREENS.md](design/SCREENS.md) | Current §2.2, Historical elsewhere | The navigation as built since 2026-11-12 (§2.2: Home, Providers, Endpoint, CLI Tools, Combos, Usage, Policies, Guide) and the v0.1 screen design kept for its reasoning |
| [SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md) | Historical | The v1.2 functional plan; superseded where ROADMAP and ADR-026 to ADR-028 say so |
| [design/README.md](design/README.md) | Historical | The original SVG sketches |
| [UNIVERSAL_SHARED_MEMORY.md](UNIVERSAL_SHARED_MEMORY.md) | Withdrawn | App-owned conversation memory; not pursued (ADR-028) |

## The record

| Document | Status | What it holds |
| --- | --- | --- |
| [../benchmark/reports/](../benchmark/reports) | Record | Benchmark results as run, one report per pass, each with its method, cost and raw results |
| [reviews/](reviews) | Record | Architecture reviews, the Codex interface reviews (2026-11-12) and the answers to them, as written at the time |
| [reports/](reports) | Record | Progress reports to the owner, in Vietnamese, and release-check reports, as written at the time |

## Planned documents

Named here so nobody writes them twice; each is written when its content exists (see the [docs study](research/2026-11-13-omniroute-docs-study.md), section 6).

- A root `SECURITY.md` pointing at [SECURITY.md](SECURITY.md), for GitHub's Security tab.
- `CHANGELOG.md` at the first tagged release.
- One page per coding tool under `docs/user/` once GETTING-STARTED outgrows one page.
- A per-platform install section when a macOS or Linux build exists (ADR-001).
- A comparison page with its method, resting on the benchmark reports.

## Status words

Each document has one status line under its title.

- **Authoritative:** wins over every other document on its subject.
- **Current:** describes what is built or decided now; kept up to date with the code.
- **Reference:** a specification or study that is partly built or not yet scheduled; ROADMAP decides when.
- **Historical:** an earlier design kept for its reasoning; its sections hold only where nothing newer says otherwise.
- **Withdrawn:** a design the owner decided not to pursue, kept for reference.
- **Record:** a review or response, left as written. Later facts go into a dated note at its top.

## Writing rules

- **Names.** The product is **Pygg** in prose. **SmartTaskRouter** stays only where it is an identifier: the repository, `x-smarttaskrouter-*` headers, `SMARTTASKROUTER_*` variables, package and crate names, and file names. Reviews keep the name they were written with.
- **Dates** are absolute (2026-11-11), never "yesterday" or "last week". A change to an older statement goes in a dated **Update** note, or the statement is corrected in place; nothing is silently rewritten in a Record.
- **Claims carry their evidence.** "Built" names its code; "verified" says how (unit test, fake provider, live run) and, for a live run, the model, the date and what it cost. Untested is said plainly.
- **Costs are honest.** Unknown is never $0; an estimate says what it was estimated from; a subscription's use is quota, not money.
- **Owner decisions** (licence, budget, scope) are recorded as ADRs and cited by number.
- **User documents say only what Implementation Status says is built**, and repeat its "not verified" where a user would otherwise assume it works.
- **Language:** documents are in English; reviews may be in Vietnamese, as written, and progress reports to the owner are.
- **File names** stay as they are, so links keep working.

## Brand

The name, tagline, logo, icon and colours are in [branding](../branding/README.md) (`brand.json`). The interface's green accent (`#0B7A4B` light, `#0D8551` dark) is the brand accent. The app's name and icons come from it (ROADMAP N5); the data folder's identifier and the credential service keep the engineering name, so existing stores and keys stay where they are.
