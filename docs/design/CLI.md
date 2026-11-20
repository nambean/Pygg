# The `pygg` command line

> **Status:** Current · the `pygg` command and its npm launcher, as built · reviewed 2026-11-20 · built: [Implementation Status](../IMPLEMENTATION-STATUS.md) · npm: [packages/npm](../../packages/npm/pygg/README.md) · platforms: ADR-037 in [DECISIONS](../DECISIONS.md)

`pygg` is the console program `pygg-cli` (`apps/desktop/src-tauri/src/cli/`), installed from npm as `@pygg/cli`; same Rust core and store as the app. On Windows the app runs the gateway and connects providers; on macOS and Linux (no app yet) use `pygg gateway add-connection` and `pygg gateway serve`.

## Rules

- **Read commands only read.** `providers`, `models`, `logs`, `usage`, `status`, `doctor`, `diagnostics`, `setup --dry-run` open the store read-only and send nothing to providers. A missing, older or newer store is refused with what to do; nothing is created or migrated.
- **`--json` on every read command:** `schemaVersion: 1`, camelCase CLI output types. Never holds a credential reference, endpoint secret, prompt, session id or upstream error text.
- **No secrets on the command line.** Provider keys come from an environment variable (`--key-env VAR`); a new Pygg API key is never printed.
- **Strict options.** Unknown, missing-value or repeated option: usage error (exit 2) saying what to type. A mistyped command gets a suggestion (`Did you mean pygg status?`).
- **Port:** `--port`, else the store's saved port, else 8741. The health probe accepts only an HTTP success whose `app` is Pygg.
- **Store:** `--db PATH`, else `PYGG_DATA_DIR`, else the app's data folder (`%APPDATA%\app.smarttaskrouter.desktop`, `~/Library/Application Support/app.smarttaskrouter.desktop`, or `$XDG_DATA_HOME` / `~/.local/share` on Linux).
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
pygg chat [options]                   a conversation in the terminal; /model, /system, /set, /file, /new, /cost, /exit
pygg ask [options] QUESTION…          one question, the answer on stdout (also from a pipe)   [--json] [-x]
      options: -m MODEL, -s SYSTEM, -t TEMPERATURE, --max-tokens N, -f FILE…, --no-stream, -r (raw), --no-input,
               --save-dir DIR, --no-save (pictures, videos and files in an answer)
pygg default [MODEL | --clear]        the model chat and ask use without -m (a model, a combo, auto or an alias)

Providers and models
pygg providers                        connections: on, last check, health over 24 hours, models enabled/total
pygg providers presets                the kinds of connection Pygg knows
pygg providers add PRESET [--key-env VAR] [--label NAME] [--endpoint URL] [--billing TYPE]
pygg providers refresh | enable | disable CONNECTION
pygg providers remove CONNECTION --yes
pygg models [CONNECTION]              models to ask for, billing, price   [--search TEXT] [--all] [--limit N]
pygg models info CONNECTION/MODEL     price, tier, limits, tools, health over 24 hours, place in auto and auto/cheap
pygg models test CONNECTION/MODEL     one short question through the gateway
pygg models facts [ROLE]              your models ranked for a role from the model facts feed, each with its
                                      source and date, or its tier   [--all] [--json]
pygg models enable | disable CONNECTION/MODEL…
pygg route [auto|auto/cheap|auto/ROLE]
                                      the models the router would try, best first, and why the rest are left out
                                      [--key NAME] [--tools] [--images] [--prompt N]
pygg combos                           combos and their models
pygg keys [--all]                     API keys: start of the key, last used, spend this month, limit

See what Pygg has and does (read only; --json on each)
pygg status                           is the gateway answering, on which store, which schema   [--port N]
pygg doctor                           status, plus coding tools, agents found, providers needing attention
pygg logs                             recent request attempts, newest first, UTC   [--limit N] [--client NAME] [--failed]
pygg usage                            requests, known and unknown cost, token mix, by client and model   [--days 1|7|30]
pygg savings [--days N]               the saving estimate against the model chosen to compare with
pygg launches [--limit N]             pygg run launches: tool, repository, requests, cost, what changed in Git
pygg launches --by-repo [--weeks N]   per repository and UTC week: cost, commits, cost per commit, launches that
                                      changed nothing (observations, not productivity)

Advanced
pygg gateway <command> …              the gateway without the window (below)
pygg diagnostics --out PATH           the redacted diagnostics bundle (a zip)   [--port N]
pygg completion powershell|bash|zsh   Tab completion for the shell
pygg help COMMAND                     the same as pygg COMMAND --help
pygg --version                        the npm package's version

Experimental task integration (on hold)
pygg mcp setup [claude-code|codex|opencode]   register Pygg's MCP server in the tool   [--dry-run | --remove]
pygg mcp                              the MCP server itself (stdio), which the tool starts
pygg agent <command> …                agent CLIs: detect, run, task, quality
```

Every command takes `--db PATH`; `pygg <command> --help` says more.

### `setup` and `run`

- `setup claude`: the launcher's key goes in the OS credential store; no file changes. `codex`, `opencode`: back up the tool's config, add Pygg's provider. `--dry-run` writes nothing; `--remove` takes Pygg out and revokes the key. Details: [AGENTS.md](../AGENTS.md).
- Old names still work: `claude-launcher`, `codex-models`, `opencode-models`. `pygg setup claude-code` is refused with a pointer to `pygg mcp setup claude-code`.
- `run`: Pygg's options go **before** the tool name; everything after is the tool's (`pygg run claude --help` is Claude Code's help). `--model`, `--statusline`: `claude` only.
- `run` gives each launch its own routing session, checks set-up, gateway and tool before starting, returns the tool's exit code, and prints a receipt on stderr at exit: cost, cache, models, Git changes ([AGENTS.md](../AGENTS.md), "The session receipt").

### `chat` and `ask`

- Send Chat Completions to `/v1/chat/completions` with `stream: true`. After the answer, a stderr line: answering model (`x-pygg-route`), time, any fallback, ledger cost (`cost unknown`, or `on a plan` with no per-request price).
- Use their own API key, "Pygg CLI (ask)", made on first use and kept in the OS credential store (`cli.ask` in settings names it). If missing from the credential store, it is revoked and remade. Limits set like any client's.
- `--model`: `connection/model`, a combo, `auto`, `auto/cheap`, or a role alias (`auto/plan`, `auto/code`, `auto/test`, `auto/review`); default: `pygg default`, else `auto`.
- A chat is one routing session (`chat-<uuid>`), staying on the model that first answered. `/model M` starts a new session and resends the conversation; `/new` forgets it.
- Conversation is in memory only (ADR-028). A refused message is removed so it can be edited and resent. Every turn resends the whole conversation, so a web session (e.g. Muse) refuses once past its prompt limit.
- `ask` reads the question from arguments, else stdin when not a terminal. Exit: 0 answered, 1 no answer, 2 wrong usage.
- **Media in answers.** A web session's pictures, videos and files arrive as Markdown links after the text (`![image 1](URL)`, `[video 1](URL)`, `[file 1](URL)`; https or a picture's `data:` URL). They are saved to the current folder (`--save-dir DIR`) as `pygg-image1-<id>.<ext>` (extension from the address or server type), max 500 MB each, and the path is printed; `--no-save` prints the addresses instead. Downloads carry no site cookies, so a sign-in-only address is reported as not saved.

### Changing things from the terminal

- `providers add|refresh|enable|disable|remove`, `models enable|disable`, `default` open the store read-write, only at the schema this program knows, never migrating (`manage::open_write`). The running app and gateway read the store per request, so changes apply at once.
- `providers add`: key from `--key-env` only, kept in the credential store; lists the connection's models with models.dev facts, as Connect Provider does. Web-session presets are refused with a pointer to the app (it opens the site's sign-in window).
- `providers remove` needs `--yes`; deletes the connection's keys from the credential store; past requests stay in the ledger.
- `models info` reads only the store: price, tier, limits, inputs, 24-hour requests, place in `auto` and `auto/cheap` for a 1,000-token chat under the first API key's rules (`route` takes another key and needs).
- `models test`: one short question through the running gateway, as `ask` does.
- `models facts [plan|code|test|review|image]` (ADR-040): ranks enabled models for the role from the facts feed kept beside the store. Shows the winning layer (feed, else tier), source, place and row count, score, the row's agent and effort, `derived` for a near-role measure, and the row's date; rows over a year old are not ranking. Never fetches; the app and `pygg gateway facts` do.
- In the app only (or `pygg gateway` for a headless gateway): making, limiting and revoking API keys; combos; prices; tiers. `keys` never prints a secret.

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
  facts [--refresh yes] [--role R] [--limit N]   fetch the model facts feed when due (or now), verify it, and
                                         rank the connected models for a role (ADR-040)
  test-model --model CONNECTION/MODEL    one short request (a paid model costs a little)
  create-client --name NAME [--monthly-usd N]
  client --name NAME [--auto-min-tier T0-T4] [--paid-retries N] [--tasks yes|no] [--no-training yes|no]
         [--strict yes|no] [--monthly-usd N] [--session-usd N] [--day-usd N]
                         0 removes a limit; a session is the x-pygg-session a request names, a day a UTC day
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

- `gateway` commands open the store for writing and migrate it.
- `serve` refuses a store another gateway already serves. `--tasks yes` also runs smart tasks from allowed clients, for a computer where the app is not open on that store.
- `pygg agent` (`detect`, `run`, `task`, `quality`): see [AGENTS.md](../AGENTS.md).

## Output

- Aligned tables, no colour.
- Bare `pygg` answers in about a second (one loopback probe, a few reads) and ends with the next command.
- `models`: 50 rows by default (`--limit 0` for all); prices with units, or "no price (cost unknown)".
- `logs`: filters before the limit (1 to 500, default 20).
- `usage`: known and unknown cost kept apart.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Done |
| 1 | Failed (store not found, the gateway not answering for `run`, a write that failed) |
| 2 | Wrong usage (unknown command or option, missing value) |
| 3 | A check found a problem (`status`, `doctor`) |

`pygg run` returns the tool's own exit code once the tool has started.

## The npm launcher (`packages/npm`)

`@pygg/cli` is a Node launcher (`bin/pygg.js`, Node 18+) that runs native `pygg-cli` with the same arguments, stdin/stdout and exit code. Its own errors go to stderr only, keeping `pygg mcp`'s stdout clean. The native program is one of four optional dependencies; npm installs the matching one (ADR-037):

| Package | Platform |
| --- | --- |
| `@pygg/win32-x64` | Windows x64 |
| `@pygg/linux-x64` | Linux x64, glibc |
| `@pygg/darwin-arm64` | macOS, Apple silicon |
| `@pygg/darwin-x64` | macOS, Intel |

All four are published under tags `dev` and `latest`; versions in [PUBLISHING.md](../../packages/npm/PUBLISHING.md).

```text
npm install -g @pygg/cli@dev
pygg

npx --yes --package=@pygg/cli@dev pygg doctor
```

- **Lookup order:** `PYGG_CLI` (full path only), the platform package, then on Windows an installed app (`%LOCALAPPDATA%\Pygg\pygg-cli.exe`).
- `pygg --launcher-info` prints the launcher version, each place checked and the program it runs, without starting anything.
- **When nothing can run**, it says why: musl Linux (Alpine) needs a glibc image; Windows on Arm needs x64 Node; a package skipped by `--omit=optional` needs a reinstall; or set `PYGG_CLI` to a build from `cargo build --release --no-default-features --bin pygg-cli`.
- On macOS and Linux, a missing execute bit is restored.
- Developer preview, unsigned: SmartScreen, Gatekeeper or antivirus may warn.

**Verified:** unit tests in `cli/mod.rs`, `cli/read.rs`, `cli/setup.rs`, `cli/run.rs` and the launcher's `node --test`; published packages installed from the registry and run on Windows x64 and Linux x64 (Debian bookworm); macOS packages installed from tarballs and run by CI on macOS. **Not yet:** installed from the registry on a real Mac.

## Not built yet

`providers check`, `models refresh`, `logs --follow`, colour, a set-up wizard.
