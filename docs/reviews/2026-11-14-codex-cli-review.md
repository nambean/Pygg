# Codex's review of the command line, 2026-11-14

> **Status:** Record · Codex's review of the [CLI design](../design/CLI.md) draft (read-only, no files changed), and what was done · the owner: "Pygg's CLI and npm are too thin and hard to use", pointing at [OmniRoute's CLI reference](https://github.com/cheaper-inference/OmniRoute/blob/release/v3.8.54/docs/reference/CLI-TOOLS.md)

**How it was run.** Codex CLI 0.163.0, `codex exec --sandbox read-only --ephemeral`, given the draft, the CLI source (`cli/`), the npm launcher and docs, and OmniRoute's reference. No model request, no money spent.

| # | Finding (Codex) | Answer |
| --- | --- | --- |
| 1 | `setup codex` cannot keep its MCP meaning and become the gateway set-up; hidden aliases for every old name blur this | **Done.** Gateway set-ups are `setup claude|codex|opencode`; MCP moved to `mcp setup`; only the unambiguous old names stay |
| 2 | JSON must be the CLI's own types, versioned; never the app's structs (credential references, web metadata) | **Done.** `schemaVersion` 1; no credential reference, endpoint, session id or upstream error text; unknown cost is null, never 0 |
| 3 | The option parser accepts anything; `status --port nonsense` silently uses 8741 | **Done.** Strict options, exit 2 with what to type |
| 4 | `pygg run claude --help` prints Pygg's help instead of Claude Code's | **Done.** Help only before the tool's name |
| 5 | `setup --dry-run` opens the store for writing (creating or migrating it) | **Done.** A preview opens it read-only and refuses a missing store |
| 6 | The health probe accepts any JSON | **Done.** HTTP success and `app` = Pygg |
| 7 | Port rules differ between commands | **Done.** `--port`, then the store's setting, then 8741, everywhere new |
| 8 | Read commands go through `gateway`'s opener, which migrates and fills facts | **Done.** A read-only opener checks the schema and says what to do |
| 9 | Failures become empty lists (`unwrap_or_default`) | **Done** in the new commands: a read error is exit 1, an empty list says what to do |
| 10 | A CLI check would race the app's checker (its turns are per process) | **Later.** `providers check` waits for an authenticated call into the running gateway |
| 11 | `logs` filters must apply before the limit; `--follow` needs a cursor | **Done** (filters in SQL); `--follow` later |
| 12 | Bare `pygg`: live state within about a second, no agent detection or provider requests | **Done** |
| 13 | No wizard in the first release; plain output, no colour; ASCII headings for Windows consoles | **Done** |
| 14 | The npm README and description lead with MCP and give a different journey from the help | **Done.** Install, `pygg`, `setup`, `run`, then the read commands; npx's `--yes` explained |
| 15 | Through npx, an MCP registration can point at a program in npm's cache | **Noted.** Kept out of the first-time journey; MCP stays experimental |

Not copied from OmniRoute: remote contexts, tunnels, credential export, a dozen tool integrations, keys on the command line.
