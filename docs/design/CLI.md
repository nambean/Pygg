# The `pygg` command line

> **Status:** Current · built, unit-tested; npm packages run from the registry on Windows x64 and Linux x64, on macOS by CI only · built: [Implementation Status](../IMPLEMENTATION-STATUS.md) · platforms: ADR-037 in [DECISIONS](../DECISIONS.md)

`pygg` is the console program `pygg-cli` (`apps/desktop/src-tauri/src/cli/`), installed from npm as `@pygg/cli`; same core and store as the app. macOS and Linux have no app yet: use `pygg gateway add-connection` and `pygg gateway serve`.

## Rules

- **Read commands only read:** `providers`, `models`, `logs`, `usage`, `status`, `doctor`, `diagnostics`, `setup --dry-run` open the store read-only and send nothing to providers.
- **`--json`** on every read command; it never holds a credential, prompt or session id.
- **No secrets on the command line:** provider keys come from `--key-env VAR`; a new Pygg API key is never printed.
- **Writes never migrate:** `providers add|…`, `models enable|disable`, `default` write only at this program's schema; `gateway` commands migrate.
- **Port:** `--port`, else the store's saved port, else 8741. **Store:** `--db PATH`, else `PYGG_DATA_DIR`, else the app's data folder.
- **Options per command:** `pygg help COMMAND` (same as `pygg COMMAND --help`); errors end with the next thing to try.

## Commands

| Command | What it does |
| --- | --- |
| `pygg` | Where you are: gateway, providers, tools set up, the next command |
| `pygg setup [claude\|codex\|opencode]` | List coding tools, or route one through Pygg (`--dry-run`, `--remove`); see [AGENTS.md](../AGENTS.md) |
| `pygg run [--model M] [--statusline] TOOL [args…]` | Start the tool through Pygg (Pygg's options before the tool name); prints a cost receipt at exit |
| `pygg statusline` | The launch's cost in one line, for Claude Code's status line |
| `pygg chat` | A conversation through the router, in memory only (ADR-028) |
| `pygg ask QUESTION…` | One question, the answer on stdout (also from a pipe) |
| `pygg default [MODEL\|--clear]` | The model `chat` and `ask` use without `-m` |
| `pygg providers [presets\|add\|refresh\|enable\|disable\|remove]` | List or change connections; `remove` needs `--yes`; web sessions only in the app |
| `pygg models [CONNECTION]` | Models to ask for, billing, price |
| `pygg models info\|test\|enable\|disable CONNECTION/MODEL` | Facts and place in `auto`; one short test; switch on or off |
| `pygg models facts [ROLE]` | Models ranked for a role from the facts feed (ADR-040) |
| `pygg route [auto\|auto/cheap\|auto/ROLE]` | The models the router would try, and why the rest are left out |
| `pygg combos` · `pygg keys` | Combos; API keys (never the secret) |
| `pygg status` · `pygg doctor` | Is the gateway answering; plus tools, agents, providers needing attention |
| `pygg logs` · `pygg usage` · `pygg savings` | Recent attempts; known and unknown cost apart; the saving estimate |
| `pygg launches [--by-repo]` | `pygg run` launches; per repository and week (observations, not productivity) |
| `pygg gateway COMMAND` | The gateway without the window: `serve`, connections, keys, clients, combos, prices, `facts` |
| `pygg diagnostics --out PATH` | The redacted diagnostics zip |
| `pygg completion powershell\|bash\|zsh` | Tab completion |
| `pygg help COMMAND` · `pygg --version` | Help for a command; the npm package's version |
| `pygg mcp setup [TOOL]` · `pygg mcp` | Register, or run, Pygg's MCP server (experimental, on hold) |
| `pygg agent COMMAND` | Agent CLIs: detect, run, task, quality (on hold; [AGENTS.md](../AGENTS.md)) |

- API keys, combos, prices and tiers change only in the app or with `pygg gateway`.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Done |
| 1 | Failed (store not found, gateway not answering for `run`, a failed write) |
| 2 | Wrong usage |
| 3 | A check found a problem (`status`, `doctor`) |

- `pygg run` returns the tool's own exit code once the tool has started.

## The npm launcher (`packages/npm`)

`@pygg/cli` is a Node 18+ launcher that runs the native `pygg-cli` with the same arguments and exit code; npm installs the matching platform package (ADR-037):

| Package | Platform |
| --- | --- |
| `@pygg/win32-x64` | Windows x64 |
| `@pygg/linux-x64` | Linux x64, glibc |
| `@pygg/darwin-arm64` | macOS, Apple silicon |
| `@pygg/darwin-x64` | macOS, Intel |

```text
npm install -g @pygg/cli@dev
npx --yes --package=@pygg/cli@dev pygg doctor
```

- Tags `dev` and `latest`; versions in [PUBLISHING.md](../../packages/npm/PUBLISHING.md).
- Lookup: `PYGG_CLI` (full path), the platform package, then on Windows the installed app; `pygg --launcher-info` shows each place checked.
- musl Linux (Alpine) needs a glibc image; Windows on Arm needs x64 Node.
- Developer preview, unsigned: SmartScreen, Gatekeeper or antivirus may warn.

## Not built yet

`providers check`, `models refresh`, `logs --follow`, colour, a set-up wizard; not yet installed from the registry on a real Mac.
