# The `pygg` command line

> **Status:** Current · first release built 2026-11-14 after [Codex's review](../reviews/2026-11-14-codex-cli-review.md) (section 6) · the owner: "Pygg's CLI and npm are too thin and hard to use", pointing at [OmniRoute's CLI reference](https://github.com/cheaper-inference/OmniRoute/blob/release/v3.8.54/docs/reference/CLI-TOOLS.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md) · npm: [packages/npm](../../packages/npm/pygg/README.md)

## 1. What is wrong today

- Commands follow Pygg's modules, not what a person does: providers are added with `pygg gateway add-connection --preset`, listed with `pygg gateway list` (providers, combos and keys in one dump), checked with nothing.
- `setup` names are internal and clash: `codex-models` and `opencode-models` route a tool through the gateway, while `codex` and `opencode` register the experimental MCP server. A person typing `pygg setup codex` gets the wrong one.
- No `--json` anywhere, so nothing is scriptable; no `logs`, no `models`, no `keys`, no provider check, no shell completion.
- `pygg` alone prints fixed text; it does not look at what is set up.
- `pygg gateway --help` is a flag list with no grouping or examples.

OmniRoute's CLI is resource-oriented (`providers list|test|add|remove`, `keys`, `models`, `combo`, `logs --follow`), has `--json` on read commands, `--yes` on destructive ones, a setup wizard with `--non-interactive`, `run <tool>` that injects variables without writing configuration, and `completion`.

## 2. Principles

1. **Verbs on nouns**: `pygg <noun> <verb>`; the bare noun lists.
2. **Read commands take `--json`**; the JSON is the same objects the app's screens read (camelCase), never a parsed table.
3. **Nothing secret on the command line**: keys come from an environment variable (`--key-env VAR`) or standard input (`--key-stdin`); a new Pygg API key is printed once, only to a terminal or with `--show-key`.
4. **Destructive commands confirm** (a prompt in a terminal; `--yes` otherwise), and say what they will do first.
5. **Free by default**: no command sends a prompt unless it says so (`models test`, which may cost a little, asks).
6. **Exit codes**: 0 done, 1 failed, 2 usage error, 3 a check found a problem (`doctor`, `providers check`).
7. **Old names keep working** as hidden aliases for one release, with a one-line note on stderr.

## 3. Commands

```
pygg                                  where you are: gateway, providers, tools set up, and the next step
pygg setup                            in a terminal: a short wizard (choose tools, set each up)
pygg setup claude|codex|opencode      route the tool through Pygg   [--dry-run | --remove]
pygg run claude|codex|opencode […]    start it through Pygg, a routing session per launch   (claude: --model M)

pygg status | doctor                  [--json]
pygg providers                        list: state, last check, health score, models       [--json]
pygg providers check [ID | --all]     read each model list (free); exit 3 if one fails    [--json]
pygg providers add PRESET             --key-env VAR | --key-stdin  [--label --billing --endpoint]
pygg providers remove ID              [--yes]
pygg providers presets                what can be connected, grouped (free, credits, paid, local)   [--search S] [--json]
pygg models [CONNECTION]              enabled models, billing, price, auto readiness       [--search S] [--all] [--json]
pygg models refresh ID                read the provider's list again and add new models
pygg models test CONNECTION/MODEL     one short request (may cost a little; asks first)
pygg auto                             what auto would choose for a coding tool's request, and why the rest are left out   [--json]
pygg keys                             Pygg API keys (clients): spend this month, last used   [--json]
pygg keys create NAME                 [--monthly-usd N]   prints the key once
pygg keys revoke NAME                 [--yes]
pygg combos                           [--json]
pygg usage                            [--days 1|7|30] totals, token mix, by client and model   [--json]
pygg logs                             recent requests   [--follow] [--client C] [--failed] [--limit N] [--json]
pygg completion powershell|bash|zsh   print a completion script

pygg gateway serve                    the gateway without the window (advanced; the other `gateway` verbs stay as aliases)
pygg mcp setup claude-code|codex|opencode   experimental task integration (on hold)
pygg mcp                              the MCP server itself, which the tool starts
pygg agent …                          experimental, on hold
pygg diagnostics --out PATH
```

Renames: `setup claude-launcher` → `setup claude`, `setup codex-models` → `setup codex`, `setup opencode-models` → `setup opencode`; the MCP registration moves to `mcp setup`. `gateway add-connection` → `providers add`, `gateway list` → `providers` + `combos` + `keys`, `gateway usage` → `usage` / `logs`, `gateway create-client` → `keys create`, `gateway refresh-models` → `models refresh`, `gateway test-model` → `models test`.

## 4. Output

Tables with aligned columns and no colour by default; colour only for a terminal that supports it and without `NO_COLOR`. Each list ends with a hint line (`pygg providers check --all to check them now`). Errors say what to do next, as `pygg run` already does.

## 5. Open questions for review

- Does `pygg` alone showing live state replace the fixed start text well enough, without being slow when the gateway is down?
- `providers check` from the command line opens the app's store while the app runs: acceptable (SQLite, short writes), or should it ask the running gateway?
- Is a wizard worth it with only three tools, or is `pygg setup` listing tools with one-line commands enough?

## 6. The first release (built 2026-11-14), after Codex's review

Codex: the draft was right in direction and too large; fix naming, read commands, errors and the npm journey first, and add nothing that writes yet.

Built:

- **Names.** `pygg setup claude|codex|opencode` route the tool through Pygg (`claude-launcher`, `codex-models`, `opencode-models` still work); the MCP registration is `pygg mcp setup claude-code|codex|opencode`, and `pygg setup claude-code` says so. `pygg setup` alone is a table: tool, installed, Pygg setup, the next command.
- **Read commands** (`cli/read.rs`), each with `--json` (`schemaVersion` 1, its own output types: no credential reference, endpoint, prompt, session id or upstream error text): `providers` (on, last check, health score, models enabled/total, what blocks it now), `models` (route, billing, price with units or "no price (cost unknown)", tier; `--search`, `--all`, `--limit`, 50 rows by default), `logs` (attempts newest first, UTC; `--limit`, `--client`, `--failed`, filters before the limit), `usage` (requests, known and unknown cost apart, token mix, by client and model).
- **A bare `pygg`** shows the gateway, the store, providers (and how many need attention), tools set up, and the next command, in about a second: one loopback probe and a few reads.
- **`status` and `doctor`** take `--json` and exit 3 when something is wrong; `doctor` lists providers that need attention and no longer prints account sign-ins in JSON.
- **Strict options**: an unknown option, a missing value or a repeated option is exit 2 with what to type; `--port nonsense` is refused instead of ignored. One port rule: `--port`, then the store's setting, then 8741.
- **Read-only means read-only**: the read commands, `status`, `doctor` and `setup --dry-run` open the store read-only and refuse a missing, older or newer store with what to do; nothing is created or migrated.
- **The health probe** requires an HTTP success whose `app` is Pygg.
- **`pygg run`**: `--help` after the tool's name is the tool's; an unknown Pygg option before it is refused.
- **npm**: the README walks from install to a working tool; the package description says what it is for.

Deferred, as Codex advised: `providers check` (checks must run in the process that serves the store, through an authenticated call the gateway does not have yet), `providers add|remove`, `models refresh|test`, `keys`, `logs --follow` (needs a stable cursor), `completion`, colour, and a wizard.
