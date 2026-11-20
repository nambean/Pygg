# Security

> **Status:** Current · what Pygg protects and how, what the user is trusted with, how to report a problem · reviewed 2026-11-21

Pygg holds API keys, sends code and prompts to providers, runs agent CLIs and approved checks, and spends money. Everything runs on the user's machine (ADR-002).

## Reporting a problem

- Report privately with [Report a vulnerability](https://github.com/nambean/Pygg/security/advisories/new) on the repository's Security tab, never in a public issue.
- Include the version or commit, what you did and what happened. Never include real keys or tokens.

## What Pygg protects, and how

| Asset | Rule | Code |
| --- | --- | --- |
| The gateway | Listens on `127.0.0.1` only; refuses a `Host` that is not a loopback name (DNS rebinding); every API route needs a Pygg API key except `/health`, which says only that Pygg is running | `gateway/mod.rs` |
| The user's code | Agents work in a Git worktree of a recorded commit, never in the user's checkout; a result becomes a branch or patch only when the user says so | `isolation.rs` |
| Output and logs | Secrets are removed from agent and check output before it is stored or shown; notifications name no task or file | `redact.rs`, `shell.rs` |
| Where data goes | Per connection, the data-use terms the user recorded; a key can require providers that do not train on its data, enforced on every route; a sensitive workspace asks before every agent step | `gateway/store.rs`, `tasks.rs` |
| Signing in with a provider | Only a provider's own OAuth that issues an API key (OpenRouter's PKCE); callback listens on `localhost` only and checks the state; the key goes straight to the credential store; no cookie or website session is read or kept (ADR-034, ADR-026) | `gateway/oauth.rs` |
| Sign-ins of agents | Agent CLIs run with their own sign-in; Pygg never reads, stores or forwards it, and never bridges cookies or other products' OAuth (ADR-026) | `agents.rs` |
| Web sessions (ADR-035) | See [Web sessions](#web-sessions) | `websession/mod.rs`, `websession/guard.rs`, `websession/window.rs`, `gateway/store.rs`, `commands.rs` |

### Provider keys (`vault.rs`, `commands.rs`, `cli/gateway.rs`)

- Kept in the OS credential store: Windows Credential Manager; the macOS Keychain or the desktop keyring for the command line.
- The store holds only a reference, one per key when a connection holds keys of several accounts.
- A key goes into the core once and never comes back out to the interface.
- The command line reads keys from an environment variable, never from its arguments.
- Endpoints must be `https://`; `http://` only to this computer or the local network (loopback, private and link-local addresses, names without a dot, `.local`, `.lan`, `.internal`). A key never crosses the internet unencrypted.

### Pygg API keys (client tokens) (`gateway/store.rs`, `commands.rs`, `vault.rs`)

- The store keeps only a fingerprint: the key's SHA-256 hash and its first 12 characters (to tell keys apart).
- Keys created in Endpoint → API Keys, and the Claude Code launcher's key, also keep a copy in the OS credential store, which **Show** reads back.
- Keys made by `pygg gateway create-client` or a tool's one-click set-up (token written into that tool's own config file) have no copy and are shown once.
- Revoking a key deletes its copy. Deleting a revoked key removes it from the list; its past requests keep its name.
- Each key can be limited (money, requests in flight, allowed connections, providers that must not train on its data) and revoked.
- **Defaults for new keys** (Policies) apply to every way a key is created; a limit given at creation wins; existing keys keep theirs.
- Not verified yet: reading a copy back from the real Credential Manager (no automated test).

### Spending (`gateway/mod.rs`, `tasks.rs`)

- Admission against a key's limit is atomic: an estimate, or the most a request could cost when the limit is strict.
- Every attempt is journaled before it is sent and settled exactly once.
- No paid request goes out while the ledger cannot be written.
- An unknown cost is never shown as $0.
- A paid task step needs the user's approval with a cap. There is no API to approve one.

### The diagnostics bundle (`diagnostics.rs`, `commands.rs`, `cli/mod.rs`)

- Saved only when the user asks (Settings → Diagnostics, or `pygg diagnostics --out PATH`); Pygg never sends it anywhere.
- Built from an allowlist: app, schema and Windows versions; gateway state and port; per connection its kind, endpoint (no user name, query or fragment), enabled state, billing, terms, health and whether a key is set; key names and limits; row counts per table; the latest 200 ledger rows without the provider's error text or session key; task and step states.
- Never reads: provider keys, client tokens or their hashes and prefixes, connection labels, prompts, task titles or reasons, agent or check output, diffs, file contents.
- Every text value is cut to 200 characters and redacted; each file is redacted again.
- Verified by a test: a bundle built from a store holding a fake key, a token and a prompt contains none of them.

### Usage history (`retention.rs`, `commands.rs`)

- Kept until the user picks 30, 90 or 365 days (Settings → Usage History).
- Then older ledger rows and route decisions are deleted at start-up and once a day, after a confirmation that says how many go.
- Kept anyway: rows a task's cost is counted from, and this month's rows (key limits count them). A deletion never lowers a task's cost or frees room under a monthly limit.
- Deleted rows stay on disk until SQLite reuses the space.

### What leaves the machine besides client traffic (`gateway/reference.rs`, `gateway/mod.rs`, `gateway/shadow.rs`)

- Pygg contacts only the providers the user connected (a web session's window loads its site, and wherever the user goes while signing in).
- At most once a day, a plain GET to [models.dev](https://models.dev) for its public model catalog (ADR-031), carrying no key, token or prompt.
- Added to requests, for the provider's prompt cache (ADR-036): a top-level `cache_control` on an agent's turn to Claude, and a session key for OpenRouter (`x-session-id`) and OpenAI (`prompt_cache_key`).
- The session key is `pygg-` plus a SHA-256 of the client and its session id; it never carries the id as sent, a key or prompt content.
- The ledger keeps the estimated size and a 64-bit fingerprint of each request's tools and system prompt, never their text.

### Agents' skill folders (`skills.rs`, ADR-039)

- Skills & Prompts reads the agents' global `CLAUDE.md`, `AGENTS.md` and skill, command and prompt folders, and shows names, paths and token estimates, never the files' text.
- Installing writes one `SKILL.md` into `~/.claude/skills` or `~/.agents/skills`.
- Pygg never overwrites a folder it did not write, and never changes or removes a copy edited since it wrote it (a hash in the file says).
- Pygg never downloads, unpacks or runs anything. A skill's instructions reach a model only when the agent uses it.

### The Claude Code launcher (`launcher.rs`, `cli/run.rs`, `process.rs`)

- Claude Code's own settings and sign-in are never changed.
- `pygg run claude` sets the gateway's address, the launcher's Pygg API key, the model (`auto` unless `--model`) and a fresh session header in **that one process's environment only**.
- The key ("Claude Code (launcher)") lives in the Windows Credential Manager, never in a file, and is read at each launch; `pygg run` opens the store read-only.
- Before launch it removes `ANTHROPIC_API_KEY`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX` and the host session's variables (`process::HOST_SESSION_VARS`).
- Remove (CLI Tools, or `pygg setup claude --remove`) revokes the key and deletes it from the Credential Manager.

### Web sessions

A site with no API (Muse, ChatGPT, Perplexity, DeepSeek, Grok, Gemini) used through a Pygg window the user signs in to (ADR-035). Code: `websession/mod.rs`, `websession/guard.rs`, `websession/window.rs`, `gateway/store.rs`, `commands.rs`.

| Topic | Rule |
| --- | --- |
| Profile | One WebView2 profile folder per connection; no site sees another connection's cookies, storage or sign-in. WebView2 keeps the sign-in there, encrypted for the Windows user |
| Passwords and cookies | Never asked for, stored, imported or read |
| Page scripts | Only fill the prompt box, press Send, read the reply and its media, click the site's own **deny** on an agent's request to act, and on Muse press **Play** (tests `no_script_reads_cookies_storage_or_the_network`, `every_page_script_is_valid_javascript`) |
| Where Pygg types | Only on the site's own host over https (sign-in hosts such as Google do not count); anywhere else it sends nothing and marks the connection Needs you (test `pygg_types_only_on_the_sites_own_page`) |
| Where the window may go | Any https address while the user has it open to sign in; otherwise only the recipe's hosts ([PROVIDERS.md](PROVIDERS.md)) |
| What is recorded | Refused addresses, every popup (all denied) and non-https addresses, by host or scheme only |
| A person's pace | The guard: one request at a time per account, a rest after each reply, limits per 10 minutes, hour and day weighted by prompt size, long prompts refused, back-off on trouble, connection turned off when the site says the account is suspended ([numbers](PROVIDERS.md#the-guard)). At a limit the gateway answers 429 with Retry-After and nothing reaches the site |
| No disguise | Never changes the browser fingerprint (user agent, client hints, canvas, fonts) or randomises timing. Never solves, skips or hides from a CAPTCHA, sign-in page or bot check: the connection stops and the user is told to deal with it in the window |
| Debugging | DevTools, remote-debugging port 9462 and a navigation-decision log exist only in debug builds with `PYGG_WEB_DEVTOOLS` set; the log prints each address in full to the console, nothing from the page. Release builds have neither |
| Kept apart from Pygg | The window has no Tauri capability, so the site cannot call Pygg's commands. The gateway reaches it through a loopback endpoint on a random port, behind an in-memory-only token |
| Text from the page | Errors and stored sign-in state use Pygg's own wording. Only exception: the signed-in account (email or name) where the page shows it without a click (ChatGPT's start-up data; Perplexity, Grok, DeepSeek sidebars), read at each sign-in check, one line of at most 120 characters, shown in the app, cleared on sign-out |
| Profile folder name | Never in the ledger, diagnostics or exports |
| Lifecycle | Needs the user's acknowledgement of the account risk; starts off; refuses tools; never chosen by `auto`. Removing the connection closes its window and deletes its folder; a folder still held is deleted at the next start, before any window opens |
| Verified | Tests in `websession/tests.rs` with a scripted page; live on Muse, ChatGPT, Perplexity, DeepSeek and Grok; Gemini's sign-in, a text answer and a picture |

**Media that comes with a reply** (pictures, videos, sounds, documents) is returned after the text as Markdown links:

- Pictures and videos: taken by the address that appeared after Send. A `blob:` one (as Muse shows them) is copied from the page's memory with `fetch`, or, where the page refuses (Gemini), drawn from the shown picture; at most 100 MB. Download links are taken by address.
- Responses the page receives (WebView2 response events): a sound or document is kept in memory until the reply takes it. One with no body from WebView2 (a download, `Content-Disposition: attachment`, as ChatGPT sends its files) is fetched again by the page, only on the page's own origin.
- Never kept: pictures from responses, the site's API answers, requests, cookies, headers. No address named in the site's traffic is downloaded.
- Blobs (Muse receives content over a WebSocket): found through the DevTools protocol on Pygg's own WebView2 (`CallDevToolsProtocolMethod`: `Runtime.queryObjects` on `Blob.prototype`; no debugging port, nothing outside the app attaches). Only sounds and documents of 1 KB to 100 MB new since the prompt are read, with the page's FileReader, as data URLs; handles are released.
- Muse only (`Recipe::play_audio`): Pygg presses **Play** once on each audio player in the new reply, window always muted, pauses once the player has a new address, and copies the sound from that `blob:` address. Nothing else is clicked; no request is changed or made.
- The command line, not the app, downloads https addresses, without the site's cookies.
- After each reply the app log notes what the page held and received by count, address kind, type, host and size only.

## What the user is trusted with

- **Checks run code.** An approved check runs agent-written code on this computer with the user's rights. Detected checks are suggestions until approved.
- **Agents can run commands** where their tools allow. Pygg gives file tools by default and a shell only when the user allows it (Claude Code's Bash, Gemini's yolo mode). Codex keeps its own sandbox.
- **The data folder** holds prompts, ledger entries, agent output and each web session's profile folder with its sign-in. The user protects it like any local file.
- **A web session's window is a browser while the user signs in.** Whatever the user opens then is kept in that connection's profile folder.
- **A web session acts as the user on the site.** The site's terms may forbid automation and the account can be limited or banned; the user accepts this per connection (ADR-035). For an agent like Muse, what a client sends is sent from the user's account, though Pygg denies every action the agent asks to take.
- **Keys of several accounts on one connection.** Pygg cannot check that a key ring's keys belong to separate accounts; the user confirms it (ADR-038).
- **`PYGG_CAPTURE_DIR`** (debugging) writes translated streams (whole model answers) to that folder. Leave it unset except when diagnosing.

## Not covered yet

- Signed builds and a signed updater: installers and npm packages are unsigned ([ROADMAP.md](ROADMAP.md)).
- A security review of the agent CLIs, which Pygg runs but does not control.
- Multi-user or remote use: Pygg assumes one user on one machine.
