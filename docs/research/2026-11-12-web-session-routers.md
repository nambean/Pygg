# How open-source AI routers connect to web-session-only AI services

Research date: 2026-11-12. Read-only web research (GitHub raw source, PyPI, project docs). Nothing was installed, run, or signed into.
"Unverified" = not confirmed from a primary source (the project's own code or docs, or the vendor's docs).

---

## Headline findings

1. **Meta has an official API now, so Muse / Muse Spark do not need a web session.** The Meta Model API (`https://api.meta.ai/v1`, `Authorization: Bearer $MODEL_API_KEY`, keys from dev.meta.ai) speaks OpenAI Chat Completions, OpenAI Responses and Anthropic Messages. Models: `muse-spark-1.3/1.2/1.1` (+ Contributor variants), `muse-wonder-1.0` (preview), `muse-ember-1.0`, `muse-image-1.0`, `muse-voice-transcribe-1.0`, `sam-3.1`. Source: https://dev.meta.ai/docs/ (redirect target of ai.developer.meta.com/docs). LiteLLM ships it as `meta/` provider with `META_API_KEY` (https://docs.litellm.ai/docs/providers/meta); 9router ships it as provider `muse` ("Muse (Meta Model API)") with two auth modes: API key and an OAuth device-code login for the Muse Code subscription (no refresh token; a 401 means log in again) — `open-sse/providers/registry/muse.js` in decolua/9router.
2. **"Muse" at muse.ai is a different thing:** Meta's consumer agent app (internal name "Hatch"), launched in the US on 2026-09-08 (news coverage, e.g. betanews, Khaleej Times). It has no API; 9router drives it through a browser (`muse-web`). Note that the muse.ai domain is Meta's agent, per 9router's code (`https://muse.ai/api/session`, `hatch_*` cookies) — the old muse.ai video host association is no longer what the router targets.
3. **The field has shifted from "reverse-engineered HTTP" to "drive a real Chromium via Playwright, seeded with the user's cookies".** Both 9router and OmniRoute now run ChatGPT Web and Gemini Web (and Muse Web in 9router) by driving the DOM in a pooled Playwright context; direct HTTP survives where the site's internal API is simple (Claude Web, Grok Web, Perplexity Web, DeepSeek Web, Meta AI's GraphQL/WebSocket).
4. **Session input is almost always "paste a cookie" (or a Playwright storage-state JSON).** No project found offers a built-in login window for these providers; g4f is the exception (it opens a visible browser and lets the user sign in, or reads cookies from installed browsers via `browser_cookie3`).

---

## 1. 9router — https://github.com/decolua/9router (MIT, Node/Next.js)

Web providers (category `webCookie`, `authType: "cookie"`): `chatgpt-web`, `gemini-web`, `grok-web`, `perplexity-web`, `deepseek-web`, `muse-web`, `uc-web`, `veoaifree-web` (`open-sse/executors/*.js`, registry in `open-sse/providers/registry/*.js`).

| Aspect | What 9router does |
|---|---|
| Session input | Paste in dashboard. Registry `authHint`s, e.g. muse-web: "Paste your muse.ai cookies (hatch_sess=...; hatch_gw=...; hatch_vml=...; hatch_native_auth_device=...)"; chatgpt-web: cookie header containing `__Secure-next-auth.session-token` **or** a full Playwright storage-state JSON. No login window. |
| Storage | SQLite `${DATA_DIR}/db/data.sqlite`, `DATA_DIR` default `~/.9router` (README). Encryption of provider credentials at rest not documented (unverified). |
| Refresh | muse-web: `GET https://muse.ai/api/session` before each turn; any `Set-Cookie` merged into the jar (`muse-web/session.js`); 401/403 = `MuseWebAuthError` (re-paste). gemini-web: no rotation/write-back. chatgpt-web: `GET https://chatgpt.com/api/auth/session` → `accessToken`. |
| Transport | `open-sse/services/browserPool.js`: pooled Playwright contexts, ephemeral (no persistent profile), seeded with cookie string / localStorage / storageState, keyed per credential hash; context TTL 10 min, pool idle shutdown 5 min; env `NINEROUTER_BROWSER_POOL=off` disables. Uses system Chrome if found, else Playwright Chromium; 503 with install hint if none. gemini-web & muse-web are DOM-driven; chatgpt-web tries an HTTP `/f/conversation` SSE path and falls back to the browser when Turnstile is required. |
| Anti-bot (describe only) | Headless engine chain includes "stealth" Chromium forks (Obscura, cloakbrowser), `--disable-blink-features=AutomationControlled`, headed windows placed off-screen, sentinel headers "solved in Node". **We should not copy any of this.** |
| Streaming | Real deltas: muse-web polls the last agent bubble every 300 ms and emits only new text, done when text stable for 4 polls and no Stop button; gemini-web forwards incremental DOM deltas. |
| Conversation | Stateless: fresh page (`/thread/new`, `/app`) per request; OpenAI history flattened into one prompt ("Previous conversation:" / "Current user message:"). |
| Tools | muse-web and gemini-web reject non-empty `tools` with 400 (Hatch agent ignores prompt-level tool contracts). chatgpt-web: text-protocol emulation, partly unfinished. chatgpt-web is registered as **image-only** "until the chat path has more mileage". |
| Models | Static lists in registry (muse-web: `muse-chat`, `muse-image`, `muse-video`). gemini-web switches model in the UI and **fails closed** (400) if the UI cannot confirm the selection. |
| Agent-specific | muse-web auto-**denies** Hatch approval cards (`[data-testid="hatch-inline-approval-card"]`) so the agent never acts (e.g. publishing a file) on the user's behalf; optional `POST https://muse.ai/api/hatch/vm/wake` to wake the user's cloud VM. |
| Warnings | `RISK_NOTICE` (`src/shared/constants/providersDisplay.js`): provider "uses a subscription/OAuth session not officially licensed for proxy/router use" and the account "may be restricted or banned. Use at your own risk." |

## 2. OmniRoute — https://github.com/diegosouzapw/OmniRoute (MIT, TypeScript)

37 web-cookie providers per `docs/reference/PROVIDER_REFERENCE.md`, executors in `open-sse/executors/` (chatgpt-web, claude-web, gemini-web, grok-web, perplexity-web, copilot-web, copilot-m365-web, deepseek-web, muse-spark-web, …).

| Aspect | What OmniRoute does |
|---|---|
| Session input | Paste in dashboard (`/api/oauth/{provider}/paste-credentials` per a third-party doc summary — unverified). Per-provider: chatgpt-web **only** Playwright storage-state JSON ("Cookie headers and individual token values are not accepted"); claude-web `sessionKey`; gemini-web full cookie / `__Secure-1PSID` (+`1PSIDTS`, `1PSIDCC`) / JSON export; grok-web `sso` + `sso-rw`; perplexity-web `__Secure-next-auth.session-token`; deepseek/kimi/zai: a token copied from Local Storage; muse-spark-web `ecto_1_sess` + `ecto1:…` token. Multiple cookies per connection rotate (`extraApiKeys`). |
| Storage | SQLite `~/.omniroute/` (`DATA_DIR`); `STORAGE_ENCRYPTION_KEY` = optional AES full-DB encryption (empty = disabled); `API_KEY_SECRET` encrypts key values (`docs/reference/ENVIRONMENT.md`). README claims "AES-256-GCM encrypted keys". |
| Refresh | gemini-web: after each run reads the browser cookie jar and writes back changed `__Secure-1PSID/1PSIDTS/1PSIDCC` via `onCredentialsRefreshed`. muse-spark-web: GraphQL call may return a new `ecto_1_sess` via Set-Cookie. claude-web: none — 401 = re-paste. |
| Transport | Mixed. claude-web direct HTTP to `https://claude.ai/api/organizations/{org}/chat_conversations/{id}/…` with `anthropic-client-platform: web_claude_ai`; grok-web direct `POST https://grok.com/rest/app-chat/conversations/new` (NDJSON); gemini-web and chatgpt-web drive Playwright ("rebuilt solely from first-party UI/network observations"). Env `WEB_COOKIE_USE_BROWSER` forces browser transport. |
| Anti-bot (describe only) | Chrome-like TLS fingerprint (`wreq-js` in `grokTlsClient.ts`, `claudeTlsClient.ts`), browser-like `Sec-Ch-Ua`/`Sec-Fetch-*` headers, optional browser "clearance refresh" after a Cloudflare 403, residential-proxy options. Cloudflare challenges surface as 403 `cloudflare_challenge`. |
| Streaming | grok: NDJSON → OpenAI SSE (with `reasoning_content`); claude: SSE, turn committed only after `message_stop`; gemini: pseudo-stream (one chunk after completion). |
| Conversation | grok: new conversation each call with `temporary: true, disableMemory: true`; claude: prepared turn committed/invalidated; muse-spark-web: reuses server conversation via a 30-min in-memory cache keyed by SHA-256 of (connection, model, history up to last assistant turn); others stateless. |
| Tools | Prompt emulation: tool contract in system text, parse `<tool>{...}</tool>` replies into `tool_calls`; forced `tool_choice` → 400; grok also maps native tool events. |
| Rate limits | 429 → passes upstream `Retry-After` (claude-web, capped 24 h); account-fallback loop skips a depleted account and tries the next. No ToS/ban text in the provider catalogue; some entries are labelled "Unofficial/Experimental". |

**OmniRoute `muse-spark-web` (Meta AI, meta.ai) — the most complete Meta AI implementation found**
(`open-sse/executors/muse-spark-web.ts`, `muse-spark-web/fresh-access-token.ts`):
- Cookie `ecto_1_sess` (bare value gets the name prepended).
- Access token `ecto1:…` (~5 min life) is scraped from server-rendered `https://www.meta.ai/` HTML; plain fetch gets 403 / a JS challenge, so a **headless Playwright page** loads it (`networkidle`, 30 s) and regex-extracts `accessToken…ecto1:…`; cached 4 min keyed by cookie hash.
- Each turn: GraphQL `POST https://www.meta.ai/api/graphql` (persisted `doc_id`s for warm-up and mode switch, header `X-FB-Friendly-Name: useEctoSendMessageSubscription`, `X-ASBD-ID`), then WebSocket `wss://gateway.meta.ai/ws/clippy` with the token in the `Authorization` **query parameter**, app id `1522763855472543`, binary frames; parser handles `full` and `patch`/`delta` events on `/sections/0/view_model/primitive/text`.
- Models: `muse-spark` (think_fast), `muse-spark-thinking`, `muse-spark-contemplating` (think_hard). 30 s WS timeout; 401 asks user to re-paste `ecto_1_sess`; no 429 handling.
- Comments record doc_ids and fields Meta already removed — breaks often.

## 3. CLIProxyAPI — https://github.com/router-for-me/CLIProxyAPI (Go)

| Aspect | Notes |
|---|---|
| Web sessions | Only **Gemini Web**, now removed/being removed (docs say support is dropped from 6.2.x because Gemini CLI OAuth covers it — wording seen in search snippet; exact release unverified). Current README lists only OAuth (Gemini CLI, Codex, Claude Code, Antigravity, Devin) and API keys. |
| Input | `cli-proxy-api --gemini-web-auth`: paste full `Cookie` header copied from the `ListAccounts` request, or `__Secure-1PSID` / `__Secure-1PSIDTS` (https://cliproxyapidocs.pages.dev/hands-on/tutorial-3). |
| Storage | "automatically saved" — auth JSON in the auth dir (exact path unverified). |
| Notes | Warned that rotating multiple Gemini Web accounts breaks conversation continuity; 6.0.17 added sticky sessions. |

## 4. AIClient-2-API — https://github.com/justlovemaki/AIClient-2-API

Only one web-cookie provider: **Grok** — copy `sso` cookie from DevTools → `GROK_COOKIE_TOKEN` in config; warns "Ensure `GROK_USER_AGENT` matches the browser used when obtaining the cookie to avoid being blocked". Everything else is OAuth files (`~/.codex/oauth_creds.json`, `~/.antigravity/oauth_creds.json`, Kiro token) with background refresh, account pool, health checks, `RATE_LIMIT_COOLDOWN_ENABLED`. Disclaimer: "for learning and research purposes only".

## 5. gpt4free (g4f) — https://github.com/xtekky/gpt4free

| Aspect | Notes |
|---|---|
| Session input | Three routes (`g4f/cookies.py`): (a) read cookies from installed browsers with `browser_cookie3` (Firefox, Chrome, Chromium, Opera, Brave, Edge, Vivaldi, plus a dedicated g4f Chrome profile), filtered by `COOKIE_DOMAINS` (includes `.meta.ai`, `.google.com`, `chatgpt.com`, `.perplexity.ai`, `.bing.com`); (b) drop `.har`/`.json` cookie files into a cookies dir (created 0700 on Unix); (c) **nodriver/CDP opens a visible browser** and the user signs in; tokens captured from network traffic. |
| Storage | Plain files in the cookies dir + in-memory cache; no keyring. |
| Transport | OpenaiChat: HTTP to `https://chatgpt.com/backend-api/...` (`sentinel/chat-requirements`, conversation SSE, files upload), proof-of-work + Turnstile tokens generated/captured (describe only). Copilot: drives a visible browser at copilot.com; stream via `wss://substrate.office.com/m365Copilot/Chathub` (SignalR frames). |
| Conversation | Reuses `conversation_id` / `parent_message_id`; only messages after the last assistant turn are sent. |
| Meta AI | `.meta.ai` is in the cookie domains; a dedicated MetaAI provider was not found at the top level of `g4f/Provider/` today (may be removed or nested — unverified). |

## 6. Single-service libraries

- **gemini-webapi** (https://github.com/HanaokaYuzu/Gemini-API, `src/gemini_webapi/constants.py`): cookies `__Secure-1PSID` + `__Secure-1PSIDTS` (pasted, or `browser_cookie3` from Firefox); init `GET https://gemini.google.com/app` (scrapes page token); generate `POST https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate` (form-encoded `f.req`); model chosen via header `x-goog-ext-525001261-jspb`; background `auto_refresh` of `1PSIDTS` via `POST https://accounts.google.com/RotateCookies`, persisted at `GEMINI_COOKIE_PATH`; real streaming deltas; `ChatSession` metadata to resume chats; `list_models()` discovered at init. Warns Chromium's Device Bound Session Credentials make cookies die in hours (recommends Firefox), and advises a separate short-lived browser session because using cookies may force a re-login.
- **meta-ai-api** (https://github.com/Strvm/meta-ai-api, `src/meta_ai_api/main.py`): legacy Meta AI (Llama era). Anonymous: scrape `https://www.meta.ai/` for `_js_datr`, `datr`, `lsd`, `abra_csrf`; mutation `useAbraAcceptTOSForTempUserMutation` → temp `access_token`; send `useAbraSendMessageMutation` to `https://graph.meta.ai/graphql?locale=user`. Logged-in: Facebook email/password → `abra_sess` cookie + `fb_dtsg` against `https://www.meta.ai/api/graphql/`. Streams NDJSON (done at `streaming_state == "OVERALL_DONE"`), `new_conversation=True` makes a new `externalConversationId`. "Educational purposes only"; non-JSON response → `FacebookRegionBlocked`. **Out of date** — Meta moved to `ecto_1_sess` / `ecto1:` tokens and a WebSocket gateway.
- **metaai-sdk** (https://github.com/mir-ashiq/metaai-api, PyPI `metaai-sdk` 5.3.0): cookies `datr` + `ecto_1_sess` (`abra_sess` optional "for some regions"); `ecto_1_sess` "expires frequently"; optional DGW WebSocket token `META_AI_ACCESS_TOKEN`; image gen uses a browser (~30-40 s). Disclaimer: unofficial, not affiliated with Meta, "may break at any time".
- **LiteLLM**: no cookie/web-session providers; relevant only for the official Meta Model API provider.

---

## Common pattern

1. **Input = paste.** User copies a cookie header (or one named cookie, or a Local-Storage token, or a Playwright storage-state JSON) from DevTools; the UI shows a per-provider hint naming the exact cookies. Accept three shapes: bare value, full `Cookie:` header, JSON cookie export. A cheap "test connection" call validates it. Built-in sign-in windows are rare (only g4f; OmniRoute lists "browser sign-in" for one minor provider).
2. **Storage = the router's normal credential store** (SQLite row per connection, optionally AES-encrypted). No project uses the OS keychain; none keeps a persistent browser profile — browser contexts are ephemeral and re-seeded from the stored cookies.
3. **Refresh = opportunistic write-back.** Merge any `Set-Cookie` seen on a session/keep-alive call (or read the browser jar after a run) and save it back; short-lived bearer tokens are re-minted per request from the long-lived cookie and cached a few minutes. When the long-lived cookie dies → 401 with "re-paste your cookie".
4. **Transport = two tiers.** Direct HTTP to the site's internal endpoint when it is simple; otherwise a pooled Playwright Chromium (system Chrome first, keyed per credential hash, idle-evicted) that types into the page and reads the reply from the DOM or from captured network responses. Anti-bot work (TLS fingerprint impersonation, stealth browsers, PoW/Turnstile solving, clearance cookies) lives in this layer — this is the part to leave out.
5. **Stateless OpenAI facade.** New/temporary chat per request with history flattened into one prompt; only one project caches server conversation ids. Tools: either rejected (400) or emulated with a `<tool>{…}</tool>` text protocol. Model list: static per provider; when switching model in a UI, verify and fail closed.
6. **Risk handling = a fixed banner + error mapping.** 9router shows a "not officially licensed… may be restricted or banned" notice; others say "unofficial/experimental"/"educational". 429/quota → mark account cooled down, fall back to next account/provider; Cloudflare challenge → distinct error code.

## What applies to Muse / Meta AI

- **Muse Spark / Muse Wonder / Muse Ember → use the official Meta Model API**, not a web session: `https://api.meta.ai/v1`, Bearer key, OpenAI Chat Completions / Responses and Anthropic Messages compatible, `GET /v1/models` for the list. 9router additionally supports Meta's **device-code OAuth** for the Muse Code subscription (no refresh token). This is a normal provider for our router — no ToS grey zone.
- **Meta AI chat (meta.ai) web session** (only OmniRoute `muse-spark-web` is current): needs `ecto_1_sess` cookie + a ~5-minute `ecto1:` token scraped from the meta.ai page via headless Chromium (plain HTTP gets a JS challenge), GraphQL persisted queries at `https://www.meta.ai/api/graphql`, and a binary WebSocket at `wss://gateway.meta.ai/ws/clippy`. Fragile (doc_ids already rotated) and redundant given the official API. `graph.meta.ai` + `abra_sess`/`datr`/`lsd` belong to the 2024-era protocol (meta-ai-api) and look obsolete.
- **Muse agent app (muse.ai, "Hatch")**: only 9router `muse-web`. Paste `hatch_sess`, `hatch_gw`, `hatch_vml`, `hatch_native_auth_device`; renew via `GET https://muse.ai/api/session` (merge `Set-Cookie`); drive `https://muse.ai/thread/new` in Playwright (textarea + Send button, read `div[class*="hatch-agent-bubble-bg"]`, Stop button = generating); **auto-deny approval cards** so the agent cannot act for the user; tools rejected; also image/video generation. Since Muse is an agent that can send email and pay, an automated bridge carries real risk; if built, approval-denial and a clear per-connection risk notice are mandatory.
- If we build a web-session connection, the pieces worth copying: cookie paste with shape normalisation, per-provider cookie hint, test-connection, encrypted storage in our existing store (on Windows, DPAPI/Credential Manager would be better than anything these projects do), Set-Cookie write-back, 401 → "reconnect" state, 429 → cooldown, fresh/temporary chat per request, static model list, tools rejected or text-emulated, and a fixed risk banner. Skip all stealth/anti-bot layers.

## Sources
- https://github.com/decolua/9router — `open-sse/executors/{muse-web.js, muse-web/session.js, muse-web/dom.js, chatgpt-web.js, gemini-web.js}`, `open-sse/services/browserPool.js`, `open-sse/providers/registry/{muse.js, muse-web.js, chatgpt-web.js}`, `src/shared/constants/providersDisplay.js`
- https://github.com/diegosouzapw/OmniRoute — `open-sse/executors/{muse-spark-web.ts, muse-spark-web/fresh-access-token.ts, chatgpt-web.ts, claude-web.ts, gemini-web.ts, grok-web.ts}`, `docs/reference/PROVIDER_REFERENCE.md`, `docs/reference/ENVIRONMENT.md`
- https://github.com/router-for-me/CLIProxyAPI, https://cliproxyapidocs.pages.dev/hands-on/tutorial-3
- https://github.com/justlovemaki/AIClient-2-API (README)
- https://github.com/xtekky/gpt4free — `g4f/cookies.py`, `g4f/Provider/needs_auth/OpenaiChat.py`, `g4f/Provider/Copilot.py`
- https://github.com/HanaokaYuzu/Gemini-API (README, `src/gemini_webapi/constants.py`)
- https://github.com/Strvm/meta-ai-api (`src/meta_ai_api/main.py`), https://pypi.org/project/meta-ai-api/
- https://pypi.org/project/metaai-sdk/ (repo https://github.com/mir-ashiq/metaai-api)
- https://dev.meta.ai/docs/, https://docs.litellm.ai/docs/providers/meta, https://docs.openclaw.ai/nl/providers/meta
- Muse launch news: https://betanews.com/article/meta-muse-ai-agent-launch/, https://www.khaleejtimes.com/tech/meta-muse-ai-agent-rollout
