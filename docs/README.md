# Pygg documentation

> **Status:** Current · the index of every project document, grouped by reader, and the rules they follow · reviewed 2026-11-14

**Pygg** — *One Flow. Many Agents.* Run your coding agents on cheaper models without losing the answers: a local gateway that routes each request to a model you connected, and measures what it saved ([benchmarks](BENCHMARKS.md), [branding](../branding/README.md)). Smart tasks, run by agent CLIs and started from an IDE, are experimental and on hold. The repository is [nambean/Pygg](https://github.com/nambean/Pygg). Only the app's data folder (`app.smarttaskrouter.desktop`) and its credential service name keep the engineering name SmartTaskRouter.

Every document here is final: it describes Pygg as it is now. How it got there is in the git history.

## Where to start

| You want to… | Read |
| --- | --- |
| **Use Pygg** | [GETTING-STARTED](GETTING-STARTED.md), top to bottom, stopping at your coding tool; then [PROVIDERS](PROVIDERS.md) to choose a provider and [SECURITY](SECURITY.md) before pasting a key. The in-app Guide repeats the steps. |
| **Know what it saves** | [BENCHMARKS](BENCHMARKS.md), then the [reports](../benchmark/reports) it links. |
| **Change Pygg** | [CONTRIBUTING](../CONTRIBUTING.md) → [ARCHITECTURE](ARCHITECTURE.md) → [IMPLEMENTATION-STATUS](IMPLEMENTATION-STATUS.md) → [ROADMAP](ROADMAP.md) → the ADR in [DECISIONS](DECISIONS.md) that bounds the area. |
| **Review Pygg** | IMPLEMENTATION-STATUS ("Not verified yet", "Designed, not built"), the ROADMAP's release gates, SECURITY's "Not covered yet", and the benchmark reports. |

When documents disagree: **IMPLEMENTATION-STATUS** says what is built and how it was verified; the **ROADMAP** says what comes next; **DECISIONS** says why; the other documents say how a part works.

## Using Pygg

| Document | What it holds |
| --- | --- |
| [../README.md](../README.md) | The overview: what Pygg does, the animated diagrams, install, quick start, measured results |
| [GETTING-STARTED.md](GETTING-STARTED.md) | Install, first run, connect a provider, set up Claude Code, Codex, OpenCode or an editor, cost and savings, the command line, troubleshooting, FAQ |
| [PROVIDERS.md](PROVIDERS.md) | Connection presets, endpoints, free tiers, which have answered live; web-session sites and the guard |
| [BENCHMARKS.md](BENCHMARKS.md) | Every published benchmark in one table, with what it does not claim |
| [SECURITY.md](SECURITY.md) | What Pygg protects and how, what the user is trusted with, how to report a problem |

## Building and changing Pygg

| Document | What it holds |
| --- | --- |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) | **Authoritative.** Every built part and how it is verified; what is not verified yet |
| [ROADMAP.md](ROADMAP.md) | **Authoritative.** What comes next, in order, and the release gates |
| [DECISIONS.md](DECISIONS.md) | Architecture decision records, ADR-001 to ADR-042 |
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the parts fit: a gateway request, a web session, the store, the CLI, where to change what |
| [SCHEMA.md](SCHEMA.md) | The store's tables, generated from the migrations (`scripts/schema-reference.py`); edit the migrations, not this file |
| [AGENTS.md](AGENTS.md) | Coding tools through Pygg, agent CLIs with their own sign-in, the MCP server and the task engine (on hold) |
| [design/SCREENS.md](design/SCREENS.md) | The app's screens as built |
| [design/CLI.md](design/CLI.md) | The `pygg` command line: commands, output, exit codes, the npm launcher |
| [design/BRAIN.md](design/BRAIN.md) | The decided, not built, next shape of smart tasks: the model facts feed, `pygg/brain` and the Kanban board (ADR-040 to ADR-042) |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | How to change Pygg: order, tests, schema, money, writing, commits |
| [../benchmark/README.md](../benchmark/README.md) | The benchmark harness, its cases and how to run it through the gateway |
| [../benchmark/reports/](../benchmark/reports) | Measured results, one report per run, with method, cost and raw results |
| [../packages/npm/PUBLISHING.md](../packages/npm/PUBLISHING.md) | How the npm packages are published, and what each version shipped |
| [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) | Code and data taken from other projects, with their licences |
| [`../scripts/readme-diagrams.mjs`](../scripts/readme-diagrams.mjs) | Generates the README's animated SVGs; update it when the request flow, the CLI or the npm status changes |

## Planned documents

- A root `SECURITY.md` pointing at [SECURITY.md](SECURITY.md), for GitHub's Security tab.
- `CHANGELOG.md` at the first tagged release.

## Writing rules

- **Final only.** A document says what is true now. When something changes, change the sentence to the new truth. No dated logs ("Update 2026-…", "Live verification on …"), no "last update" prose, no review records, progress reports or superseded plans in `docs/`: Codex consultations and progress reports go to the chat or the session notes, and history stays in git. A measured result is the exception: it is a report under `benchmark/reports/`, left as run.
- **Status line.** Each document opens with `> **Status:** Current · <what it is> · reviewed <date>` (IMPLEMENTATION-STATUS and ROADMAP say Authoritative).
- **Names.** The product is **Pygg** everywhere: prose, the repository, packages, crates, `x-pygg-*` headers and `PYGG_*` variables. The engineering name SmartTaskRouter stays in two identifiers only, so existing data stays readable: the app identifier `app.smarttaskrouter.desktop` (it names the folder that holds the user's store) and the credential service name `SmartTaskRouter`. The gateway still reads the old session header `x-smarttaskrouter-session`, which clients set up before the rename send.
- **Claims carry their evidence.** "Built" names its code; "verified" says how (unit test, fake provider, live run with which provider). Untested is said plainly.
- **Costs are honest.** Unknown is never $0; an estimate says what it was estimated from; a subscription's use is quota, not money.
- **Owner decisions** (licence, budget, scope) are recorded as ADRs and cited by number.
- **User documents say only what IMPLEMENTATION-STATUS says is built**, and repeat its "not verified" where a user would otherwise assume it works.
- **Language:** English.

## Brand

The name, tagline, logo, icon and colours are in [branding](../branding/README.md) (`brand.json`). The interface's green accent (`#0B7A4B` light, `#0D8551` dark) is the brand accent, and the README's diagrams use it.
