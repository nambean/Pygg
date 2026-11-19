# Providers

> **Status:** Current · connection presets, their endpoints and free tiers, which have answered live, and web sessions · reviewed 2026-11-14

Each preset in `apps/desktop/src-tauri/src/gateway/presets.rs` fills in a provider's endpoint, API format and default billing type. You can change any of them when connecting, for example to use a token plan's or a regional endpoint. Every endpoint below comes from the provider's own documentation.

**Live** means a real request went through the gateway and the provider answered it. Every other preset is configured from the provider's documentation and has not answered a real request yet. OpenAI, Groq and Mistral have only had their answer to an invalid key recorded, and DeepSeek its answer to an empty balance (HTTP 402); a successful request to any of them is not verified yet.

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

An Anthropic-format endpoint is written as the base an Anthropic SDK takes plus `/v1`, because the gateway appends `/messages`. Presets for one provider in both formats (DeepSeek, Kimi, Z.AI, MiniMax) share a quota group by default, since they take the same key.

**Several accounts on one connection.** A connection can hold API keys of several separate accounts, used in turn: in order, round robin, least used, or by the room each account's stated rate limits leave (ADR-038). A key that is rate-limited or out of credit cools down and the next key answers in the same request; a rejected key is blocked until you replace or remove it. Usage names the key that answered. Add keys in Providers → a connection → API keys, or with `pygg gateway key --id CONNECTION --label NAME --key-env VAR --separate-account yes`. Add only keys of separate accounts: several keys of one account share its limits. Web sessions, subscription sign-ins and local runtimes have no key ring.

## Local

| Preset | Default endpoint | Notes | Source |
| --- | --- | --- | --- |
| Ollama | `http://127.0.0.1:11434/v1` | **Live** (`qwen3:8b`, through the Anthropic surface for OpenCode). Agents send prompts of 20k tokens or more: start Ollama with a larger context (`OLLAMA_CONTEXT_LENGTH=32768`), or the prompt is cut. `qwen2.5-coder:7b` did not produce tool calls OpenCode could use | Ollama docs |
| LM Studio | `http://localhost:1234/v1` | key only if "Require Authentication" is on | https://lmstudio.ai/docs/developer/openai-compat |
| llama.cpp server | `http://127.0.0.1:9931/v1` | the README's default port; Docker examples use 8080 | https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md |
| vLLM | `http://localhost:8000/v1` | optional `--api-key` | https://docs.vllm.ai/en/latest/serving/online_serving/openai_compatible_server/ |
| Docker Model Runner | `http://localhost:12434/engines/v1` | TCP access must be enabled | https://docs.docker.com/ai/model-runner/api-reference/ |
| Jan | `http://127.0.0.1:1337/v1` | optional key | https://docs.jan.ai/desktop/api-server |

## Providers outside the list

**Custom (OpenAI-compatible)** and **Custom (Anthropic-compatible)** take any endpoint you enter, for a provider that has no preset. Billing starts as unknown and you set it. `/models` is read when the endpoint offers it; otherwise add model names by hand. From the command line, the key comes from an environment variable:

```
pygg gateway add-connection --preset custom --label "My provider" --endpoint https://api.example.com/v1 --key-env CUSTOM_API_KEY
```

Not verified yet: an Anthropic-compatible endpoint other than Anthropic's own.

## Web sessions (unofficial)

A web session uses an AI site that has no API through a Pygg window you sign in to yourself (ADR-035). It needs the desktop app.

- **Off until you say so.** Each connection starts off. You acknowledge the account-risk warning for that connection and sign in. The site's terms may forbid automated use, and the account can be limited or banned.
- **Billing.** Free by default ($0 a request: your plan with the site is paid by the month or free, never per request), or a subscription (counted as quota, cost unknown) if you choose.
- **What it does not do.** It is never chosen by `auto`; use it by name or in a combo. It takes no tools and no images.
- **One profile per connection.** Each connection has its own WebView2 profile with its own cookies, storage and sign-in. Pygg never changes the window's browser fingerprint. Connections of one site, each signed in to a different account, can form a pool.
- **When the page changes.** A site can change its page at any time; the connection then fails with "the page has changed".

Recipes, hosts and selectors are in `apps/desktop/src-tauri/src/websession/mod.rs` (`RECIPES`).

| Preset (id) | Site and new chat | Model | Live |
| --- | --- | --- | --- |
| Muse (`muse-web`) | `https://muse.ai/thread/new` | `muse` | yes: sign-in, a test message, and the benchmark (8 of 8). Selectors follow 9router's `muse-web` executor (MIT; see THIRD_PARTY_NOTICES.md) |
| ChatGPT (`chatgpt-web`) | `https://chatgpt.com/` | `chatgpt` | yes: sign-in with Google, Check Sign-In and Test, and the benchmark with two accounts (15 of 16) |
| Perplexity (`perplexity-web`) | `https://www.perplexity.ai/` | `perplexity` | yes: the benchmark (8 of 8) |
| DeepSeek (`deepseek-web`) | `https://chat.deepseek.com/` | `deepseek` | yes: the benchmark (8 of 8) |
| Grok (`grok-web`) | `https://grok.com/` | `grok` | yes: sign-in with Google, Check Sign-In and Test, and the benchmark (8 of 8) |
| Gemini (`gemini-web`) | `https://gemini.google.com/app` | `gemini` | selectors read from the live page in a browser (2026-11-19); not yet run in Pygg's window |

The benchmark is [web sessions](../benchmark/reports/2026-11-13-web-sessions.md): eight short checked tasks per account through the app's gateway, at Pygg's pace, with no money spent. Each live check was a single run. Not verified yet: use over many days, how close each site's own limits are, and a pool of two accounts used live.

### The guard

DeepSeek suspended an account after a run of 24 prompts of 2,000 to 6,000 tokens in 15 minutes, sent one at a time with a 10 s rest. Counting requests was not enough, so every web session goes through a guard (`websession/guard.rs`). It is always on, and its numbers are ceilings you cannot raise from the app.

- **Prompt size.** A prompt longer than 12,000 characters (about 3,000 tokens) is refused. Longer work, such as whole files, belongs on an API connection.
- **Weight.** A request weighs one unit, plus one for each full 2,000 characters of prompt.
- **Rest after a reply.** 10 s, plus 2 s for each extra unit, plus the time a person takes to read the reply (50 characters a second), at most three minutes. The rest is fixed, never random: Pygg slows down; it does not disguise itself.
- **Per hour and per 10 minutes.** At most 20 units and 15 requests in any hour, and 5 requests in any 10 minutes.
- **Per day.** A daily request limit, 50 by default, which you can set from 1 to 100, and twice that limit in units. Days start at 00:00 UTC.
- **One request at a time** per connection. A request waits in line at most 30 s; beyond that, or when a limit is reached, the gateway answers 429 with Retry-After and Pygg's reason, and nothing reaches the site.
- **Signs of trouble.** No reply in time, a usage-limit notice, a request to slow down, a check or a changed page rests the account: 5 minutes, doubled for each one in a row, at most 6 hours. A request to slow down rests it for at least half an hour. A clean reply clears it.
- **Suspension.** When the site says the account is suspended or restricted, Pygg turns the connection off and sends it nothing until you turn it back on.
- **DeepSeek** has lower ceilings: prompts of at most 4,000 characters, 8 units and 6 requests an hour, 3 requests in 10 minutes, at most 30 requests a day, and a rest that starts at 30 s.

The guard decides from what Pygg recorded, so it holds across restarts. If Pygg cannot read or write that record, it sends nothing.

### Sign-in hosts

While you have the window open to sign in, it may go to any https address, like a browser. When Pygg drives the window (hidden, for a request or a check), it keeps to the recipe's hosts; a refused address is blocked and named on the connection by host. A host allows its subdomains too.

| Preset | Hosts in the recipe | Why each is there |
| --- | --- | --- |
| `muse-web` | muse.ai, meta.com, meta.ai, facebook.com, instagram.com, threads.com, threads.net, messenger.com | muse.ai is the site. Meta's sign-in checks for an existing account by passing through its sites in turn (auth.muse.ai, facebook.com, instagram.com, auth.meta.com, back to muse.ai); blocking instagram.com left the window blank. meta.ai, threads.com, threads.net and messenger.com are Meta's other sites, allowed with them and not seen in that chain |
| `chatgpt-web` | chatgpt.com, openai.com, google.com, youtube.com, apple.com, live.com, microsoftonline.com, microsoft.com | chatgpt.com is the site; auth.openai.com is its email and phone sign-in. Google, Apple and Microsoft (login.live.com, login.microsoftonline.com) are its other sign-in buttons. The Google path was seen live; Apple and Microsoft are from the documented sign-in paths |
| `perplexity-web` | perplexity.ai, google.com, youtube.com, apple.com | perplexity.ai is the site and its emailed-code sign-in; Google and Apple are its other sign-in buttons. From the documented sign-in paths |
| `deepseek-web` | chat.deepseek.com, deepseek.com, google.com, youtube.com, apple.com | chat.deepseek.com is the site and sends a visitor to `/sign_in` (email or phone with a password); Google and Apple are its other sign-in buttons. From the documented sign-in paths |
| `grok-web` | grok.com, grokusercontent.com, x.ai, x.com, twitter.com, google.com, youtube.com, apple.com | grok.com is the site. After its OAuth callback Grok sets its cookie on auth.grok.com and then auth.grokusercontent.com; blocking that hop left the window white. accounts.x.ai is its email sign-in; x.com and twitter.com are "Sign in with X"; Google and Apple are its other buttons. The Google path was seen live |
| `gemini-web` | gemini.google.com, google.com, youtube.com | gemini.google.com is the site; Google is its only sign-in, through accounts.google.com and accounts.youtube.com. No other site's window reaches gemini.google.com, although they allow google.com to sign in with Google |

**Google's country sign-in hosts.** Google's sign-in also sets its cookie on the account's country domain, after accounts.youtube.com: `accounts.google.com.vn` for an account in Viet Nam. Blocking that hop left ChatGPT's Google sign-in spinning. A recipe that lists google.com therefore also allows exactly `accounts.google.` followed by a two-letter country code, `com.` and a code, or `co.` and a code (`accounts.google.de`, `accounts.google.com.vn`, `accounts.google.co.jp`), and nothing else of that shape: `www.google.com.vn` and `accounts.google.com.vn.evil.com` stay blocked (test `the_window_goes_only_to_the_sites_hosts`). Muse has no Google sign-in and does not get them.

- **Google sign-in.** Google may refuse to sign in inside an embedded window ("This browser or app may not be secure"). Pygg does not disguise the window, so the site's own email sign-in is then the way. Google's sign-in has worked in the window for ChatGPT and for Grok.
- **Visitor chats.** ChatGPT, Perplexity and Grok also chat with visitors. Their recipes treat a page that shows "Log in" or "Sign in" buttons as signed out, even when the prompt box is there.
- **Cloudflare.** Grok's site sits behind Cloudflare. Any check it shows stops the connection until you clear it in the window.
- **Official APIs.** Meta's Muse models (Meta (Muse API), above), OpenAI's, DeepSeek's and xAI's models all have official APIs, and Pygg has a preset for each. The muse.ai agent app has no API.
- **Not offered as web sessions:** Gemini (Google's API has a free tier, and Google may refuse sign-in in embedded windows); image- and video-only sites; scrapers that need no sign-in.

## Left out

- **GitHub Models**: retired as of July 30, 2026, per GitHub's docs.
- **Perplexity's API**: Sonar chat completions support ended on September 27, 2026. The API now uses Responses-style endpoints, which the gateway sends to OpenAI only (ADR-033: the gateway serves the Responses API to clients, and translates it to Chat Completions for every other provider).
- **Novita**: its docs do not make clear which base URL OpenAI clients should use. It will be added once checked with a request.
- **Cookie bridges (pasted or imported cookies), OAuth bridges into other products' subscriptions, and scrapers that need no sign-in** (ADR-026). A site with no API can be connected as a web session instead, where you sign in yourself and no cookie leaves its window.
