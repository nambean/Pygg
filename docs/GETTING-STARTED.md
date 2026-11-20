# Getting started

> **Status:** Current · install Pygg, connect a provider and point a coding tool at it · reviewed 2026-11-20

From the installer to "my coding tool uses the model `auto`". The in-app **Guide** has the same steps with every command ready to copy; what is built and verified is in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md).

## What you need

- **Windows 10 or 11, x64** for the desktop app (ADR-001), with **WebView2** (built into Windows 11; Windows 10 may need it from Microsoft).
- **One provider account.** Gemini, OpenRouter and Groq have free tiers; Ollama runs locally. See [PROVIDERS.md](PROVIDERS.md).
- **A coding tool:** Claude Code, Codex CLI, OpenCode, an editor extension that calls an OpenAI-compatible API, or an SDK.

## 1. Install

**Desktop app (Windows).** Download `Pygg_0.1.0_x64-setup.exe` (NSIS, per user) or the MSI, with `SHA256SUMS.txt`, from the [`v0.1.0-dev.3` prerelease](https://github.com/nambean/Pygg/releases/tag/v0.1.0-dev.3), and check it:

```powershell
Get-FileHash .\Pygg_0.1.0_x64-setup.exe -Algorithm SHA256   # compare with the line in SHA256SUMS.txt
```

- Not code-signed: at "Windows protected your PC" choose **More info → Run anyway**.
- Installs for the current user: `%LOCALAPPDATA%\Pygg\Pygg.exe` (Start-menu shortcut), `pygg-cli.exe` next to it, and the headless programs the app uses.
- Uninstall: Windows Settings → Apps, or `uninstall.exe` in the same folder.
- Build it yourself: `pnpm install`, then `pnpm --filter @pygg/desktop tauri:build`; output in `apps/desktop/src-tauri/target/release/bundle` (requirements in the [README](../README.md)).
- Not verified yet: installing on a clean machine.

**Command line only (Windows x64, macOS Apple silicon and Intel, Linux x64 glibc).** A developer preview on npm (ADR-037); versions in [PUBLISHING](../packages/npm/PUBLISHING.md).

```bash
npm install -g @pygg/cli@dev
pygg --version
```

On Windows it works with the desktop app, which runs the gateway. On macOS and Linux there is no app: connect and serve from the terminal (the key is read from an environment variable, never the command line):

```bash
pygg gateway add-connection --preset openrouter --key-env OPENROUTER_API_KEY
pygg gateway create-client --name my-tool     # prints a Pygg API key once
pygg gateway serve                            # in a terminal of its own
```

- Unsigned: macOS Gatekeeper may refuse it the first time; allow it in System Settings → Privacy & Security.
- Verified: the Linux package installed from npm and run (`pygg --version`); macOS packages installed from their tarballs and run by CI. Not verified: a provider answering through `pygg gateway serve` on macOS or Linux, an npm install on a real Mac.

## 2. Start Pygg

Open Pygg from the Start menu. It opens on **Home** (connect a provider, set up your coding tool, work with `auto`), and the gateway starts on this computer only:

```
OpenAI-compatible:  http://127.0.0.1:8741/v1
Anthropic:          http://127.0.0.1:8741
Model:              auto
```

- Nothing is sent to a provider until you connect one and a request comes through.
- Data (store, request ledger, web-session profiles): `%APPDATA%\app.smarttaskrouter.desktop`. Provider keys are in the Windows Credential Manager, not there.
- `PYGG_DATA_DIR` points the app at another folder (for example to try it without touching your store); that app runs beside the installed one and needs its own gateway port (Endpoint → Configure…).

## 3. Connect a provider

**Providers → Connect Provider**: choose a provider and paste its API key. Pygg reads the model list (not billed); the key goes to the Credential Manager and never back to the interface.

- **Free first:** Gemini (AI Studio), OpenRouter (`:free` models), Groq; limits set by each provider. When one is used up, `auto` moves on.
- **Local:** Ollama and five other runtimes have presets; a local model costs $0.
- **Facts for `auto`:** each model needs tier, price, context size and tool support. Providers → **Auto…** fills them from the models.dev catalog (also `pygg gateway lookup`); a model missing a fact is not eligible for `auto`.
- **Several accounts:** a connection can hold keys of separate accounts, used in turn when one is rate-limited, out of credit or rejected (Providers → a connection → API keys, ADR-038).
- **Answered live:** Gemini, Anthropic, OpenRouter, Ollama, Qwen's token plan. The rest are configured from their docs ([PROVIDERS.md](PROVIDERS.md)).
- **Web sessions** (Muse, ChatGPT, Perplexity, DeepSeek, Grok, Gemini): an AI site with no API, through a Pygg window you sign in to yourself. Opt-in, unofficial, behind an account-risk warning: a site can suspend an account it takes for automated use, and one has. Short questions only; see [PROVIDERS.md](PROVIDERS.md#web-sessions-unofficial) and ADR-035.

## 4. Set up your coding tool

**CLI Tools** sets up each tool, shows its state, and says before **Set Up** whether `auto` has an eligible model for that tool. Each tool gets its own Pygg API key, so Usage tells tools apart and each can have its own limit. `pygg setup` runs the same code (not verified yet against a real tool's configuration). Details per tool: [AGENTS.md](AGENTS.md).

**Claude Code** (a launcher; Claude Code's settings are not changed, and plain `claude` keeps your Claude plan):

```powershell
pygg setup claude --dry-run     # shows what would change
pygg setup claude
pygg run claude                 # Claude Code through Pygg, this launch only
pygg run --model auto/cheap claude
pygg run claude -p "Explain this repository"
```

- Pygg's options go before `claude`; arguments after it are Claude Code's.
- Claude Code may say it does not know the model `auto` and assume a 200k context: expected.
- Verified: headless `claude -p` through the gateway, with a sub-agent and `/compact`. Not verified: the launcher with a provider answering, an interactive session.

**Codex CLI** (adds a `pygg` provider and profile after a backup; Codex's default model is unchanged):

```powershell
pygg setup codex --dry-run
pygg setup codex
pygg run codex
codex --profile pygg -m auto/cheap
pygg setup codex --remove       # undo
```

Verified live: Codex through the Responses API, and the one-click set-up in CLI Tools.

**OpenCode** (adds a Pygg provider to `opencode.json` after a backup, listing `auto`, `auto/cheap`, `auto/plan`, `auto/code`, `auto/test`, `auto/review` and your enabled models, kept current):

```powershell
pygg setup opencode
pygg run opencode
# then in OpenCode: /models → Pygg → auto
pygg setup opencode --remove    # undo
```

Verified live: an OpenCode run through the gateway, with a combo fallback recorded in the ledger.

**Editors and SDKs** (Cline, Continue, Roo Code and others calling from this computer):

```
Provider:  OpenAI Compatible
Base URL:  http://127.0.0.1:8741/v1
API key:   a Pygg API key (Endpoint → API Keys)
Model:     auto
```

Endpoints: `POST /v1/chat/completions`, `POST /v1/responses`, `POST /v1/messages`. Tools that call from their own servers cannot reach `127.0.0.1`. A Python example (sends a real request, recorded in Usage):

```python
from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:8741/v1", api_key="<your Pygg API key>")
reply = client.chat.completions.create(model="auto", messages=[{"role": "user", "content": "Hello"}])
print(reply.choices[0].message.content)
```

## 5. Work with `auto`

Use your tool as usual with the model `auto`; CLI Tools shows **Working** once a request has come through. Every response says why its route was chosen.

| Model name | Picks |
| --- | --- |
| `auto` | An eligible model at or above the key's minimum tier (T2 unless changed): local, free and subscription before paid, then lowest estimated cost. Follows your rules; does not judge prompt difficulty. |
| `auto/cheap` | The lowest estimated cost among models that can take the request. |
| `auto/plan`, `auto/code`, `auto/test`, `auto/review` | Your own runs of that role first, then public benchmark facts in close bands, cheaper first within a band (feed fetched daily; with none, or facts off, by tier alone). No claim yet: one run measured `auto/code` before its current order ([BENCHMARKS](BENCHMARKS.md)). |
| `coding`, or any combo's name | Your ordered list from **Combos**; the next model answers when one is out of quota or unavailable. Templates (Free first, Use my plans first, Local only, …) never fill a gap with a paid model. |
| `gemini/gemini-2.5-flash`, any `connection/model` | That model. |

- Simulate a route without sending: **How auto chooses** in Combos, or `pygg gateway auto --alias auto --tools yes`.
- Fallback happens only on errors, before any answer reaches the client; Pygg never judges or retries a poor answer.

## 6. See cost and set limits

- **Usage:** every request with its cost source: **actual** (provider usage × model price), **estimated** (tokens × price) or **unknown** (never $0, left out of totals; set a price in Providers). Subscription use counts as quota, not money.
- **Estimated Savings:** the same tokens priced at a model you choose, at list prices. Not verified against real usage (unit-tested on a test ledger).
- **Limits** (Endpoint → API Keys → Limits): monthly limit (estimated or strict), requests in flight, policy for unpriced models, allowed connections, minimum tier for `auto`. A strict limit holds each request at its maximum cost, so it cannot be passed. **Policies** sets defaults for new keys.

## Command line

`pygg-cli.exe` next to the app; `pygg` from npm. Without npm: `& "$env:LOCALAPPDATA\Pygg\pygg-cli.exe" doctor`.

```powershell
pygg                    # where you are, and what to type next
pygg status             # is the gateway answering
pygg doctor             # status, plus your tools and the providers that need attention
pygg setup              # the tools Pygg can set up, and their state
pygg providers          # connections, their health and models
pygg logs               # recent requests
pygg usage              # requests and cost
pygg chat --model muse/muse                  # talk to any connected model through Pygg's router
pygg ask --model auto/cheap "a question"     # one question, the answer printed
pygg default muse/muse                       # the model chat and ask use without --model
pygg models info muse/muse                   # one model's price, limits and place in auto
pygg route                                   # which models auto would try now, and why the rest are left out
pygg providers add openrouter --key-env OPENROUTER_API_KEY   # connect an API provider from the terminal
pygg completion powershell >> $PROFILE       # Tab completion
```

- `pygg chat` and `pygg ask` use every connection, web sessions included, with the same routing, fallbacks and ledger; the conversation stays in memory only.
- Read commands take `--json`. Keys never go on the command line (`--key-env`). `pygg <command> --help` lists options; [design/CLI.md](design/CLI.md) describes every command.

## Troubleshooting

| You see | Do |
| --- | --- |
| Connection refused, "nothing is listening" | Start the Pygg app (it serves the gateway); check with `pygg status`. Port: Endpoint → Configure. |
| HTTP 401 | Wrong or revoked key. Endpoint → API Keys → **Show** copies it. A key that cannot be shown (older, or from `pygg gateway create-client` or a tool's set-up) was shown once: create a new one. |
| HTTP 422 "No connected model is eligible for auto" | No model has every fact `auto` needs (tier, price, tools, context). Run `pygg gateway lookup`, use Providers → Auto…, lower the key's minimum tier, or name a model. |
| HTTP 429, "cooling down" | A provider's rate limit or quota. `auto` and combos move on; Providers shows when it is retried. |
| Web session says "Not sent" | Its guard is pacing the account, or the prompt is too long. Wait the time it gives, or use an API connection. |
| Cost shows as unknown | The model has no price, the provider reported no usage, or a failed request may have been billed. Set a price in Providers or run `pygg gateway lookup`. |
| Claude Code does not know the model `auto` | Expected; it assumes a 200k context and carries on. |
| Stopped after a server error | The provider may have run and billed it; Pygg tries another model only if the key allows paid retries. |
| Gemini CLI: "client no longer supported" | Google no longer accepts its free Google-account login. Use a Gemini API key, or connect Gemini in Providers and go through Pygg. |

Before asking for help, attach the redacted bundle (no keys, tokens or prompts):

```powershell
pygg doctor
pygg diagnostics --out "$env:USERPROFILE\Desktop\pygg-diagnostics.zip"
```

Bugs: the repository's issues. Security problems: privately, per [SECURITY.md](SECURITY.md).

## Good to know

- Pygg provides no models: it routes to the providers you connect, with your keys or local runtimes. Free tiers are not unlimited.
- The gateway listens on `127.0.0.1` only; other computers cannot use it.
- Claude Code through `pygg run` uses your connected providers, not your Claude plan.
- No chat app of its own (ADR-028). Smart Tasks (IDE agents handing subtasks over MCP) are experimental and on hold: [AGENTS.md](AGENTS.md).
- More: [SECURITY.md](SECURITY.md), [BENCHMARKS.md](BENCHMARKS.md), [README.md](README.md) (index of every document).
