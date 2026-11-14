# Resilience: what each gateway does when a provider fails, 2026-11-14

> **Status:** Record · $0 (a fault-injecting fake provider; one live check with revoked keys, which Anthropic refuses without charge) · raw results: [`results/2026-11-14-resilience`](../results/2026-11-14-resilience/) · scripts: `benchmark/resilience/` · decisions: [ADR-038](../../docs/DECISIONS.md) (key rings), the gateway's failure policy (`apps/desktop/src-tauri/src/gateway/policy.rs`)

**Question.** When a provider runs out of credit (402), is rate-limited (429), rejects a revoked key (401), fails (5xx), cannot be reached, drops the connection, hangs or cuts a stream, what does the coding agent get, how many requests reach the providers, is anything run twice, and does the ledger keep every attempt? Measured for Pygg and for LiteLLM proxy with the same targets, the same fake provider and the same requests.

**Answer.**
- **Out of credit, rate-limited, revoked key, unreachable:** both gateways served **every request**. Pygg sent about **1.04 provider requests per client request** and answered in about **70 ms** (the fake takes 40 ms): the failing target is cooled down after its first refusal, for the time its `Retry-After` says (30 s without one, 10 minutes for no credit, until fixed for a revoked key), and later requests go straight to the fallback. LiteLLM with its defaults and a fallback list cooled down the revoked key as Pygg did, but went back to the failing provider on **every** request for the rest: **4 provider requests** per client request and about **4.6 s** for a 429 or a 500, 2 requests for a 402, about 4.6 s with an unreachable primary, and **about 88 s** per request when the 429 said `Retry-After: 30`, because it waits that long between its retries.
- **A key ring:** with one account's key rate-limited and another's out of credit, both gateways served everything from the third key. Pygg kept every session on one key (**60 of 60** over three runs) while using all three keys; LiteLLM did so only with its session-affinity check turned on (0 of 60 by default, 60 of 60 tuned).
- **A provider that may have run the request (500, dropped connection, timeout):** this is where they differ by design. Pygg does not send such a request again unless the client allows it, because the provider may have billed it: with a provider failing every request with 500 it served **96%** (288 of 300; the first requests, before the cooldown, got the error), with one dropping every connection **none**, and with one retry allowed a 500 was served **100%**. LiteLLM served them all, by sending each request to the failing provider three times first: every one of those requests could have been billed up to three times by a provider that runs requests before it fails them. With a 30 s timeout configured, a hanging provider was tried three times (90 s) before the fallback, and each abandoned attempt kept running at the provider.
- **A stream cut in the middle:** neither can finish it. Pygg passed the break to the client every time (the client saw an incomplete answer and an error, never a spliced one). LiteLLM served 61 and 70 of 300 (defaults, tuned) cut streams from the fallback, the ones where the cut came before it had sent the client anything, and passed the error for the rest.
- **The ledger:** Pygg recorded **one row for every attempt that reached a provider**, plus one for each attempt that could not connect, each with its HTTP status, its outcome and, on a key ring, the key; attempts whose cost the provider may have charged are recorded as unknown, not $0. No request was answered twice by the providers in any run of either gateway.
- **Live check:** a key ring of two revoked keys on Anthropic's real API, then local Ollama: Anthropic answered 401 to each key, both keys were marked invalid, the request was answered by Ollama, and the next two requests went straight to Ollama.

**Where Pygg is worse here:** it loses requests LiteLLM saves when a provider fails after receiving the request (a 5xx before the cooldown starts, a dropped connection, a stream cut before the first byte reached the client), and it has no timeout fallback for a provider that hangs. Those are the price of never sending a request twice unless allowed; the 5xx retry can be turned on per client.

## Method

`benchmark/resilience/run.py` against `benchmark/resilience/fake_provider.py`:

```text
python -I benchmark/resilience/run.py --gateway direct  --out benchmark/results/2026-11-14-resilience --repeat 3
python -I benchmark/resilience/run.py --gateway pygg    --out … --repeat 3 --pygg-exe <gateway.exe built from 80d35d2>
python -I benchmark/resilience/run.py --gateway pygg    --out … --repeat 3 --label pygg-retry1 --pygg-paid-retries 1
python -I benchmark/resilience/run.py --gateway litellm --out … --repeat 3 --litellm-image ghcr.io/berriai/litellm:v1.106.2
python -I benchmark/resilience/run.py --gateway litellm --out … --repeat 3 --label litellm-tuned --litellm-config benchmark/resilience/litellm-config-tuned.yaml
```

- **The fake provider** speaks OpenAI's Chat Completions, whole and streamed, in a `python:3.11-slim` container on a Docker network, published on 127.0.0.1. Several providers share it: the path names the provider (`a`, `b`, `r`), the bearer token the key. For each scenario it is told what each provider or key does: answer, a status (401, 402, 429 with or without `Retry-After`, 429 `insufficient_quota`, 500), fail a share of requests, wait before answering, close the connection without an answer, or start a streamed answer and cut it after two chunks. It answers in 40 ms, and every answer names the request id the client put in its message and the provider and key that answered. It logs every request that reached it.
- **The same targets in every gateway.** A primary `a` with a fallback `b` (Pygg: a combo; LiteLLM: `fallbacks`); a key ring of three accounts' keys `r1`–`r3` with `b` as fallback (Pygg: one connection with a ring, order `round_robin`; LiteLLM: three deployments of one model group); and a primary whose port refuses connections (`d`). Pygg's four connections are four different provider kinds, so each is its own quota group, as four real providers would be. LiteLLM's configuration is in [`litellm-config.yaml`](../resilience/litellm-config.yaml): its documented defaults plus the fallbacks; the "tuned" copy adds session affinity (`deployment_affinity`) and a 30 s timeout, as its reliability docs suggest.
- **Configurations:**
  - `direct`: no gateway, the client calls `a` (or `r1`) itself: what an agent sees without one.
  - `pygg`: the headless gateway (`gateway serve`) built from commit 80d35d2, as shipped.
  - `pygg-retry1`: the same with the client allowed one retry after a server error (`--paid-retries 1`, Clients → Limits). Off by default because a provider may have run, and billed, a request it answered with a 5xx.
  - `litellm`: LiteLLM proxy `ghcr.io/berriai/litellm:v1.106.2` (digest `sha256:7294b90f868c5c38c99b83021c7ba46f9b439e95f8e32964c925c7d2563a03f7`, built before the window of LiteLLM's 2026-10-21 incident, the version its incident post recommends pinning).
  - `litellm-tuned`: the same with the tuned configuration.
- **Each scenario** starts the gateway on a fresh state (Pygg: a copy of a store set up once; LiteLLM: a new container), so no cooldown carries over, then sends 100 requests (8 for the 45-second hang), half of them streamed, from 4 clients in sessions of 5 requests sent one after another (a session header on each: `x-smarttaskrouter-session` for Pygg, `x-litellm-session-id` and `litellm_session_id` for LiteLLM). Three runs of every scenario.
- **What is counted.** A request is *served* when the client got HTTP 200 with its own request id and, when streamed, a complete stream. *Upstream attempts* are requests the fake logged. *Duplicated* is a request the providers answered successfully more than once (billed twice). For Pygg, the ledger (`usage_events`) is read after each scenario and compared with the fake's log.
- **Exceptions.** LiteLLM's `429-retry-after` scenario ran once per configuration (100 requests) instead of three times: each request took about 88 s. `auto-429-hold` is Pygg only (an `auto/cheap` alias over the same targets, with `a` the cheapest). LiteLLM's tuned configuration ran against a second copy of the fake provider so both LiteLLM configurations could run at once, and the `auto` scenario against a third.
- **Machine:** Intel Core i7-8700K, 32 GB, Windows 11 Pro, Docker Desktop 29.8.0 (WSL 2). Pygg runs natively and LiteLLM in a container; both paths cross Docker's port proxy once.

## Results

Scenarios (100 requests each unless noted): `healthy`; `429-retry-after` (primary answers 429 with `Retry-After: 30`); `429-no-reset` (429, no reset stated); `402-no-credit`; `429-insufficient-quota` (OpenAI's out-of-credit 429); `401-revoked`; `500-down` (500 to everything); `503-flaky-30` (500 to 30% of requests); `unreachable` (the primary's port refuses connections); `dropped` (reads the request, closes without answering); `cut-midstream` (streams two chunks, then closes; all requests streamed); `hang-45s` (answers after 45 s; 8 requests); `ring-mixed` (key ring: key 1 429, key 2 402, key 3 healthy); `ring-all-dead` (all three keys fail: 429, 402, 401); `ring-affinity` (three healthy keys); `auto-429-hold` (Pygg's `auto/cheap`, cheapest model answers 429).

### Served, and provider requests per client request

Served / sent · provider requests per client request · requests answered twice, if any (all runs of a scenario together). Bold: not every request served.

| Scenario | direct | pygg | pygg-retry1 | litellm | litellm-tuned |
| --- | --- | --- | --- | --- | --- |
| healthy | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 |
| 429-retry-after | **0/300** · 1.00 | 300/300 · 1.04 | 300/300 · 1.04 | 100/100 · 4.00 | 100/100 · 4.00 |
| 429-no-reset | **0/300** · 1.00 | 300/300 · 1.04 | 300/300 · 1.03 | 300/300 · 4.00 | 300/300 · 4.00 |
| 402-no-credit | **0/300** · 1.00 | 300/300 · 1.04 | 300/300 · 1.04 | 300/300 · 2.00 | 300/300 · 2.00 |
| 429-insufficient-quota | **0/300** · 1.00 | 300/300 · 1.04 | 300/300 · 1.04 | 300/300 · 4.00 | 300/300 · 4.00 |
| 401-revoked | **0/300** · 1.00 | 300/300 · 1.04 | 300/300 · 1.04 | 300/300 · 1.04 | 300/300 · 1.04 |
| 500-down | **0/300** · 1.00 | **288/300** · 1.00 | 300/300 · 1.04 | 300/300 · 4.00 | 300/300 · 4.00 |
| 503-flaky-30 | **210/300** · 1.00 | **279/300** · 1.00 | 300/300 · 1.06 | 300/300 · 1.41 | 300/300 · 1.42 |
| unreachable | **0/300** · 0.00 | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 |
| dropped | **0/300** · 1.00 | **0/300** · 1.00 | **0/300** · 1.00 | 300/300 · 4.00 | 300/300 · 4.00 |
| cut-midstream | **0/300** · 1.00 | **0/300** · 1.00 | **0/300** · 1.00 | **61/300** · 1.20 | **70/300** · 1.23 |
| hang-45s | 24/24 · 1.00 | 24/24 · 1.00 | 24/24 · 1.00 | 24/24 · 1.00 | 24/24 · 3.88 |
| ring-mixed | **0/300** · 1.00 | 300/300 · 1.03 | 300/300 · 1.03 | 300/300 · 1.04 | 300/300 · 1.03 |
| ring-all-dead | **0/300** · 1.00 | 300/300 · 1.05 | 300/300 · 1.06 | 300/300 · 1.09 | 300/300 · 1.10 |
| ring-affinity | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 | 300/300 · 1.00 |
| auto-429-hold | — | 300/300 · 1.04 | — | — | — |

### Time to the answer

Median latency of served requests in ms (the median of the runs' medians). The fake provider answers in 40 ms.

| Scenario | direct | pygg | pygg-retry1 | litellm | litellm-tuned |
| --- | ---: | ---: | ---: | ---: | ---: |
| healthy | 59 | 72 | 72 | 91 | 92 |
| 429-retry-after | — | 73 | 72 | 87733 | 87583 |
| 429-no-reset | — | 72 | 69 | 4580 | 4612 |
| 402-no-credit | — | 73 | 70 | 146 | 144 |
| 429-insufficient-quota | — | 71 | 74 | 4588 | 4607 |
| 401-revoked | — | 69 | 74 | 104 | 101 |
| 500-down | — | 69 | 73 | 4659 | 4615 |
| 503-flaky-30 | 58 | 70 | 69 | 81 | 80 |
| unreachable | — | 73 | 74 | 4658 | 4591 |
| dropped | — | — | — | 4707 | 4702 |
| cut-midstream | — | — | — | 267 | 283 |
| hang-45s | 43382 | 43393 | 43351 | 43367 | 91260 |
| ring-mixed | — | 77 | 75 | 96 | 97 |
| ring-all-dead | — | 74 | 76 | 101 | 111 |
| ring-affinity | 57 | 74 | 74 | 88 | 92 |
| auto-429-hold | — | 76 | — | — | — |

### Pygg's ledger against what reached the provider

Default configuration (`pygg`):

| Scenario | Runs | Upstream attempts | Ledger rows | Rows for attempts that never connected | Requests with rows | Cost unknown | Keys named |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| healthy | 3 | 300 | 300 | 0 | 300/300 | 0 | 0 |
| 429-retry-after | 3 | 311 | 311 | 0 | 300/300 | 0 | 0 |
| 429-no-reset | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |
| 402-no-credit | 3 | 311 | 311 | 0 | 300/300 | 0 | 0 |
| 429-insufficient-quota | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |
| 401-revoked | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |
| 500-down | 3 | 300 | 300 | 0 | 300/300 | 12 | 0 |
| 503-flaky-30 | 3 | 300 | 300 | 0 | 300/300 | 21 | 0 |
| unreachable | 3 | 300 | 312 | 12 | 300/300 | 0 | 0 |
| dropped | 3 | 300 | 300 | 0 | 300/300 | 300 | 0 |
| cut-midstream | 3 | 300 | 300 | 0 | 300/300 | 300 | 0 |
| hang-45s | 3 | 24 | 24 | 0 | 24/24 | 0 | 0 |
| ring-mixed | 3 | 310 | 310 | 0 | 300/300 | 0 | 3 |
| ring-all-dead | 3 | 316 | 316 | 0 | 300/300 | 0 | 3 |
| ring-affinity | 3 | 300 | 300 | 0 | 300/300 | 0 | 3 |
| auto-429-hold | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |

With one retry allowed (`pygg-retry1`):

| Scenario | Runs | Upstream attempts | Ledger rows | Rows for attempts that never connected | Requests with rows | Cost unknown | Keys named |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| healthy | 3 | 300 | 300 | 0 | 300/300 | 0 | 0 |
| 429-retry-after | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |
| 429-no-reset | 3 | 310 | 310 | 0 | 300/300 | 0 | 0 |
| 402-no-credit | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |
| 429-insufficient-quota | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |
| 401-revoked | 3 | 312 | 312 | 0 | 300/300 | 0 | 0 |
| 500-down | 3 | 312 | 312 | 0 | 300/300 | 12 | 0 |
| 503-flaky-30 | 3 | 318 | 318 | 0 | 300/300 | 18 | 0 |
| unreachable | 3 | 300 | 312 | 12 | 300/300 | 0 | 0 |
| dropped | 3 | 300 | 300 | 0 | 300/300 | 300 | 0 |
| cut-midstream | 3 | 300 | 300 | 0 | 300/300 | 300 | 0 |
| hang-45s | 3 | 24 | 24 | 0 | 24/24 | 0 | 0 |
| ring-mixed | 3 | 309 | 309 | 0 | 300/300 | 0 | 3 |
| ring-all-dead | 3 | 318 | 318 | 0 | 300/300 | 0 | 3 |
| ring-affinity | 3 | 300 | 300 | 0 | 300/300 | 0 | 3 |

### Served requests that may have been billed more than once

Served requests that also had an attempt the provider may have run before it failed: a 5xx, a connection closed after the request was sent, a cut stream, or an attempt the gateway abandoned after its timeout while the provider was still working.

| Scenario | direct | pygg | pygg-retry1 | litellm | litellm-tuned |
| --- | ---: | ---: | ---: | ---: | ---: |
| healthy | 0 | 0 | 0 | 0 | 0 |
| 429-retry-after | — | 0 | 0 | 0 | 0 |
| 429-no-reset | — | 0 | 0 | 0 | 0 |
| 402-no-credit | — | 0 | 0 | 0 | 0 |
| 429-insufficient-quota | — | 0 | 0 | 0 | 0 |
| 401-revoked | — | 0 | 0 | 0 | 0 |
| 500-down | — | 0 | 12 | 300 | 300 |
| 503-flaky-30 | 0 | 0 | 18 | 97 | 100 |
| unreachable | — | 0 | 0 | 0 | 0 |
| dropped | — | — | — | 300 | 300 |
| cut-midstream | — | — | — | 61 | 70 |
| hang-45s | 0 | 0 | 0 | 0 | 24 |
| ring-mixed | — | 0 | 0 | 0 | 0 |
| ring-all-dead | — | 0 | 0 | 0 | 0 |
| ring-affinity | 0 | 0 | 0 | 0 | 0 |
| auto-429-hold | — | 0 | — | — | — |

### Session affinity (ring of three healthy keys, sessions of 5)

- Session affinity (direct): 60/60 sessions stayed on one key; keys used: ['r/key-r1']
- Session affinity (pygg): 60/60 sessions stayed on one key; keys used: ['r/key-r1', 'r/key-r2', 'r/key-r3']
- Session affinity (pygg-retry1): 60/60 sessions stayed on one key; keys used: ['r/key-r1', 'r/key-r2', 'r/key-r3']
- Session affinity (litellm): 0/60 sessions stayed on one key; keys used: ['r/key-r1', 'r/key-r2', 'r/key-r3']
- Session affinity (litellm-tuned): 60/60 sessions stayed on one key; keys used: ['r/key-r1', 'r/key-r3']

## Findings in detail

- **Cooldowns.** Pygg's extra attempts in the 429, 402 and 401 scenarios (about 4 per run of 100) are the requests already in flight when the first refusal came back; after it, the target is skipped until its cooldown ends. LiteLLM cooled down a failing deployment only inside a model group of several deployments (the key ring, 1.04–1.10 attempts per request). With the primary and the fallback as separate model groups linked by `fallbacks`, as its fallback docs show, the primary was tried again on every request, three times for a 429 or a 500, waiting between tries. A single model group with ordered deployments might cool it down; that configuration was not run.
- **`Retry-After`.** Pygg uses it as the cooldown. LiteLLM waited about 29 s between its retries when the 429 said 30 s (this scenario was run once for each LiteLLM configuration, 100 requests, because it took 37 minutes: a median of 88 s per request).
- **Requests the provider may have run.** The bottom table counts served requests that also had an attempt the provider may have run before it failed (a 5xx, a connection closed after the request was sent, a cut stream, or an attempt the gateway gave up on while the provider was still working). Pygg's default has none: it answers the client with the error instead. With one retry allowed, 12 of 300 requests in the 500 scenario and 18 in the flaky one were such cases, once each. LiteLLM had one for every request it saved this way, with up to three attempts each.
- **A hanging provider.** Both waited for it by default (about 43 s, then the answer). LiteLLM with a 30 s timeout gave up three times before falling back (91 s); each abandoned attempt went on at the provider, which in this fake finished all of them.
- **A cut stream.** Pygg forwards a stream from its first byte, so a cut always reaches the client as a broken stream with the text so far. LiteLLM reads the start of a stream before answering: when the cut came within that window it fell back cleanly (61 and 70 of 300); otherwise the client got the partial text and an error event.
- **Session affinity.** Pygg pins a session to the key that first answered it and still spread sessions over all three keys (`round_robin`). LiteLLM's tuned configuration pinned every session, but put them on fewer keys.
- **`auto` and a 429** (Pygg only; LiteLLM has no such alias): `auto/cheap` routed to the cheapest model `a`; when it answered 429 with no reset, every request was served from `b` and `a` was held out of `auto`. The four requests in flight when the first 429 came back counted as four strikes in a row, so the hold was 80 minutes (10 × 2³) instead of the 10 minutes a single strike gives. That is how the hold is specified (each 429 is a strike), but concurrent refusals of one burst arguably should count once. Since changed: a 429 to a request sent before the last strike was recorded now repeats that strike, so this burst holds the model for 10 minutes (`gateway/auto.rs`, not rerun here).
- **The ledger.** In every Pygg run, the ledger had exactly one row per attempt the fake provider logged, plus one row per attempt that could not connect (the `unreachable` scenario, 12 in all), and every client request had its rows. Attempts that ended in a 5xx, a dropped connection or a cut stream were recorded with an unknown cost. Ring attempts name the key (3 keys). LiteLLM's spend log needs a database, which these runs did not set up, so its ledger was not compared.

## Live confirmation

`results/2026-11-14-resilience/live-401-ring.json`: a connection to Anthropic's API (`api.anthropic.com`) with a key ring of two invalid keys, then local Ollama (`qwen2.5-coder:7b`), as a combo; three requests in one session (whole, streamed, whole). The first request: Anthropic answered 401 "invalid x-api-key" to each key, both were marked `invalid_credentials` until fixed, and Ollama answered (9 s, the model loading). The next two went to Ollama directly (0.26 s each), with no request to Anthropic. Nothing was charged: Anthropic refuses an invalid key before any work. No other live provider was used.

## What this does not show

- **Real providers' failures.** The fake fails cleanly and instantly; real providers fail slowly, partly, and with bodies Pygg has to read (Pygg's policy covers the formats in `tests/fixtures/provider-errors`). Only the 401 path was confirmed live.
- **Whether a provider bills a failed request.** The tables count attempts that *could* have been billed; providers differ, and most do not bill a request refused with 429, 402 or 401.
- **Other gateways' behaviour beyond LiteLLM's two configurations.** LiteLLM has many settings (retry policies, allowed fails, order-based groups, mid-stream continuation); these runs used its defaults plus fallbacks, and one tuned variant. The other projects in [docs/BENCHMARKS.md](../../docs/BENCHMARKS.md) were not run.
- **Load.** Four clients at a time; the throughput limit is in the [overhead report](2026-11-14-gateway-overhead.md).
- **Latency comparisons between gateways.** Pygg runs natively; LiteLLM runs in a container. Both paths cross Docker's port proxy once, but the healthy-path latencies (72 ms against 91 ms, with a 40 ms provider) are not a fair overhead measurement: see the overhead report for Pygg and LiteLLM's own published figures.
