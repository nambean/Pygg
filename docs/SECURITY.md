# Security

> **Status:** Current · what Pygg protects, what the user is trusted with, how to report a problem · reviewed 2026-11-21

Pygg holds API keys, sends code and prompts to providers, runs agent CLIs and approved checks, and spends money, all on the user's machine (ADR-002).

## Reporting a vulnerability

- Report privately with [Report a vulnerability](https://github.com/nambean/Pygg/security/advisories/new) on the Security tab, never in a public issue.
- Include the version or commit, what you did and what happened; never real keys or tokens.

## Protections

| What | Rule |
| --- | --- |
| Gateway | `127.0.0.1` only; non-loopback `Host` refused; every route but `/health` needs a Pygg API key |
| User's code | Agents work in a Git worktree, never the user's checkout; a result becomes a branch or patch only when the user says so |
| Output and logs | Secrets redacted from agent and check output before storing or showing; notifications name no task or file |
| Where data goes | A key can require providers that do not train on its data; a sensitive workspace asks before every agent step |
| Provider keys | In the OS credential store (the store keeps a reference); never returned to the interface; CLI reads them from an environment variable |
| Endpoints | `https://` only; `http://` only to this computer or the local network |
| Pygg API keys | Stored as a SHA-256 hash and prefix; app-made keys keep a copy for **Show**; limits per key; revocable |
| Spending | Atomic admission against a key's limit; every attempt journaled before sending; no paid request while the ledger cannot be written; unknown cost never $0 |
| Paid task steps | Need the user's approval with a cap in the app; no API approves one |
| Provider sign-in | Only OAuth that issues an API key (OpenRouter PKCE), callback on `localhost` with state check; no cookie or website session kept (ADR-034, ADR-026) |
| Agent sign-ins | Agent CLIs keep their own sign-in; Pygg never reads, stores, forwards or bridges it (ADR-026) |
| Diagnostics bundle | Saved only on request, never sent; allowlisted fields, no keys, tokens, prompts, output, diffs or file contents; redacted |
| Usage history | Kept until the user picks 30, 90 or 365 days; task-cost and this month's rows kept |
| Outbound traffic | Only connected providers, plus a daily keyless GET to [models.dev](https://models.dev) (ADR-031) |
| Prompt cache | Adds a Claude `cache_control` marker and a hashed session key for OpenRouter/OpenAI (ADR-036); the ledger keeps only sizes and fingerprints |
| Skill folders | Shows names and token estimates, never text; installs one `SKILL.md`; never overwrites what it did not write; never downloads or runs anything (ADR-039) |
| Claude Code launcher | Never changes Claude Code's settings; sets the gateway in one process's environment; key in the credential store; Remove revokes it |
| Web sessions | See [Web sessions](#web-sessions) (ADR-035) |

### Web sessions

Sites with no API (Muse, ChatGPT, Perplexity, DeepSeek, Grok, Gemini), used through a Pygg window the user signs in to (ADR-035).

| What | Rule |
| --- | --- |
| Profile | One WebView2 profile per connection; no site sees another's cookies or sign-in |
| Passwords and cookies | Never asked for, stored, imported or read |
| Page scripts | Only fill the prompt, press Send, read the reply and media, click the site's **deny** on agent actions, and Muse's **Play** (muted) |
| Where Pygg types | Only on the site's own https host; elsewhere the connection is marked Needs you |
| Navigation | Any https address while the user signs in; otherwise only the recipe's hosts ([PROVIDERS](PROVIDERS.md)) |
| Pace | Guard always on: one request at a time, rests, size-weighted limits, long prompts refused, back-off; limit → 429 with Retry-After ([numbers](PROVIDERS.md#the-guard)) |
| No disguise | No fingerprint changes or random timing; never solves or skips a CAPTCHA or bot check |
| Isolation | Window has no Tauri capability; gateway reaches it on a random loopback port with an in-memory token |
| Page text | Pygg's own wording, except the signed-in account name (one line, cleared on sign-out) |
| Media | Returned as Markdown links; never keeps cookies, headers or the site's API answers; at most 100 MB |
| Debugging | DevTools and debug port only in debug builds with `PYGG_WEB_DEVTOOLS` |
| Lifecycle | Needs the user's risk acknowledgement; starts off; refuses tools; never chosen by `auto`; removal deletes the profile |

## What the user is trusted with

- **Checks run code**: an approved check runs agent-written code with the user's rights.
- **Agents can run commands**: a shell only when the user allows it; Codex keeps its own sandbox.
- **The data folder** holds prompts, ledger, agent output and web-session profiles; protect it like any local file.
- **A web session acts as the user**: the site may limit or ban the account; accepted per connection (ADR-035).
- **Key rings**: the user confirms the keys belong to separate accounts (ADR-038).
- **`PYGG_CAPTURE_DIR`** writes whole model answers to disk; leave it unset except when diagnosing.

## Not covered yet

- Signed builds and updater ([ROADMAP](ROADMAP.md)).
- A security review of the agent CLIs.
- Multi-user or remote use.
