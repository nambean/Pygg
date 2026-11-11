# Pygg documentation

> **Status:** Current · the index of every project document and the rules they follow · reviewed 2026-11-11

**Pygg** — *One Flow. Many Agents.* AI task and agent routing orchestration ([branding](../branding/README.md)). The repository and its technical identifiers keep the engineering name **SmartTaskRouter**.

## Which document answers what

Read them in this order when they disagree:

1. **What is built, and how was it verified?** [Implementation Status](IMPLEMENTATION-STATUS.md).
2. **What comes next, and what is the gate?** [ROADMAP](ROADMAP.md). It is the only authority on order.
3. **Why was it decided?** [DECISIONS](DECISIONS.md) (ADR-001 to ADR-028).
4. **How does one part work?** The current and reference documents below.
5. **What did reviewers object to, and what was answered?** [reviews](reviews).

## Documents

| Document | Status | What it holds |
| --- | --- | --- |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) | Authoritative | Every built part, its verification level, and every live run with its date and cost |
| [ROADMAP.md](ROADMAP.md) | Authoritative | Delivery order (P0 status, the revised plan A–E, housekeeping) and release gates |
| [DECISIONS.md](DECISIONS.md) | Current | Architecture decision records |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current | How the parts fit together: a gateway request, a task, the boundaries, where to change what |
| [SECURITY.md](SECURITY.md) | Current | What Pygg protects and how, what the user is trusted with, how to report a problem |
| [SCHEMA.md](SCHEMA.md) | Current | The store's tables and columns, generated from the migrations (`scripts/schema-reference.py`) |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Current | How to change Pygg: order, status, tests, schema, money, writing, commits |
| [AGENTS.md](AGENTS.md) | Current | Agent CLIs with their own sign-in, the task engine, the MCP server and task API |
| [PROVIDERS.md](PROVIDERS.md) | Current | Connection presets, their endpoints and the documentation each came from |
| [../benchmark/README.md](../benchmark/README.md) | Current | The benchmark harness, its cases and how to run it through the gateway |
| [../benchmark/reports/](../benchmark/reports) | Record | Benchmark results as run, one report per pass |
| [ACCOUNT_API_ROTATION.md](ACCOUNT_API_ROTATION.md) | Reference | Account pool specification; partly built, pools are ROADMAP E |
| [CLI_IDE_NPM_PROVIDER_TIERS.md](CLI_IDE_NPM_PROVIDER_TIERS.md) | Reference | CLI, IDE integrations, npm launcher and billing tiers; partly built |
| [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md) | Reference | What was taken from OmniRoute, what was rejected, and why |
| [SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md](SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md) | Historical | The v1.2 functional plan; superseded where ROADMAP and ADR-026 to ADR-028 say so |
| [design/SCREENS.md](design/SCREENS.md) | Historical | The v0.1 screen design; the built screens are listed in Implementation Status |
| [design/README.md](design/README.md) | Historical | The original SVG sketches |
| [UNIVERSAL_SHARED_MEMORY.md](UNIVERSAL_SHARED_MEMORY.md) | Withdrawn | App-owned conversation memory; not pursued (ADR-028) |
| [reviews/](reviews) | Record | Architecture reviews and the answers to them, as written at the time |

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
- **Language:** documents are in English; reviews may be in Vietnamese, as written.
- **File names** stay as they are, so links keep working.

## Brand

The name, tagline, logo, icon and colours are in [branding](../branding/README.md) (`brand.json`). The interface's green accent (`#0B7A4B` light, `#0D8551` dark) is the brand accent. The app's name and icons come from it (ROADMAP N5); the data folder's identifier and the credential service keep the engineering name, so existing stores and keys stay where they are.
