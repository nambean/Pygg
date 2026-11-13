# Production readiness, 2026-11-14

> **Status:** Current · an assessment before the open-source release · independent review: Codex CLI 0.163.0, read-only, of the source, docs, CI and packages · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

**Verdict.** Pygg is ready for a **public developer preview on Windows x64**, not yet for a production claim. Its accounting design (attempts journaled before they are sent, atomic budget admission, crash recovery as unknown-cost entries, unknown never shown as $0) holds up. Recovery, bounded resource use, distribution assurance and release validation need more work.

| Area | Codex's assessment | Since then |
| --- | --- | --- |
| Correctness under failure | Not ready: unbounded response bodies, a client that stops reading holds an attempt for ever, no corrupt-store path, no fault tests | Bodies capped at 64 MiB (`read_body`); a streamed piece waits at most 2 minutes for the client (`deliver`), then the client counts as gone; a store that cannot be opened is logged with where its daily copies are. Fault-injection tests still to do |
| Money | Ready with caveats: a strict limit reserves an *estimate* of the most a request could cost, not a cap on the provider's invoice; known $0 depends on the billing type being right | Wording says so in Policies and the key sheet ("the bill can differ"); README's verdict says it |
| Security | Ready with caveats: loopback, Host checks, Credential Manager and an allowlisted diagnostics bundle are sound; CSV exports carry provider error text; web sessions stay experimental | Docs corrected (`/health` needs no key; web sessions record $0 by default) |
| Operability | Not ready: no app log, no store backup, no recovery procedure, no updater | **App log** `logs/pygg.log`, redacted, rotated at 1 MB keeping two, panics included, its last 200 lines in the diagnostics bundle (`applog.rs`). **Daily store copies** `backups/store-<date>.sqlite3` with `VACUUM INTO`, newest three kept, restore by copying one back (`backup.rs`). No updater yet |
| Distribution | Not ready: unsigned installers, no clean-machine test, MSI scope and WebView2 policy not checked, no complete dependency licence inventory | Owner's steps below |
| Performance | Not ready for a claim: no measured overhead, soak or large-ledger figures | Benchmarked for tokens, not latency yet |
| Platform | Ready with caveats for Windows x64 only | README and npm say Windows x64 only |
| Tests and CI | Ready with caveats: 307 Rust unit and end-to-end tests with a fake provider, CI on Windows; no packaged-artifact, native-UI or fault tests | CI added 2026-11-14 |

## Release blockers left

1. **Code signing** of the installers and the CLI (needs a certificate: the owner's decision and cost).
2. **Clean-machine tests** of the signed NSIS and MSI installers: install, upgrade over the previous version, uninstall, WebView2 present and absent, per-user scope.
3. **A packaged smoke test** of the exact npm tarballs and installers being released (the steps in `packages/npm/PUBLISHING.md`).
4. **Fault-injection tests**: process kill mid-request, disk full during settlement, a locked store, a corrupt store at start-up.
5. **A dependency licence inventory** generated from Cargo and npm and shipped with the installer.
6. **Web sessions stay outside any production guarantee**: opt-in, unofficial, an account-risk warning, and the guard (ADR-035).

## First month after release

- Measure gateway overhead, concurrent traffic, slow readers, memory over days, 900+ models and large ledgers; publish the limits.
- Reconcile ledger costs against providers' own usage reports for a sample.
- A signed update channel, or a documented manual update with schema compatibility.
- Keyboard and Narrator passes of the critical flows; the "Later" list of the [UI review](../reviews/2026-11-14-codex-ui-review-all-screens.md).
- Dependency audit and release provenance in CI.

## For the README

> Pygg is a Windows x64 developer preview for local AI routing. Its gateway journals every attempt before sending it, reserves spending atomically, recovers after a crash and keeps keys in the Windows Credential Manager, and the main provider and coding-tool paths have been verified live. It is not yet recommended for unattended or financially critical use: spending limits rely on prices and token estimates, installers are unsigned, and clean-machine installation and long-running behaviour need more validation. Web sessions are optional, unofficial and experimental; automated use can lead to account restrictions.
