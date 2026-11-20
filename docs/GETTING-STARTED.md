# Getting started

> **Status:** Current · install Pygg, connect a provider and point a coding tool at it · reviewed 2026-11-20

The in-app **Guide** has the same steps with copyable commands; status in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md).

## What you need

- **Windows 10/11 x64** with WebView2 for the desktop app (ADR-001).
- **One provider account** (Gemini, OpenRouter, Groq free tiers; Ollama local): [PROVIDERS.md](PROVIDERS.md).
- **A coding tool:** Claude Code, Codex CLI, OpenCode, an OpenAI-compatible editor extension or SDK.

## 1. Install

**Desktop app:** `Pygg_0.1.0_x64-setup.exe` or the MSI from the [`v0.1.0-dev.3` prerelease](https://github.com/nambean/Pygg/releases/tag/v0.1.0-dev.3); compare with `SHA256SUMS.txt`.

```powershell
Get-FileHash .\Pygg_0.1.0_x64-setup.exe -Algorithm SHA256
```

- Unsigned: **More info → Run anyway**.
- Installs to `%LOCALAPPDATA%\Pygg` (with `pygg-cli.exe`); uninstall from Windows Settings → Apps.
- Clean-machine install: not verified.

**Command line only** (Windows, macOS, Linux; npm developer preview, ADR-037):

```bash
npm install -g @pygg/cli@dev
pygg gateway add-connection --preset openrouter --key-env OPENROUTER_API_KEY   # macOS/Linux: no app
pygg gateway create-client --name my-tool     # prints a Pygg API key once
pygg gateway serve
```

- Provider answering via `pygg gateway serve` on macOS/Linux: not verified.

## 2. Start Pygg

Open Pygg from the Start menu; it opens on **Home** and serves on this computer only:

```
OpenAI-compatible:  http://127.0.0.1:8741/v1
Anthropic:          http://127.0.0.1:8741
Model:              auto
```

- Data: `%APPDATA%\app.smarttaskrouter.desktop`; provider keys in Windows Credential Manager.

## 3. Connect a provider

**Providers → Connect Provider**, paste the API key.

- **Facts for `auto`:** tier, price, context, tools; **Providers → Auto…** or `pygg gateway lookup` fills them. A model missing one is not eligible.
- **Several accounts:** Providers → a connection → API keys (ADR-038).
- **Web sessions** (opt-in, unofficial, account risk): [PROVIDERS.md](PROVIDERS.md#web-sessions-unofficial).

## 4. Set up your coding tool

**CLI Tools → Set Up**, or the commands below; each tool gets its own Pygg API key. Details: [AGENTS.md](AGENTS.md).

```powershell
pygg setup claude                 # launcher; Claude Code's settings unchanged
pygg run claude
pygg setup codex                  # adds a pygg profile after a backup
pygg run codex
pygg setup opencode               # adds a Pygg provider after a backup
pygg run opencode                 # then /models → Pygg → auto
pygg setup <tool> --remove        # undo (Codex, OpenCode)
```

**Editors and SDKs:** Provider OpenAI Compatible, Base URL `http://127.0.0.1:8741/v1`, a Pygg API key (Endpoint → API Keys), model `auto`.

## 5. Work with `auto`

| Model | Picks |
| --- | --- |
| `auto` | Eligible model at or above the key's minimum tier (T2): local, free, subscription, then cheapest |
| `auto/cheap` | Lowest estimated cost |
| `auto/plan`, `auto/code`, `auto/test`, `auto/review` | Your runs of that role, then benchmark bands |
| A combo name | Your ordered list from **Combos** |
| `connection/model` | That model |

- Fallback only on errors before any answer; poor answers are never retried.

## 6. See cost and set limits

- **Usage:** each request's cost: actual, estimated or unknown (never $0).
- **Limits:** Endpoint → API Keys → Limits; a strict limit cannot be passed.

```powershell
pygg status
pygg usage
pygg doctor
```

## Troubleshooting

| You see | Do |
| --- | --- |
| Connection refused | Start the app; `pygg status` |
| HTTP 401 | Wrong or revoked key; create a new one in Endpoint → API Keys |
| HTTP 422 "No connected model is eligible for auto" | `pygg gateway lookup`, Providers → Auto…, or name a model |
| HTTP 429 "cooling down" | Provider limit; `auto` moves on |
| Cost unknown | Set a price in Providers |

Help: attach `pygg diagnostics --out pygg-diagnostics.zip` (redacted). Security issues: [SECURITY.md](SECURITY.md).

## Good to know

- Pygg provides no models; the gateway listens on `127.0.0.1` only.
- `pygg run claude` uses your providers, not your Claude plan.
- No chat app (ADR-028); Smart Tasks on hold ([AGENTS.md](AGENTS.md)).
