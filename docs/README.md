# Pygg documentation

> **Status:** Current · the index of every project document and the rules they follow · reviewed 2026-11-21

**Pygg** — *One Flow. Many Agents.* A local gateway that routes each coding agent's requests to a model you connected and measures what it saved. Smart tasks (agent CLIs started from an IDE) are experimental and on hold.

- Repository: [nambean/Pygg](https://github.com/nambean/Pygg). The data folder (`app.smarttaskrouter.desktop`) and the credential service name keep the engineering name SmartTaskRouter.
- Every document is final: it describes Pygg as it is now. History is in git.

## Where to start

| You want to… | Read |
| --- | --- |
| **Use Pygg** | [GETTING-STARTED](GETTING-STARTED.md) up to your coding tool → [PROVIDERS](PROVIDERS.md) → [SECURITY](SECURITY.md) before pasting a key. The in-app Guide repeats the steps |
| **Know what it saves** | [BENCHMARKS](BENCHMARKS.md), then the [reports](../benchmark/reports) |
| **Change Pygg** | [CONTRIBUTING](../CONTRIBUTING.md) → [ARCHITECTURE](ARCHITECTURE.md) → [IMPLEMENTATION-STATUS](IMPLEMENTATION-STATUS.md) → [ROADMAP](ROADMAP.md) → the ADR in [DECISIONS](DECISIONS.md) for the area |
| **Review Pygg** | IMPLEMENTATION-STATUS ("Not verified yet", "Designed, not built"), the ROADMAP's release gates, SECURITY's "Not covered yet", the benchmark reports |

When documents disagree: **IMPLEMENTATION-STATUS** = what is built and verified; **ROADMAP** = what comes next; **DECISIONS** = why; the rest = how a part works.

## Using Pygg

| Document | What it holds |
| --- | --- |
| [../README.md](../README.md) | Overview, animated diagrams, install, quick start, measured results |
| [GETTING-STARTED.md](GETTING-STARTED.md) | Install, first run, connect a provider, set up a coding tool or editor, cost and savings, CLI, troubleshooting, FAQ |
| [PROVIDERS.md](PROVIDERS.md) | Presets, endpoints, free tiers, which answered live; web-session sites and the guard |
| [BENCHMARKS.md](BENCHMARKS.md) | Every published benchmark in one table, with what it does not claim |
| [SECURITY.md](SECURITY.md) | What is protected and how, what the user is trusted with, how to report a problem |

## Building and changing Pygg

| Document | What it holds |
| --- | --- |
| [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) | **Authoritative.** Every built part and how it is verified; what is not verified yet |
| [ROADMAP.md](ROADMAP.md) | **Authoritative.** What comes next, in order; release gates |
| [DECISIONS.md](DECISIONS.md) | Architecture decision records, ADR-001 to ADR-042 |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Components, a gateway request step by step, web sessions, where to change what |
| [SCHEMA.md](SCHEMA.md) | Store tables, generated from the migrations (`scripts/schema-reference.py`); edit the migrations, not this file |
| [AGENTS.md](AGENTS.md) | Coding tools through Pygg, agent CLIs with their own sign-in, the MCP server, the task engine (on hold) |
| [design/SCREENS.md](design/SCREENS.md) | The app's screens as built |
| [design/CLI.md](design/CLI.md) | The `pygg` command line: commands, output, exit codes, npm launcher |
| [design/BRAIN.md](design/BRAIN.md) | Facts feed, role aliases, `pygg/brain`, Kanban board, Brain Storming screen (ADR-040 to ADR-042); built and not built |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | How to change Pygg: order, tests, schema, money, writing, commits |
| [../benchmark/README.md](../benchmark/README.md) | Benchmark harness, cases, running it through the gateway |
| [../benchmark/reports/](../benchmark/reports) | One report per run: method, cost, raw results |
| [../packages/npm/PUBLISHING.md](../packages/npm/PUBLISHING.md) | How npm packages are published; versions shipped |
| [../THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) | Code and data from other projects, with licences |
| [`../scripts/readme-diagrams.mjs`](../scripts/readme-diagrams.mjs) | Generates the README's animated SVGs; update when the request flow, CLI or npm status changes |

## Writing rules

[CONTRIBUTING](../CONTRIBUTING.md#documents): final only, one status line, names, claims with evidence, honest costs, owner decisions as ADRs, user docs say only what IMPLEMENTATION-STATUS says is built ([Writing](../CONTRIBUTING.md#writing)). English only.

## Brand

Name, tagline, logo, icon, colours: [branding](../branding/README.md) (`brand.json`). UI and README diagrams use the green accent `#0B7A4B` (light) / `#0D8551` (dark).
