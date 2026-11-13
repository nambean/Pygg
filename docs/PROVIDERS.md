# Connection presets and their sources

> **Status:** Current · connection presets and their sources · reviewed 2026-11-13 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

Each preset in `apps/desktop/src-tauri/src/gateway/presets.rs` fills in a provider's endpoint, API format and default billing type. The user can change any of them when connecting, for example a token plan's or a regional endpoint. Every endpoint below comes from the provider's own documentation, checked on 2026-11-11. OmniRoute's provider reference was used only to decide which providers to look at (see [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md#provider-reference-review-2026-11-11)).

"Live" means a real request went through the gateway (see [Implementation Status](IMPLEMENTATION-STATUS.md)). Everything else is configured from the docs and not yet tried; OpenAI, Groq and Mistral have only had their answer to an invalid key recorded, as error fixtures. DeepSeek has also answered a real 402 for an empty balance (2026-11-12), so a successful DeepSeek API request is still unverified.

## Cloud, API key

| Preset | Endpoint | Format | `/models` lists | Free tier per the docs | Live | Source |
| --- | --- | --- | --- | --- | --- | --- |
| Google Gemini | `https://generativelanguage.googleapis.com/v1beta/openai` | OpenAI | ids only | yes (AI Studio) | yes | Google AI docs |
| Anthropic | `https://api.anthropic.com/v1` | Anthropic | input and output limits, image support | no | yes | Anthropic docs |
| OpenRouter | `https://openrouter.ai/api/v1` | OpenAI | context, output, tools, images, price | `:free` models | yes (2026-11-12, a `:free` model in OpenAI and Anthropic format, $0) | OpenRouter docs |
| OpenAI | `https://api.openai.com/v1` | OpenAI | ids | no | no | OpenAI docs |
| Groq | `https://api.groq.com/openai/v1` | OpenAI | context window | Free plan | no | https://console.groq.com/docs/openai |
| DeepSeek | `https://api.deepseek.com` | OpenAI | context, output | granted balance only | no | https://api-docs.deepseek.com/ |
| DeepSeek (Anthropic API) | `https://api.deepseek.com/anthropic/v1` | Anthropic | — | as above | no | https://api-docs.deepseek.com/ |
| Qwen (Alibaba Model Studio) | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | OpenAI | ids | — | yes (token plan endpoint) | Alibaba Cloud docs |
| Mistral | `https://api.mistral.ai/v1` | OpenAI | context, tools, images | a free mode; the docs disagree on API access | no | https://docs.mistral.ai/api/endpoint/models |
| Cerebras | `https://api.cerebras.ai/v1` | OpenAI | ids | $5 trial credit with a payment method | no | https://inference-docs.cerebras.ai/resources/openai |
| Together AI | `https://api.together.ai/v1` | OpenAI | yes | not stated | no | https://docs.together.ai/docs/openai-api-compatibility |
| Fireworks AI | `https://api.fireworks.ai/inference/v1` | OpenAI | not documented here (add models by hand) | small credit, not confirmed | no | https://docs.fireworks.ai/tools-sdks/openai-compatibility |
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
| Meta (Muse API) | `https://api.meta.ai/v1` | OpenAI (also Anthropic Messages and Responses) | yes | pay as you go; no free tier stated | no | https://dev.meta.ai/docs/ (checked 2026-11-12) |

An Anthropic-format endpoint is written as the base an Anthropic SDK takes plus `/v1`, because the gateway appends `/messages`. Presets for one provider in both formats (DeepSeek, Kimi, Z.AI, MiniMax) share a quota group by default, since they take the same key.

## Local

| Preset | Default endpoint | Notes | Source |
| --- | --- | --- | --- |
| Ollama | `http://127.0.0.1:11434/v1` | **Live** (2026-11-11, Ollama 0.40.16 in Docker on an RTX 3060, `qwen3:8b`, through the Anthropic surface for OpenCode). Agents send prompts of 20k tokens or more: start Ollama with a larger context (`OLLAMA_CONTEXT_LENGTH=32768`), or the prompt is cut. `qwen2.5-coder:7b` did not produce tool calls OpenCode could use | Ollama docs |
| LM Studio | `http://localhost:1234/v1` | key only if "Require Authentication" is on | https://lmstudio.ai/docs/developer/openai-compat |
| llama.cpp server | `http://127.0.0.1:9931/v1` | the README's default port; Docker examples use 8080 | https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md |
| vLLM | `http://localhost:8000/v1` | optional `--api-key` | https://docs.vllm.ai/en/latest/serving/online_serving/openai_compatible_server/ |
| Docker Model Runner | `http://localhost:12434/engines/v1` | TCP access must be enabled | https://docs.docker.com/ai/model-runner/api-reference/ |
| Jan | `http://127.0.0.1:1337/v1` | optional key | https://docs.jan.ai/desktop/api-server |

## Providers outside the list

**Custom (OpenAI-compatible)** and **Custom (Anthropic-compatible)** take any endpoint the user enters, for a provider that has no preset. Billing starts as unknown and the user sets it; `/models` is read when the endpoint offers it, otherwise model names are added by hand. Headless, the key comes from an environment variable, for example:

```
gateway add-connection --preset custom --label "My provider" --endpoint https://api.example.com/v1 --key-env CUSTOM_API_KEY
```

## Web session (unofficial)

A site with no API, used through a Pygg window the user signs in to (ADR-035). Off until the user acknowledges the account-risk warning for that connection and signs in; billed as free by default ($0 a request: the user's plan with the site, paid by the month or free, never per request; schema v27, 2026-11-13) or as a subscription (quota, cost unknown) at the user's choice; never chosen by `auto`; no tools or images.

Each connection has its own WebView2 profile: its own cookies, storage and sign-in. Pygg never changes a window's browser fingerprint. Each connection sends one request at a time, rests 10 s after each reply and stops at its daily limit (50 requests by default, 1 to 1,000, counted since 00:00 UTC; schema v26). Connections of one site, each signed in to a different account ("Another account on the site"), can form a pool; see [ACCOUNT_API_ROTATION.md](ACCOUNT_API_ROTATION.md). Recipes, hosts and selectors are in `apps/desktop/src-tauri/src/websession/mod.rs` (`RECIPES`); the presets are in `gateway/presets.rs`.

| Preset (id) | Site and new chat | Model | Selectors from | Live |
| --- | --- | --- | --- | --- |
| Muse (`muse-web`) | `https://muse.ai/thread/new` | `muse` | 9router's `muse-web` executor (MIT), see THIRD_PARTY_NOTICES.md | **yes**: the owner's sign-in and a test message, 2026-11-12 (HTTP 200, 13.9 s, cost unknown); benchmark 2026-11-13, 8 of 8 |
| ChatGPT (`chatgpt-web`) | `https://chatgpt.com/` | `chatgpt` | its public page on 2026-11-12, corrected live on 2026-11-13: signed in, the prompt box is a ProseMirror `div[contenteditable]` with no id (signed out, `#prompt-textarea`); a reply is the element after `[data-conversation-role=assistant]` (older pages: `[data-message-author-role=assistant]`) | **yes**: signed in with Google, Check Sign-In and Test answered (HTTP 200, 7.9 s), 2026-11-13; benchmark 2026-11-13 with two accounts, 15 of 16 (one 504 on a first request) |
| Perplexity (`perplexity-web`) | `https://www.perplexity.ai/` | `perplexity` | its public page on 2026-11-12 (a Lexical `div[contenteditable][role=textbox]`, a "Submit" button; answers `[id^=markdown-content-]` or `.prose`) | **yes**: benchmark 2026-11-13, 8 of 8 |
| DeepSeek (`deepseek-web`) | `https://chat.deepseek.com/` | `deepseek` | DeepSeek's chat page as commonly described (`textarea#chat-input`, Enter sends, answers `.ds-markdown`); first used signed in by the benchmark | **yes**: benchmark 2026-11-13, 8 of 8 |
| Grok (`grok-web`) | `https://grok.com/` | `grok` | Grok's chat page as commonly described (a `textarea` or ProseMirror box, a "Submit" button, answers `.response-content-markdown`); first used signed in on 2026-11-13 | **yes**: signed in with Google, Check Sign-In and Test answered (HTTP 200, 7.6 s), 2026-11-13; benchmark 2026-11-13, 8 of 8 |

The benchmark is [2026-11-13-web-sessions](../benchmark/reports/2026-11-13-web-sessions.md): eight short checked tasks per account through the running app's gateway, at Pygg's own pace, no money. Each live check above was one run on the owner's accounts; a site can change its page at any time, and the recipe then fails as "the page has changed".

### Sign-in hosts, and why each is there

A window the user has open to sign in may go to any https address, like a browser (ADR-035, 2026-11-13), so these lists no longer limit the user's own sign-in. They limit the window when Pygg drives it (hidden, for a request or a check): there a refused address is blocked and named on the connection by host. A host allows its subdomains too.

| Preset | Hosts in the recipe | Why each is there |
| --- | --- | --- |
| `muse-web` | muse.ai, meta.com, meta.ai, facebook.com, instagram.com, threads.com, threads.net, messenger.com | muse.ai is the site. Meta's sign-in checks for an existing account by passing through its sites in turn (the "aymh" redirect cycle: auth.muse.ai, facebook.com, instagram.com, auth.meta.com, back to muse.ai); blocking instagram.com left the window blank (found live, 2026-11-12, commit f8ea72e). meta.ai, threads.com, threads.net and messenger.com are Meta's other sites, allowed with them and not seen in that chain |
| `chatgpt-web` | chatgpt.com, openai.com, google.com, youtube.com, apple.com, live.com, microsoftonline.com, microsoft.com | chatgpt.com is the site; auth.openai.com is its email and phone sign-in. Google (accounts.google.com, then accounts.youtube.com and the country domain below), Apple and Microsoft (login.live.com, login.microsoftonline.com) are its other sign-in buttons. The Google path was seen live on 2026-11-13; Apple and Microsoft are from the documented sign-in paths and not seen live |
| `perplexity-web` | perplexity.ai, google.com, youtube.com, apple.com | perplexity.ai is the site and its emailed-code sign-in; Google and Apple are its other sign-in buttons. From the documented sign-in paths; which one the owner used on 2026-11-13 was not recorded |
| `deepseek-web` | chat.deepseek.com, deepseek.com, google.com, youtube.com, apple.com | chat.deepseek.com is the site and sends a visitor to `/sign_in` (email or phone with a password); Google and Apple are its other sign-in buttons. From the documented sign-in paths; which one the owner used on 2026-11-13 was not recorded |
| `grok-web` | grok.com, grokusercontent.com, x.ai, x.com, twitter.com, google.com, youtube.com, apple.com | grok.com is the site. After its OAuth callback Grok sets its cookie on auth.grok.com and then auth.grokusercontent.com; that hop was blocked and the window went white (found live, 2026-11-13, commit 5548303). accounts.x.ai is its email sign-in; x.com and twitter.com are "Sign in with X"; Google and Apple its other buttons. The Google path was seen live on 2026-11-13 |

**Google's country sign-in hosts** (`google_country_sign_in`, 2026-11-13). Google's sign-in also sets its cookie on the account's country domain, after accounts.youtube.com: `accounts.google.com.vn` for an account in Viet Nam. Blocking that hop left ChatGPT's Google sign-in spinning (found live, commit c34cf2a). A recipe that lists google.com therefore also allows exactly `accounts.google.` followed by a two-letter country code, `com.` and a code, or `co.` and a code (`accounts.google.de`, `accounts.google.com.vn`, `accounts.google.co.jp`), and nothing else of that shape: `www.google.com.vn`, `accounts.google.com.vn.evil.com` and the like stay blocked (test `the_window_goes_only_to_the_sites_hosts`). Muse has no Google sign-in and does not get them.

- **Google sign-in.** Google may refuse to sign in inside an embedded window ("This browser or app may not be secure"); Pygg does not disguise the window, so the site's own email sign-in is then the way. On 2026-11-13 Google's sign-in worked in the window for ChatGPT and for Grok.
- **Visitor chats.** ChatGPT, Perplexity and Grok also chat with visitors. Their recipes treat a page that shows "Log in" or "Sign in" buttons as signed out, even when the prompt box is there.
- **Cloudflare.** Grok's site sits behind Cloudflare. Any check it shows stops the connection until the user clears it in the window.
- **Official APIs.** Meta's Muse *models* (Meta (Muse API), above), OpenAI's, DeepSeek's and xAI's models all have official APIs, and Pygg has presets for each. Perplexity's Sonar API is paid and not supported by the gateway yet. The muse.ai agent app has no API.
- **Not added** ([research](research/2026-11-12-web-session-routers.md)):
  - Gemini web: Google's API has a free tier (the owner's decision, 2026-11-12), and Google may refuse sign-in in embedded windows.
  - 9router's UC (image and video only, adult content).
  - Veo AI Free (no sign-in; a video scraper).

## Left out

- **GitHub Models**: fully retired as of July 30, 2026, per GitHub's docs.
- **Perplexity**: Sonar chat completions support ended on September 27, 2026. The API now uses Responses-style endpoints, which the gateway sends to OpenAI only (ADR-033: the gateway serves the Responses API to clients, and translates it to Chat Completions for every other provider).
- **Novita**: its docs do not make clear which base URL OpenAI clients should use. To be added once checked with a request.
- **Cookie bridges (pasted or imported cookies), OAuth bridges into other products' subscriptions, and no-auth scrapers**: see [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md#provider-reference-review-2026-11-11). A site with no API can be connected as a web session instead (above, ADR-035), where the user signs in themselves and no cookie leaves its window.

## Reusing OmniRoute code

OmniRoute is MIT-licensed ("Copyright (c) 2026 diegosouzapw", checked at `release/v3.8.54`). The license allows reading, porting and reusing its code, provided the copyright and license notice is kept with any substantial portion. If code is ported, its origin goes in a `THIRD_PARTY_NOTICES.md` with that notice. None has been ported so far. The license covers OmniRoute's code. It does not change what each provider's own terms allow.
