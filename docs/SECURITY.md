# Security

> **Status:** Current · what Pygg protects, how, and how to report a problem · reviewed 2026-11-11 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

Pygg holds API keys, sends source code and prompts to providers, runs agent CLIs and the checks a user approved, and spends money. Everything runs on the user's machine (ADR-002).

## Reporting

Report a vulnerability privately: through the repository's Security tab (private vulnerability reporting) where it is enabled, or to the maintainer directly; not in a public issue. Include the version or commit, what you did and what happened. Do not include real keys or tokens.

## What is protected, and how

| Asset | Protection | Where |
| --- | --- | --- |
| Provider keys | Kept in the OS credential store (Windows Credential Manager, macOS Keychain); the store holds only a reference. A key goes into the core once and never comes back out to the interface. Keys are read from an environment variable by the headless CLIs, never from a command line | `vault.rs`, `commands.rs`, `bin/gateway.rs` |
| Client tokens | Shown once, stored as SHA-256 hashes; each token can be limited (money, requests in flight, connections it may use, providers that must not train on its data) and revoked | `gateway/store.rs` |
| The gateway | Listens on `127.0.0.1` only; refuses a `Host` that is not a loopback name (DNS rebinding); every request needs a client token | `gateway/mod.rs` |
| Spending | Atomic admission against a client's limit (an estimate, or the most a request could cost when the limit is strict); every attempt journaled before it is sent and settled exactly once; no paid request while the ledger cannot be written; unknown cost never shown as $0; a paid task step needs the user's approval with a cap, and there is no API to approve | `gateway/mod.rs`, `tasks.rs` |
| Where data goes | Per connection, data-use terms the user recorded; a client can require providers that do not train on its data, enforced on every route; a sensitive workspace asks before every agent step | `gateway/store.rs`, `tasks.rs` |
| The user's code | Agents work in a Git worktree of a recorded commit, never in the user's checkout; results are kept as a new branch or a patch only when the user says so | `isolation.rs` |
| Output and logs | Secrets are removed from agent and check output before it is stored or shown; notifications name no task or file | `redact.rs`, `shell.rs` |
| Sign-ins of agents | Agent CLIs run with their own sign-in; Pygg never reads, stores or forwards it, and never bridges web sessions, cookies or other products' OAuth (ADR-026) | `agents.rs` |

## What the user is trusted with

- **Checks run code.** An approved check runs the code an agent wrote, on this computer, with the user's rights. Detected checks are only suggestions until approved.
- **Agents can run commands** where their own tools allow it; Pygg gives them file tools by default, and shell only when the user allows it (Claude Code's Bash, Gemini's yolo mode). Codex keeps its own sandbox.
- **The data folder** holds prompts, ledger entries and agent output. It is the user's to protect, like any other local file.
- **`SMARTTASKROUTER_CAPTURE_DIR`**, a debugging aid, writes translated streams (whole model answers) to that folder. Leave it unset except when diagnosing.

## Not covered yet

- Signed builds and a signed updater (ROADMAP D3, D4).
- A security review of the agent CLIs themselves, which Pygg runs but does not control.
- Multi-user or remote use: Pygg assumes one user on one machine.
