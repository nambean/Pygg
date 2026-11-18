# The `pygg` command line

> **Status:** Current · the `pygg` command and its npm launcher, as built · reviewed 2026-11-14 · built: [Implementation Status](../IMPLEMENTATION-STATUS.md) · npm: [packages/npm](../../packages/npm/pygg/README.md) · platforms: ADR-037 in [DECISIONS](../DECISIONS.md)

`pygg` is the console program `pygg-cli` (`apps/desktop/src-tauri/src/cli/`), installed from npm as `@pygg/cli`. It runs the same Rust core and store as the app. On Windows the desktop app runs the gateway and is where providers are connected; on macOS and Linux, where there is no app yet, `pygg gateway add-connection` and `pygg gateway serve` do it from the terminal.

## Rules

- **Read commands only read.** `providers`, `models`, `logs`, `usage`, `status`, `doctor`, `diagnostics` and `setup --dry-run` open the store read-only. A missing, older or newer store is refused with what to do; nothing is created or migrated. They send nothing to any provider.
- **`--json` on every read command.** The document has `schemaVersion: 1` and uses the CLI's own output types (camelCase). It never holds a credential reference, endpoint secret, prompt, session id or upstream error text.
- **Nothing secret on the command line.** A provider key comes from an environment variable (`--key-env VAR`). A new Pygg API key is never printed.
- **Strict options.** An unknown option, a missing value or a repeated option is a usage error (exit 2) that says what to type. A mistyped command gets a suggestion (`Did you mean pygg status?`).
- **One port rule:** `--port`, then the port saved in the store, then 8741. The health probe accepts only an HTTP success whose `app` is Pygg.
- **One store rule:** `--db PATH`, else `PYGG_DATA_DIR`, else the app's data folder (`%APPDATA%\app.smarttaskrouter.desktop`, `~/Library/Application Support/app.smarttaskrouter.desktop`, or `$XDG_DATA_HOME` / `~/.local/share` on Linux).
- **Errors** go to stderr and end with the next thing to try.

## Commands

```text
pygg                                  where you are: gateway, providers, tools set up, and what to type next

Use coding tools through Pygg
pygg setup                            the tools: installed, Pygg set-up, the next command
pygg setup claude|codex|opencode      route the tool through Pygg   [--dry-run | --remove] [--db PATH]
pygg run [--model M] [--statusline] [--db PATH] claude|codex|opencode [the tool's own arguments…]
pygg statusline                       the launch's cost in one line, for Claude Code's status line   [--db PATH]

Talk to any connected model through Pygg's router
pygg chat [--model M] [--db PATH]     a conversation in the terminal; /model M, /new, /cost, /exit; nothing kept
pygg ask [--model M] [--db PATH] QUESTION…   one question, the answer on stdout (also from a pipe)

See what Pygg has and does (read only; --json on each)
pygg status                           is the gateway answering, on which store, which schema   [--port N]
pygg doctor                           status, plus coding tools, agents found, providers needing attention
pygg providers                        connections: on, last check, health over 24 hours, models enabled/total
pygg models [CONNECTION]              models to ask for, billing, price   [--search TEXT] [--all] [--limit N]
pygg logs                             recent request attempts, newest first, UTC   [--limit N] [--client NAME] [--failed]
pygg usage                            requests, known and unknown cost, token mix, by client and model   [--days 1|7|30]

Advanced
pygg gateway <command> …              the gateway without the window (below)
pygg diagnostics --out PATH           the redacted diagnostics bundle (a zip)   [--port N]
pygg --version                        the npm package's version, such as 0.1.0-dev.2

Experimental task integration (on hold)
pygg mcp setup [claude-code|codex|opencode]   register Pygg's MCP server in the tool   [--dry-run | --remove]
pygg mcp                              the MCP server itself (stdio), which the tool starts
pygg agent <command> …                agent CLIs: detect, run, task, quality
```

Every command takes `--db PATH`; `pygg <command> --help` says more.

### `setup` and `run`

- `pygg setup claude` keeps the launcher's key in the OS credential store; no file changes. `codex` and `opencode` back up the tool's configuration, then add Pygg's provider. `--dry-run` writes nothing; `--remove` takes Pygg out and revokes the key. Details: [AGENTS.md](../AGENTS.md).
- Older names still work: `claude-launcher`, `codex-models`, `opencode-models`. `pygg setup claude-code` is refused with a pointer to `pygg mcp setup claude-code`.
- `pygg run`: Pygg's options go **before** the tool's name; everything after it is the tool's, `--help` included (`pygg run claude --help` is Claude Code's help). `--model` and `--statusline` are for `claude` only. Each launch gets its own routing session. `run` checks the set-up, the gateway and the tool before starting, then returns the tool's own exit code. When the tool exits it prints the launch's receipt on stderr: cost, cache, models, and what changed in Git ([AGENTS.md](../AGENTS.md), "The session receipt").

### `chat` and `ask`

- Both send Chat Completions requests to the gateway's `/v1/chat/completions` with `stream: true`, so the answer is printed as it arrives, then a line on stderr: the model that answered (`x-pygg-route`), the time, a fallback if there was one, and the cost the ledger knows (`cost unknown` or `on a plan` where there is no price per request).
- They use an API key of their own, "Pygg CLI (ask)", made at first use and kept in the OS credential store (`cli.ask` in settings names it); a key missing from the credential store is revoked and made again. Its limits are set like any client's.
- `--model` takes `connection/model`, a combo, `auto` or `auto/cheap` (default `auto`). A chat is one routing session (`chat-<uuid>`), so it stays on the model that first answered; `/model M` starts a new session for the next messages and sends the conversation so far with them. `/new` forgets the conversation.
- The conversation is held in memory only (ADR-028). A message the gateway refused is taken out of it, so it can be changed and sent again. Every turn sends the whole conversation, so a web session such as Muse refuses a chat once it passes the site's prompt limit.
- `ask` reads the question from its arguments, or from stdin when there are none and stdin is not a terminal. Exit codes: 0 answered, 1 no answer, 2 wrong usage.

### `gateway` (advanced)

```text
pygg gateway [--db PATH] <command>
  serve [--port N] [--tasks yes]
  add-connection --preset KIND [--label NAME] [--key-env VAR] [--billing TYPE] [--endpoint URL]
  remove-connection --id ID
  key --id CONNECTION --label NAME --key-env VAR --separate-account yes [--strategy S]
                         add another account's key to a connection's key ring (ADR-038);
                         S: in_order, round_robin, least_used, quota_aware
  refresh-models --id ID
  lookup [--refresh yes]                 fill model facts from the models.dev catalog
  test-model --model CONNECTION/MODEL    one short request (a paid model costs a little)
  create-client --name NAME [--monthly-usd N]
  client --name NAME [--auto-min-tier T0-T4] [--paid-retries N] [--tasks yes|no] [--no-training yes|no]
         [--strict yes|no] [--monthly-usd N]
  combo --name NAME --member CONNECTION/MODEL [--member …]
  model --model CONNECTION/MODEL [--tier T0-T4] [--context N] [--max-output N] [--tools yes|no] [--vision yes|no]
        [--price-in USD] [--price-out USD] [--price-cached USD] [--price-cache-write USD] [--price-source WHERE]
        [--billing TYPE]                 prices in USD per million tokens
  terms --id CONNECTION --terms no_training|may_train|unknown [--source WHERE]
  auto --alias auto|auto/cheap [--client NAME] [--anthropic yes] [--tools yes] [--prompt N] [--max-tokens N]
       [--thinking adaptive|enabled] [--effort low|medium|high|xhigh|max]
  list
  usage
```

Unlike the read commands, `gateway` commands open the store for writing and migrate it. `serve` refuses a store another gateway already serves. `--tasks yes` also runs smart tasks from clients allowed to start them, for a computer where the app is not open on that store.

`pygg agent` (`detect`, `run`, `task`, `quality`) is described in [AGENTS.md](../AGENTS.md).

## Output

Tables with aligned columns and no colour. A bare `pygg` answers in about a second (one loopback probe and a few reads) and ends with the next command. `models` shows 50 rows by default (`--limit 0` for all) and a price with its units, or "no price (cost unknown)". `logs` filters before the limit (1 to 500, 20 by default). `usage` keeps known and unknown cost apart.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Done |
| 1 | Failed (store not found, the gateway not answering for `run`, a write that failed) |
| 2 | Wrong usage (unknown command or option, missing value) |
| 3 | A check found a problem (`status`, `doctor`) |

`pygg run` returns the tool's own exit code once the tool has started.

## The npm launcher (`packages/npm`)

`@pygg/cli` is a Node launcher (`bin/pygg.js`, Node 18 or later) that runs the native `pygg-cli` with the same arguments, standard input and output, and exits with its code. Its own errors go to stderr only, so `pygg mcp` keeps a clean stdout. The native program is one of four optional dependencies, of which npm installs only the one matching the computer (ADR-037):

| Package | Platform | Published |
| --- | --- | --- |
| `@pygg/win32-x64` | Windows x64 | `0.1.0-dev.2`, tag `dev` |
| `@pygg/linux-x64` | Linux x64, glibc | `0.1.0-dev.2`, tag `dev` |
| `@pygg/darwin-arm64` | macOS, Apple silicon | `0.1.0-dev.2`, tag `dev` |
| `@pygg/darwin-x64` | macOS, Intel | `0.1.0-dev.2`, tag `dev` |

```text
npm install -g @pygg/cli@dev
pygg

npx --yes --package=@pygg/cli@dev pygg doctor
```

- The program is found in this order: `PYGG_CLI` (a full path only), the platform package, then on Windows an installed Pygg app (`%LOCALAPPDATA%\Pygg\pygg-cli.exe`).
- `pygg --launcher-info` prints the launcher's version, each place it looked and the program it runs, without starting anything.
- Where nothing can run, the launcher says why and what to do: musl Linux (Alpine) needs a glibc image; Windows on Arm needs the x64 build of Node; a package skipped by `--omit=optional` needs a reinstall; or set `PYGG_CLI` to a program built with `cargo build --release --no-default-features --bin pygg-cli`.
- On macOS and Linux, a program unpacked without its execute bit gets it back.
- It is a developer preview and unsigned: Windows SmartScreen, macOS Gatekeeper or an antivirus may warn.
- Publishing: [packages/npm/PUBLISHING.md](../../packages/npm/PUBLISHING.md).

Verified: unit tests in `cli/mod.rs`, `cli/read.rs`, `cli/setup.rs`, `cli/run.rs` and the launcher's `node --test`; `0.1.0-dev.2` installed back from the registry and run on Windows x64 and Linux x64 (Debian bookworm); the macOS packages installed back from their tarballs and run by CI on macOS. Not yet installed from the registry on a real Mac.

## Not built yet

`providers check|add|remove`, `models refresh|test` and `keys` outside `pygg gateway`, `logs --follow`, shell completion, colour, and a set-up wizard.
