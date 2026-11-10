# Implementation status

What exists in code today, what is designed but not built, and how it was verified. Updated with each change that moves a line.

## Built

| Area | State | Verified by |
| --- | --- | --- |
| Workspace | pnpm 9 workspace, `apps/desktop`, root `dev` / `build` / `typecheck` scripts | `pnpm install`, `pnpm build` |
| Desktop shell (Tauri) | Single-instance window, hidden until the page loads, traffic-light controls on Windows/Linux, `health_check` command | `cargo check` |
| Interface shell | Sidebar (task filters with counts, workspaces, Manage, App), toolbar with budget subtitle and popover, sample-data banner, keyboard shortcuts (`Ctrl+1…6`, `Ctrl+N`, `Ctrl+,`, `Ctrl+F`, `Ctrl+Shift+L`) | Browser preview at 1440×900, light and dark |
| Task list | Needs Attention pinned on All Tasks, rows with attempt sequence and honest cost text, search, per-filter empty states | Browser preview |
| Task detail (first cut) | Attempts with model, tier, billing and outcome | Browser preview |
| New Task (first cut) | Task text, strategy, workspace summary; Analyze is disabled until the engine exists | Browser preview |
| Settings (first cut) | Language (System, English, Tiếng Việt) | Browser preview |
| i18n | `t()` with English keys; Vietnamese for every current string | Browser preview in Vietnamese |
| Design tokens | DownFetch token set with orange accent, dark and increased-contrast variants | Browser preview |

All task data is sample data (`src/lib/sample.ts`) and is labeled as such on screen (ADR-014).

### Rust core (`apps/desktop/src-tauri/src`)

| Module | What it does | Verified by |
| --- | --- | --- |
| `db.rs` + `migrations/0001_init.sql` | SQLite store with versioned migrations in one transaction, WAL, foreign keys, refusal of newer schemas. Money is integer micro-USD; unknown cost cannot be stored as an amount | 5 unit tests |
| `vault.rs` | Provider credentials in the OS credential store; the database keeps only a reference | Unit test, plus a round trip against Windows Credential Manager (`cargo test -- --ignored`) |
| `redact.rs` | Masks known secrets and common API-key shapes in output | 5 unit tests |
| `process.rs` | Supervised child processes: time limit, cancellation, bounded output, whole-tree kill (`taskkill /T` on Windows, process group on Unix), no console windows | 5 unit tests |
| `isolation.rs` | A Git worktree per attempt from a recorded base, change stats, diff, **Keep as Branch** without touching the user's checkout, discard | 4 unit tests against real Git repositories |
| `checks.rs` | Detects checks from `package.json` (npm/pnpm/yarn) and `Cargo.toml`, runs one with redacted output | 3 unit tests |
| `bench.rs` + `bin/bench.rs` | Phase 0 harness: cases, rungs, single and cascade runs, JSONL results, summary | Unit test with a fake agent that climbs T0 → T1; end-to-end CLI run on both seed cases with a no-op agent |

`cargo test`: 26 passed, 1 ignored (the OS credential store test, which passes when run with `--ignored`).

### Benchmark

There are two seed cases in `benchmark/cases`: `js-sum-sign-bug` (bug fix, low) and `js-slugify-vietnamese` (feature, medium). Both checks were verified to fail on the fixture as given and to pass with a reference solution.

## Designed, not built

Connections, Policies, Usage, Endpoints, Workspace Settings, Add Workspace, the task Plan/Run/Result phases, the Setup Assistant and the approval sheet. Each is specified in [design/SCREENS.md](design/SCREENS.md). Their sidebar entries open a "not built yet" page.

## Not started

Gateway, provider adapters, router, agent adapters, the Tauri commands that connect the core to the interface, tray and notifications. The remaining 18–38 benchmark cases, and any benchmark run against real models (which needs an agreed budget).

## Not verified yet

- The native window on Windows: a debug build started, showed its "SmartTaskRouter" window and kept running for 8 seconds, but nobody looked at it. The traffic-light controls, dragging and the hidden-until-loaded start have only been checked in the browser preview.
- The D12 checks for traffic-light controls (snap layouts, scaling, screen readers).
