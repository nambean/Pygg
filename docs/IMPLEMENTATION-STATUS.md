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

## Designed, not built

Connections, Policies, Usage, Endpoints, Workspace Settings, Add Workspace, the task Plan/Run/Result phases, the Setup Assistant and the approval sheet. Each is specified in [design/SCREENS.md](design/SCREENS.md). Their sidebar entries open a "not built yet" page.

## Not started

Rust core (SQLite, vault, isolation, process supervision), benchmark harness, gateway, provider adapters, router, agent adapters, tray and notifications.

## Not verified yet

- The native window on Windows: `cargo check` passes, but `tauri dev` has not been run in this environment.
- The D12 checks for traffic-light controls (snap layouts, scaling, screen readers).
