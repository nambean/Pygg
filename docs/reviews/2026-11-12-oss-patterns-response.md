# The chief architect's decisions on the original-vision RFC and the open-source study, 2026-11-12

> **Status:** Record · decisions taken under the owner's instruction of 2026-11-12 ("decide and carry out what was agreed; answer the rest") · answers [the RFC](../RFC-2026-11-12-ORIGINAL-VISION-GAPS.md), [the counter-review](2026-11-12-original-vision-gaps-response.md) and [the open-source study](../research/2026-11-12-open-source-adoption-study.md) · recorded as [ADR-030](../DECISIONS.md) · order: [ROADMAP](../ROADMAP.md)

> **Update 2026-11-12 (later):** PV-05 is built (`75233a3`, [AGENTS.md](../AGENTS.md#from-the-ide-pygg-mcp-and-the-task-api)), out of ADR-030's order, by a session that had started on it before ADR-030 was pushed. It differs from the row below in one respect, on purpose. The entry goes into each agent's **user** configuration, not a workspace file. The reason: a project `.mcp.json` or `opencode.json` is usually committed, and the entry carries the client token. The MCP server already takes the project from the folder the agent runs in, so one user-wide entry serves every workspace. Claude Code's `--scope local` would keep a per-project entry private, but it would also mean one setup per project, and Codex has no per-project MCP entry, so it was not used. A second note, on npm: `npm publish --provenance` ties a package to its source commit and CI run at no cost. That gives a Windows-only developer preview a publisher identity on npm's side before an Authenticode certificate exists, though it does nothing for SmartScreen. The name is still the owner's to register.

## In one paragraph

The counter-review and the study agree on the shape: no memory subsystem, a bounded task handoff tested against today's, and the integrations hub before npm. **The study is right where it corrects the counter-review on PV-02.** MCP task dispatch carries a task description, not a typed, source-linked context package, so PV-02 is "deferred by name", not "already built". I accept that correction. I disagree with three things:
- the RFC's new tables;
- the study's JSON package as the prompt format;
- recording "inspected paths". Pygg cannot see those, and claiming them would be invented provenance.

The reasons are below. What was agreed is being built now, in the order of ADR-030.

> **Update, 2026-11-12 02:10.**
> - **Done:**
>   - PV-11, the completed N2 report and results bundle (`0e25c75`, `bb5b7db`).
>   - H1, behind its flag; the evidence builder lives in `handoff.rs` rather than in `tasks.rs`.
>   - The PV-06 slice (`e3629b1`).
>   - PV-05, the integrations hub, built in a parallel session (`75233a3`). It writes **user-wide** entries through each agent's own command rather than workspace files, so that no token lands in a project file; see that commit and the note below. That supersedes "workspace files by default" in the PV-05 row.
> - **Running:** the H1 A/B.
> - **Next:** PV-04.

## Decisions, item by item

| Item | Decision | What is built or done (2026-11-12) |
| --- | --- | --- |
| **PV-11 / evidence** | **Accept, now** | The N2 report is completed from the raw files. The free-first arm is reported with what it measured, an outage of the free model. A sanitized results bundle is committed under `benchmark/results/2026-11-12-n2/`: no keys, no machine paths, cases by id, plus a SHA-256 of each file |
| **PV-01 / OSS-01 / H1** | **Accept as an experiment**, off by default | A deterministic handoff builder in `tasks.rs`, with three modes: `none`, `tail` (today) and `evidence` (H1). It runs at an attempt boundary. The text actually sent is stored on the attempt row and shown in task detail and in `pygg_task_status`. The A/B (below) decides the default |
| **PV-02** | **Defer by name** (the study's correction accepted) | Nothing. Reopening needs H1 to help, and an owner decision on ADR-028 |
| **PV-03** | Defer | A task attempt is a new agent process, so there is no cache to keep across steps. Within a step the cache already holds (N2: 70–72% of input was cache reads) |
| **PV-04 / OSS-05** | Accept, **after** PV-05 | First one native `pygg` binary with subcommands. npm needs the owner to register the name; a public npm release also needs signing. A clearly labelled Windows-only developer preview may come before signing, if the owner agrees |
| **PV-05 / OSS-02** | **Accept, next product item** | Claude Code, Codex CLI and OpenCode, with the steps detect → show the change → back up → apply with consent → check → restore. Workspace files by default; where a client has its own command for its config, that command is used |
| **PV-06** | Accept the slice that costs nothing | The last rate-limit headers a provider sent are recorded per connection and model, and shown as "as the provider last said". There are no reservations built on them, and everything else stays Unknown |
| **PV-07** | Defer | No run has been short of one key's quota, and no provider's terms on pooling have been read |
| **PV-08 / OSS-03** | Defer the adaptive part | A live shadow cannot be scored without paying to run the path that was not taken, so policies are evaluated only on benchmark replays. OSS-03 is a gap analysis on paper, later |
| **PV-09** | Not a feature | Its deterministic rules go into the H1 builder: strip ANSI and progress noise; keep exit codes, failing lines and paths; mark omissions |
| **PV-10 / OSS-06** | P2, unchanged | — |
| **OSS-04 / H2** | Conditional on H1 | Only if H1 helps and tasks still lose evidence after several attempts |

## Where I disagree, and why

1. **New tables (RFC PV-01).** `task_context_events`, `checkpoints`, `handoff_packages` and an audit table would duplicate what the `attempts` and `audit_events` rows already hold. Each attempt has its base revision, agent and model, check results, escalation reason, the agent's final message and a log tail. The one thing missing is the text sent to the next agent, and that needs a single column. More tables are justified only if H1 shows value and H2 needs retrieval.
2. **"Inspected path names and hashes" (RFC PV-01, study's question 2).** Pygg reads usage and the final message from each agent CLI. It does not read which files the agent opened, because those CLIs do not report it in their headless output. What Pygg does know is which paths an attempt **changed**: the worktree diff before it is removed. H1 carries those, labelled "changed in a discarded attempt; not present in your copy". Listing "inspected" files would be invented provenance.
3. **A JSON package as the prompt (study's H1 contract).** The receiving agent reads a prompt. A JSON object costs more tokens, and agents follow a short, labelled text just as well. H1 sends text with a version header (`pygg.task-handoff.v1`) and fixed sections. Each claim names its source: the check command, the attempt number, the agent. Every claim is labelled as one of:
   - verified by a check;
   - changed in a discarded attempt;
   - claimed by an agent, unverified.
4. **The previous agent's summary.** It is included, because it is often the most useful line. It is always marked as that agent's unverified claim and capped, never promoted to a requirement.
5. **Aider-style repo maps (study, OSS-01).** These are not in H1. Claude Code, Codex and OpenCode each build their own view of the repository, so a second map doubles the context for no measured gain.
6. **Signing before any npm package (counter-review).** I relax this: a labelled Windows-only developer preview could be unsigned. A public release could not. Both still wait for the owner to register the name.

## The A/B that can disprove H1

- **Cases:** the 12 `hard-*` cases, 3 runs each.
- **Ladder:** local first: `qwen3:8b` on Ollama, then Claude Haiku 5.5. The local step fails 35 of 36 times (N2), so the second step almost always receives a handoff.
- **Arms:** three, identical except for the handoff mode: `none`, `tail` and `evidence`.
- **What is measured:** pass rate, input tokens, time and cost of the second step, paired by case; also the share of runs where the second step repeats the first step's failing change.
- **Decision:**
  - H1 becomes the default only if `evidence` beats `tail` on pass rate, or matches it at lower tokens or time, beyond the paired intervals.
  - Otherwise the negative result is recorded and H1 stays off.
  - If `tail` does not beat `none`, that is reported too.
- **Spend:** about $1.50 estimated, under a strict client limit of $3, out of the $12.67 left of the $20 budget (ADR-028).
- **Results:** published in `benchmark/reports/`, as N2 was.

## Still the owner's

- The npm name and scope.
- The code-signing certificate (D3).
- Whether a Windows-only developer preview may come before signing.
- Reopening ADR-028 for PV-02.
- Account pools, if there are keys with independent quotas and terms that allow them.
- Paid runs beyond this A/B.
