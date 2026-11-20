# Providers

> **Status:** Current · connection presets, their endpoints and free tiers, which have answered live, and web sessions · reviewed 2026-11-20

Presets live in `apps/desktop/src-tauri/src/gateway/presets.rs` (endpoint, API format, default billing type); any field can be changed when connecting, e.g. a token plan's or regional endpoint. Every endpoint is from the provider's own docs.

**Live** = a real request went through the gateway and the provider answered. Partly checked: OpenAI, Groq and Mistral (only an invalid-key answer recorded), DeepSeek (only HTTP 402 on an empty balance).

## Cloud, API key

| Preset | Endpoint | Format | `/models` lists | Free tier per the docs | Live | Source |
| --- | --- | --- | --- | --- | --- | --- |
| Google Gemini | `https://generativelanguage.googleapis.com/v1beta/openai` | OpenAI | ids only | yes (AI Studio) | yes | Google AI docs |
| Anthropic | `https://api.anthropic.com/v1` | Anthropic | input and output limits, image support | no | yes | Anthropic docs |
| OpenRouter | `https://openrouter.ai/api/v1` | OpenAI | context, output, tools, images, price | `:free` models | yes (a `:free` model, in OpenAI and Anthropic format) | OpenRouter docs |
| OpenAI | `https://api.openai.com/v1` | OpenAI | ids | no | no | OpenAI docs |
| Groq | `https://api.groq.com/openai/v1` | OpenAI | context window | Free plan | no | https://console.groq.com/docs/openai |
| DeepSeek | `https://api.deepseek.com` | OpenAI | context, output | granted balance only | no | https://api-docs.deepseek.com/ |
| DeepSeek (Anthropic API) | `https://api.deepseek.com/anthropic/v1` | Anthropic | — | as above | no | https://api-docs.deepseek.com/ |
| Qwen (Alibaba Model Studio) | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | OpenAI | ids | — | yes (token plan endpoint) | Alibaba Cloud docs |
| Mistral | `https://api.mistral.ai/v1` | OpenAI | context, tools, images | a free mode; the docs disagree on API access | no | https://docs.mistral.ai/api/endpoint/models |
| Cerebras | `https://api.cerebras.ai/v1` | OpenAI | ids | $5 trial credit with a payment method | no | https://inference-docs.cerebras.ai/resources/openai |
| Together AI | `https://api.together.ai/v1` | OpenAI | yes | not stated | no | https://docs.together.ai/docs/openai-api-compatibility |
| Fireworks AI | `https://api.fireworks.ai/inference/v1` | OpenAI | not documented (add models by hand) | small credit, not confirmed | no | https://docs.fireworks.ai/tools-sdks/openai-compatibility |
| xAI (Grok) | `https://api.x.ai/v1` | OpenAI (labeled legacy; Responses preferred) | context | prepaid only | no | https://docs.x.ai/developers/rest-api-reference/inference/chat-completions |
| NVIDIA (build.nvidia.com) | `https://integrate.api.nvidia.com/v1` | OpenAI | not stated | Developer Program | no | https://docs.api.nvidia.com/nim/reference/llm-apis |
| SambaNova Cloud | `https://api.sambanova.ai/v1` | OpenAI | per guides | free tier without a payment method | no | https://docs.sambanova.ai/docs/en/get-started/api-keys-urls |
| Hugging Face (Inference Providers) | `https://router.huggingface.co/v1` | OpenAI | per-provider details | PRO credit only | no | https://huggingface.co/docs/inference-providers/index |
| DeepInfra | `https://api.deepinfra.com/v1/openai` | OpenAI | elsewhere (add models by hand) | not stated | no | https://docs.deepinfra.com/chat/overview |
| SiliconFlow | `https://api.siliconflow.com/v1` | OpenAI | ids | not stated | no | https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions |
| Kimi (Moonshot) | `https://api.moonshot.ai/v1` | OpenAI | context, images | not stated | no | https://platform.kimi.ai/docs/api/list-models.md |
| Kimi (Anthropic API) | `https://api.moonshot.ai/anthropic/v1` | Anthropic | — | as above | no | as above |
| Z.AI (GLM) | `https://api.z.ai/api/paas/v4` (Coding Plan: `…/api/coding/paas/v4`) | OpenAI | not stated | some free Flash models | no | https://docs.z.ai/devpack/tool/others.md |
| Z.AI (Anthropic API) | `https://api.z.ai/api/anthropic/v1` | Anthropic | — | as above | no | as above |
| Cohere | `https://api.cohere.ai/compatibility/v1` | OpenAI | elsewhere | trial key, 1,000 calls a month | no | https://docs.cohere.com/docs/compatibility-api |
| MiniMax | `https://api.minimax.io/v1` | OpenAI | not stated | not stated | no | https://platform.minimax.io/docs/api-reference/text-openai-api |
| MiniMax (Anthropic API) | `https://api.minimax.io/anthropic/v1` | Anthropic | — | as above | no | as above |
| Nebius Token Factory | `https://api.tokenfactory.nebius.com/v1` | OpenAI | context (verbose) | not stated | no | https://docs.tokenfactory.nebius.com/ |
| Meta (Muse API) | `https://api.meta.ai/v1` | OpenAI (also Anthropic Messages and Responses) | yes | pay as you go; no free tier stated | no | https://dev.meta.ai/docs/ |

- **Anthropic-format endpoints** are the Anthropic SDK base plus `/v1`; the gateway appends `/messages`.
- **Quota groups:** presets for one provider in both formats (DeepSeek, Kimi, Z.AI, MiniMax) share one by default (same key); custom connections share one only with the same host.

**Key ring: several accounts on one connection** (ADR-038)

- Keys are used in order, round robin, least used, or by the room each account's stated rate limits leave.
- A rate-limited or out-of-credit key cools down and the next key answers in the same request; a rejected key is blocked until replaced or removed. Usage names the key that answered.
- Add: Providers → a connection → API keys, or `pygg gateway key --id CONNECTION --label NAME --key-env VAR --separate-account yes`.
- Only keys of separate accounts (one account's keys share its limits). No key ring for web sessions, subscription sign-ins or local runtimes.

## Local

| Preset | Default endpoint | Notes | Source |
| --- | --- | --- | --- |
| Ollama | `http://127.0.0.1:11434/v1` | **Live** (`qwen3:8b`, through the Anthropic surface for OpenCode). Agents send prompts of 20k tokens or more: start Ollama with `OLLAMA_CONTEXT_LENGTH=32768` or the prompt is cut. `qwen2.5-coder:7b` produced no tool calls OpenCode could use | Ollama docs |
| LM Studio | `http://localhost:1234/v1` | key only if "Require Authentication" is on | https://lmstudio.ai/docs/developer/openai-compat |
| llama.cpp server | `http://127.0.0.1:9931/v1` | the README's default port; Docker examples use 8080 | https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md |
| vLLM | `http://localhost:8000/v1` | optional `--api-key` | https://docs.vllm.ai/en/latest/serving/online_serving/openai_compatible_server/ |
| Docker Model Runner | `http://localhost:12434/engines/v1` | TCP access must be enabled | https://docs.docker.com/ai/model-runner/api-reference/ |
| Jan | `http://127.0.0.1:1337/v1` | optional key | https://docs.jan.ai/desktop/api-server |

## Providers outside the list

**Custom (OpenAI-compatible)** and **Custom (Anthropic-compatible)** take any endpoint. Billing starts as unknown until you set it; `/models` is read when offered, otherwise add model names by hand.

```
pygg gateway add-connection --preset custom --label "My provider" --endpoint https://api.example.com/v1 --key-env CUSTOM_API_KEY
```

Not verified yet: an Anthropic-compatible endpoint other than Anthropic's own.

## Web sessions (unofficial)

An AI site with no API, used through a Pygg window you sign in to yourself (ADR-035; desktop app only). Recipes, hosts and selectors: `RECIPES` in `apps/desktop/src-tauri/src/websession/mod.rs`.

- **Off until you say so:** each connection starts off; you acknowledge the account-risk warning and sign in. A site's terms may forbid automated use; the account can be limited or banned.
- **Billing:** free by default ($0 a request) or a subscription (quota, cost unknown) if you choose.
- **Never chosen by `auto`:** use by name or in a combo. No tools, no images.
- **One WebView2 profile per connection** (own cookies, storage, sign-in). The browser fingerprint is never changed. Connections of one site on different accounts can form a pool.
- **Page changes** make the connection fail with "the page has changed".

| Preset (id) | Site and new chat | Model | Live |
| --- | --- | --- | --- |
| Muse (`muse-web`) | `https://muse.ai/thread/new` | `muse` | yes: sign-in, a test message, the benchmark (8 of 8). Selectors follow 9router's `muse-web` executor (MIT; see THIRD_PARTY_NOTICES.md) |
| ChatGPT (`chatgpt-web`) | `https://chatgpt.com/` | `chatgpt` | yes: sign-in with Google, Check Sign-In and Test, the benchmark with two accounts (15 of 16) |
| Perplexity (`perplexity-web`) | `https://www.perplexity.ai/` | `perplexity` | yes: the benchmark (8 of 8) |
| DeepSeek (`deepseek-web`) | `https://chat.deepseek.com/` | `deepseek` | yes: the benchmark (8 of 8) |
| Grok (`grok-web`) | `https://grok.com/` | `grok` | yes: sign-in with Google, Check Sign-In and Test, the benchmark (8 of 8) |
| Gemini (`gemini-web`) | `https://gemini.google.com/app` | `gemini` | yes: Check Sign-In with the account shown, a text answer, a picture it made (2026-11-19) |

The benchmark is [web sessions](../benchmark/reports/2026-11-13-web-sessions.md): eight short checked tasks per account through the gateway, at Pygg's pace, $0; each live check was one run. Not verified yet: use over many days, how close each site's own limits are, a two-account pool used live.

### The guard

Every web session goes through `websession/guard.rs`: always on, and its ceilings cannot be raised from the app. It was set after DeepSeek suspended an account for 24 prompts of 2,000–6,000 tokens in 15 minutes, one at a time with a 10 s rest.

| Rule | All sites | DeepSeek |
| --- | --- | --- |
| Longest prompt (refused above) | 12,000 characters (about 3,000 tokens) | 4,000 characters |
| Weight of a request | 1 unit, plus 1 per full 2,000 characters | same |
| Rest after a reply | 10 s + 2 s per extra unit + reading time (50 characters/s), at most 3 minutes; fixed, never random | starts at 30 s |
| Per hour | 20 units, 15 requests | 8 units, 6 requests |
| Per 10 minutes | 5 requests | 3 requests |
| Per day (from 00:00 UTC) | 50 requests by default, settable 1–100; twice that in units | at most 30 requests |
| In flight | One request per connection; waits in line at most 30 s | same |

- **Refused** (limit reached or line too long): the gateway answers 429 with Retry-After and Pygg's reason; nothing reaches the site.
- **Signs of trouble** (no reply in time, a usage-limit notice, a request to slow down, a check, a changed page) rest the account 5 minutes, doubled for each in a row, at most 6 hours; at least 30 minutes after a slow-down request or a check, at least an hour after a usage-limit notice. A clean reply clears it.
- **Suspended or restricted** per the site: Pygg turns the connection off and sends nothing until you turn it on.
- The guard decides from Pygg's own record, so it holds across restarts; if Pygg cannot read or write that record, it sends nothing.

### Sign-in hosts

While you sign in, the window may go to any https address. When Pygg drives it (hidden, for a request or a check), it keeps to the recipe's hosts and their subdomains; a refused address is blocked and named on the connection.

| Preset | Hosts in the recipe | Why each is there |
| --- | --- | --- |
| `muse-web` | muse.ai, meta.com, meta.ai, facebook.com, instagram.com, threads.com, threads.net, messenger.com | muse.ai is the site. Meta's sign-in passes through auth.muse.ai, facebook.com, instagram.com, auth.meta.com and back to muse.ai (blocking instagram.com left the window blank). meta.ai, threads.com, threads.net, messenger.com: Meta's other sites, not seen in that chain |
| `chatgpt-web` | chatgpt.com, openai.com, google.com, youtube.com, apple.com, live.com, microsoftonline.com, microsoft.com | chatgpt.com is the site; auth.openai.com its email and phone sign-in. Google, Apple and Microsoft (login.live.com, login.microsoftonline.com) are its other sign-in buttons. Google path seen live; Apple and Microsoft from the documented paths |
| `perplexity-web` | perplexity.ai, google.com, youtube.com, apple.com | perplexity.ai is the site and its emailed-code sign-in; Google and Apple its other buttons. From the documented paths |
| `deepseek-web` | chat.deepseek.com, deepseek.com, google.com, youtube.com, apple.com | chat.deepseek.com is the site; visitors go to `/sign_in` (email or phone with a password); Google and Apple its other buttons. From the documented paths |
| `grok-web` | grok.com, grokusercontent.com, x.ai, x.com, twitter.com, google.com, youtube.com, apple.com | grok.com is the site. After OAuth it sets its cookie on auth.grok.com, then auth.grokusercontent.com (blocking that hop left the window white). accounts.x.ai is its email sign-in; x.com and twitter.com "Sign in with X"; Google and Apple its other buttons. Google path seen live |
| `gemini-web` | gemini.google.com, google.com, youtube.com | gemini.google.com is the site; Google is its only sign-in (accounts.google.com, accounts.youtube.com). No other recipe reaches gemini.google.com, although they allow google.com |

- **Google's country hosts.** A recipe that lists google.com also allows exactly `accounts.google.` + a two-letter country code, `com.` + code, or `co.` + code (`accounts.google.de`, `accounts.google.com.vn`, `accounts.google.co.jp`): Google sets its cookie there after accounts.youtube.com, and blocking it left ChatGPT's Google sign-in spinning. `www.google.com.vn` and `accounts.google.com.vn.evil.com` stay blocked (test `the_window_goes_only_to_the_sites_hosts`). Muse has no Google sign-in and does not get them.
- **Google sign-in** may be refused in an embedded window ("This browser or app may not be secure"); Pygg does not disguise the window, so use the site's email sign-in. It has worked for ChatGPT and Grok.
- **Visitor chats:** ChatGPT, Perplexity and Grok chat with visitors; a page with "Log in" or "Sign in" buttons counts as signed out, even with a prompt box.
- **Cloudflare:** Grok sits behind it; any check stops the connection until you clear it in the window.
- **Official APIs** exist for Meta's Muse models (Meta (Muse API) above), OpenAI, DeepSeek and xAI, each with a preset. The muse.ai agent app has no API.
- **Not offered as web sessions:** image- and video-only sites; scrapers that need no sign-in.

## Left out

- **GitHub Models:** retired as of July 30, 2026, per GitHub's docs.
- **Perplexity's API:** Sonar chat completions ended September 27, 2026; its Responses-style endpoints are not supported, since the gateway sends Responses to OpenAI only and translates it to Chat Completions for every other provider (ADR-033).
- **Novita:** its docs do not make clear which base URL OpenAI clients should use; to be added once checked with a request.
- **Cookie bridges (pasted or imported cookies), OAuth bridges into other products' subscriptions, scrapers that need no sign-in** (ADR-026). Use a web session instead: you sign in yourself and no cookie leaves its window.
