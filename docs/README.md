# Pygg documentation

> **Status:** Current · the index of every project document, grouped by reader, and the rules they follow · reviewed 2026-11-14

**Pygg** — *One Flow. Many Agents.* A local AI gateway that routes coding agents across the providers you connected and records what every request cost ([branding](../branding/README.md)). Smart tasks, run by agent CLIs and started from an IDE, are experimental and on hold. The repository and its technical identifiers keep the engineering name **SmartTaskRouter**.

## Where to start

| You want to… | Read |
| --- | --- |
| **Use Pygg** | [GETTING-STARTED](GETTING-STARTED.md), top to bottom, stopping at your coding tool; then [PROVIDERS](PROVIDERS.md) to choose a provider and [SECURITY](SECURITY.md) before pasting a key. The in-app Guide repeats the steps. |
| **Know what it saves** | [BENCHMARKS](BENCHMARKS.md), then the [reports](../benchmark/reports) it links. |
| **Change Pygg** | [CONTRIBUTING](../CONTRIBUTING.md) → [ARCHITECTURE](ARCHITECTURE.md) → [IMPLEMENTATION-STATUS](IMPLEMENTATION-STATUS.md) → [ROADMAP](ROADMAP.md) → the ADR in [DECISIONS](DECISIONS.md) that bounds the area. |
| **Review Pygg** | IMPLEMENTATION-STATUS ("Not verified yet", "Designed, not built"), the ROADMAP's release gates, SECURITY's "Not covered yet", the benchmark reports, then [reviews](reviews) so an objection is not raised twice. |

When documents disagree: **IMPLEMENTATION-STATUS** says what is built and how it was verified; the **ROADMAP** says what comes next; **DECISIONS** says why; the current documents below say how a part works; **reviews** say what was objected to and answered.

## Using Pygg

| Document | Status | What it holds |
| --- | --- | --- |
| [../README.md](../README.md) | Current | The overview: what Pygg does, the animated diagrams, install, quick start, measured results |
| [GETTING-STARTED.md](GETTING-STARTED.md) | Current | Install, first run, connect a provider, set up Claude Code, Codex, OpenCode or an editor, cost and savings, the command line, troubleshooting, FAQ |
| [PROVIDERS.md](PROVIDERS.md) | Current | Connection presets, endpoints, free tiers, which have answered live; web-session sites and their sign-in hosts |
| [BENCHMARKS.md](BENCHMARKS.md) | Current | Every published benchmark in one table, with what it does not claim |
| [SECURITY.md](SECURITY.md) | Current | What Pygg protects and how, what the user is trusted with, how to report a problem |

## Building and changing Pygg

| Document | Status | What it holds |
| --- | --- | --- |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) | Authoritative | Every built part, its verification level, and every live run with its date and cost |
| [ROADMAP.md](ROADMAP.md) | Authoritative | Delivery order and release gates |
| [DECISIONS.md](DECISIONS.md) | Current | Architecture decision records, ADR-001 to ADR-038 |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current | How the parts fit: a gateway request, a web session, a task, the boundaries, where to change what |
| [SCHEMA.md](SCHEMA.md) | Current | The store's tables, generated from the migrations (`scripts/schema-reference.py`); edit the migrations, not this file |
| [AGENTS.md](AGENTS.md) | Current | Agent CLIs with their own sign-in, the task engine, the MCP server and task API (smart tasks, on hold) |
| [design/CLI.md](design/CLI.md) | Current | The `pygg` command line: commands, output, exit codes |
| [design/SCREENS.md](design/SCREENS.md) | Current §2.2, Historical elsewhere | The navigation as built (§2.2) and the v0.1 screen design kept for its reasoning |
| [design/README.md](design/README.md) | Historical | The original SVG sketches |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Current | How to change Pygg: order, status, tests, schema, money, writing, commits |
| [../benchmark/README.md](../benchmark/README.md) | Current | The benchmark harness, its cases and how to run it through the gateway |
| [../packages/npm/PUBLISHING.md](../packages/npm/PUBLISHING.md) | Current | How the npm developer preview is published, and what each version shipped |
| [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) | Current | Code and data taken from other projects, with their licences |
| [`scripts/readme-diagrams.mjs`](../scripts/readme-diagrams.mjs) | Current | Generates the README's animated SVGs (`docs/images/usage-*.svg`, `request-flow-*.svg`); [Codex's review of them](reviews/2026-11-14-codex-readme-diagrams-review.md) |

## The record

| Folder | Status | What it holds |
| --- | --- | --- |
| [../benchmark/reports/](../benchmark/reports) | Record | Benchmark results as run, one report per pass, with method, cost and raw results |
| [reviews/](reviews) | Record | Architecture reviews, Codex's interface and design reviews, and the answers to them, as written at the time |
| [reports/](reports) | Record | Progress reports to the owner (in Vietnamese), release checks and the production-readiness review |
| [archive/](archive/README.md) | Historical | Plans, specifications and studies now settled by ADRs: the functional plan, account rotation, CLI/IDE/npm tiers, OmniRoute research, the original-vision RFC, shared memory (withdrawn) and the research notes. Kept because ADRs and reviews cite them |

## Planned documents

Named here so nobody writes them twice; each is written when its content exists.

- A root `SECURITY.md` pointing at [SECURITY.md](SECURITY.md), for GitHub's Security tab.
- `CHANGELOG.md` at the first tagged release.
- One page per coding tool under `docs/user/` once GETTING-STARTED outgrows one page.
- A macOS install section once the macOS packages are published (ADR-037).

## Status words

Each document has one status line under its title.

- **Authoritative:** wins over every other document on its subject.
- **Current:** describes what is built or decided now; kept up to date with the code.
- **Reference:** a specification or study that is partly built or not yet scheduled; ROADMAP decides when.
- **Historical:** an earlier design kept for its reasoning; it holds only where nothing newer says otherwise.
- **Withdrawn:** a design the owner decided not to pursue, kept for reference.
- **Record:** a review, report or response, left as written. Later facts go into a dated note at its top.

## Writing rules

- **Names.** The product is **Pygg** in prose. **SmartTaskRouter** stays only where it is an identifier: the repository, `x-smarttaskrouter-*` headers, `SMARTTASKROUTER_*` variables, package and crate names, and file names. Reviews keep the name they were written with.
- **Dates** are absolute (2026-11-11), never "yesterday" or "last week". A change to an older statement goes in a dated **Update** note, or the statement is corrected in place; nothing is silently rewritten in a Record.
- **Claims carry their evidence.** "Built" names its code; "verified" says how (unit test, fake provider, live run) and, for a live run, the model, the date and what it cost. Untested is said plainly.
- **Costs are honest.** Unknown is never $0; an estimate says what it was estimated from; a subscription's use is quota, not money.
- **Owner decisions** (licence, budget, scope) are recorded as ADRs and cited by number.
- **User documents say only what Implementation Status says is built**, and repeat its "not verified" where a user would otherwise assume it works.
- **Language:** documents are in English; reviews may be in Vietnamese, as written, and progress reports to the owner are.
- **File names** stay as they are, so links keep working. A document that is superseded moves to [archive/](archive/README.md) unchanged, with every link to it updated.

## Brand

The name, tagline, logo, icon and colours are in [branding](../branding/README.md) (`brand.json`). The interface's green accent (`#0B7A4B` light, `#0D8551` dark) is the brand accent, and the README's diagrams use it.
